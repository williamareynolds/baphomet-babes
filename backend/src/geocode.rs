//! Address → coordinates, via OpenStreetMap's Nominatim.
//!
//! Chosen to match the Leaflet/OSM map the hub already draws, and because it
//! costs nothing — a paid geocoder would bill against the project's $30 cap for
//! a handful of lookups a month.
//!
//! Nominatim's usage policy caps clients at one request per second and requires
//! an identifying User-Agent, so this runs server-side where both can actually
//! be enforced: from the browser we could neither rate-limit across members nor
//! stop the client sending whatever User-Agent it liked.
//!
//! A lookup failing is never fatal. The admin form keeps the pin hand-editable,
//! so a bad match or an outage means dropping the pin manually, not being
//! unable to post a gathering.

use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const NOMINATIM: &str = "https://nominatim.openstreetmap.org/search";

/// Nominatim asks for a real contact address in the User-Agent so they can get
/// in touch before blocking a misbehaving client.
const USER_AGENT: &str = "BaphometBabes/1.0 (https://baphometbabes.com)";

/// Their policy is one request per second, absolute.
const MIN_INTERVAL: Duration = Duration::from_millis(1_100);

/// Serializes lookups process-wide and spaces them out. Cloud Run may run
/// several instances, so this is a courtesy floor rather than a hard guarantee
/// — at a few lookups a month that is comfortably inside the policy.
static LAST_CALL: Mutex<Option<Instant>> = Mutex::const_new(None);

#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    pub lat: f64,
    pub lng: f64,
    pub display_name: String,
}

/// Pull the first usable result out of a Nominatim response.
///
/// Split from the request so the parsing — including their habit of returning
/// coordinates as strings — is testable without hitting their servers.
pub fn parse_first(body: &str) -> Option<Located> {
    let results: serde_json::Value = serde_json::from_str(body).ok()?;
    let first = results.as_array()?.first()?;
    // lat/lon come back as strings, not numbers.
    let lat = first.get("lat")?.as_str()?.parse::<f64>().ok()?;
    let lng = first.get("lon")?.as_str()?.parse::<f64>().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
        return None;
    }
    let display_name = first
        .get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    Some(Located { lat, lng, display_name })
}

/// Look up `query`, returning `None` when nothing matches or the service is
/// unreachable. Callers treat both the same: leave the pin to the human.
pub async fn lookup(query: &str) -> Option<Located> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }

    // Space out calls to stay inside the usage policy.
    {
        let mut last = LAST_CALL.lock().await;
        if let Some(prev) = *last {
            let since = prev.elapsed();
            if since < MIN_INTERVAL {
                tokio::time::sleep(MIN_INTERVAL - since).await;
            }
        }
        *last = Some(Instant::now());
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(NOMINATIM)
        .query(&[("q", query), ("format", "json"), ("limit", "1")])
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        tracing::warn!("geocode lookup returned {}", resp.status());
        return None;
    }
    parse_first(&resp.text().await.ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_normal_result() {
        let body = r#"[{"lat":"36.3729","lon":"-94.2088","display_name":"Bentonville, Arkansas"}]"#;
        let got = parse_first(body).unwrap();
        assert_eq!(got.lat, 36.3729);
        assert_eq!(got.lng, -94.2088);
        assert_eq!(got.display_name, "Bentonville, Arkansas");
    }

    #[test]
    fn no_match_is_none_not_an_error() {
        assert_eq!(parse_first("[]"), None);
    }

    #[test]
    fn garbage_is_none() {
        assert_eq!(parse_first("not json"), None);
        assert_eq!(parse_first("{}"), None);
        // Numeric lat/lon would be a format change on their end; don't guess.
        assert_eq!(parse_first(r#"[{"lat":36.37,"lon":-94.2}]"#), None);
        assert_eq!(parse_first(r#"[{"lat":"nope","lon":"-94.2"}]"#), None);
    }

    #[test]
    fn out_of_range_coordinates_are_rejected() {
        // Better no pin than a pin off the planet, which our own validation
        // would reject on save anyway.
        assert_eq!(parse_first(r#"[{"lat":"91.0","lon":"0"}]"#), None);
        assert_eq!(parse_first(r#"[{"lat":"0","lon":"181"}]"#), None);
    }

    #[test]
    fn a_missing_display_name_still_yields_coordinates() {
        let got = parse_first(r#"[{"lat":"36.37","lon":"-94.2"}]"#).unwrap();
        assert_eq!(got.display_name, "");
    }
}
