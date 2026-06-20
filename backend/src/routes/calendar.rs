//! Subscribable calendar (iCalendar / .ics) feed.
//!
//! Each member has a secret, revocable token (`/calendar/me`). The feed itself
//! lives at a public capability URL — `/calendar/{token}/baphomet-babes.ics` —
//! because Google/iCloud/Outlook fetch it anonymously (no auth header, no App
//! Check token). The token in the path is the only credential; the App Check
//! middleware exempts paths ending in `.ics` so the feed stays reachable.

use crate::{
    AppState,
    auth::require_auth,
    error::{AppError, AppResult},
    models::{CalendarTokenDoc, EventDoc},
};
use anyhow::Context;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use firestore::FirestoreQueryDirection;
use shared::CalendarToken;
use uuid::Uuid;

const EVENTS: &str = "movie_nights";
const CALENDAR_TOKENS: &str = "calendar_tokens";

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/me", get(my_token))
        .route("/me/regenerate", post(regenerate_token))
        .route("/{token}/baphomet-babes.ics", get(feed))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn new_token() -> String {
    Uuid::new_v4().simple().to_string()
}

// ---- per-user token (authenticated) ----

/// Return the caller's calendar token, minting one on first use.
async fn my_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<CalendarToken>> {
    let claims = require_auth(&state, &headers).await?;

    let existing: Option<CalendarTokenDoc> = state.db
        .fluent()
        .select()
        .by_id_in(CALENDAR_TOKENS)
        .obj()
        .one(&claims.sub)
        .await
        .context("failed to load calendar token")?;

    let token = match existing {
        Some(d) => d.token,
        None => mint_token(&state, &claims.sub).await?,
    };
    Ok(Json(CalendarToken { token }))
}

/// Rotate the caller's token: the old subscription URL stops working at once.
async fn regenerate_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<CalendarToken>> {
    let claims = require_auth(&state, &headers).await?;
    let token = mint_token(&state, &claims.sub).await?;
    Ok(Json(CalendarToken { token }))
}

/// Write a fresh token for `user_id` (doc id = user id, so this overwrites any
/// previous one in place).
async fn mint_token(state: &AppState, user_id: &str) -> anyhow::Result<String> {
    let doc = CalendarTokenDoc { user_id: user_id.to_string(), token: new_token(), created_at: now() };
    let _: CalendarTokenDoc = state.db
        .fluent()
        .update()
        .in_col(CALENDAR_TOKENS)
        .document_id(user_id)
        .object(&doc)
        .execute()
        .await
        .context("failed to write calendar token")?;
    Ok(doc.token)
}

// ---- public ICS feed ----

async fn feed(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Response> {
    // Authorize by token. Looked up via the `token` field (doc id is user id).
    let matches: Vec<CalendarTokenDoc> = state.db
        .fluent()
        .select()
        .from(CALENDAR_TOKENS)
        .filter(|q| q.field("token").eq(&token))
        .obj()
        .query()
        .await
        .context("failed to look up calendar token")?;
    if matches.is_empty() {
        return Err(AppError::NotFound);
    }

    let mut events: Vec<EventDoc> = state.db
        .fluent()
        .select()
        .from(EVENTS)
        .order_by([(firestore::path!(EventDoc::date), FirestoreQueryDirection::Ascending)])
        .obj()
        .query()
        .await
        .context("failed to load events")?;
    events.sort_by(|a, b| a.date.cmp(&b.date));

    let body = build_ics(&events);
    Ok((
        [
            (header::CONTENT_TYPE, "text/calendar; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "inline; filename=\"baphomet-babes.ics\"".to_string(),
            ),
        ],
        body,
    )
        .into_response())
}

// ---- iCalendar serialization ----

/// Escape a text value per RFC 5545 §3.3.11.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\r', "")
        .replace('\n', "\\n")
}

/// "YYYY-MM-DD" -> "YYYYMMDD". Returns None for anything malformed so a bad row
/// is skipped rather than poisoning the whole feed.
fn ics_date(date: &str) -> Option<String> {
    let b = date.as_bytes();
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' && date[..4].bytes().chain(date[5..7].bytes()).chain(date[8..].bytes()).all(|c| c.is_ascii_digit()) {
        Some(format!("{}{}{}", &date[0..4], &date[5..7], &date[8..10]))
    } else {
        None
    }
}

/// Format unix seconds as a UTC iCalendar timestamp (YYYYMMDDTHHMMSSZ), using
/// Howard Hinnant's civil-from-days algorithm so we need no date crate.
fn utc_stamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}Z")
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

fn build_ics(events: &[EventDoc]) -> String {
    // RFC 5545 requires CRLF line endings.
    let mut out = String::new();
    let mut push = |line: String| {
        out.push_str(&line);
        out.push_str("\r\n");
    };

    push("BEGIN:VCALENDAR".into());
    push("VERSION:2.0".into());
    push("PRODID:-//Baphomet Babes//Movie Nights//EN".into());
    push("CALSCALE:GREGORIAN".into());
    push("METHOD:PUBLISH".into());
    push("X-WR-CALNAME:Baphomet Babes".into());
    push("X-WR-CALDESC:Movie nights & events".into());
    // Hint clients to re-poll twice a day.
    push("REFRESH-INTERVAL;VALUE=DURATION:PT12H".into());
    push("X-PUBLISHED-TTL:PT12H".into());

    for e in events {
        let Some(date) = &e.date else { continue };
        let Some(start) = ics_date(date) else { continue };
        let mut summary = e.title.clone();
        if e.event_type == "special" {
            summary = format!("★ {summary}");
        }
        let mut description = e.description.clone().unwrap_or_default();
        if let Some(url) = &e.poll_embed_url {
            if !description.is_empty() {
                description.push_str("\n\n");
            }
            description.push_str(&format!("Poll: {url}"));
        }

        push("BEGIN:VEVENT".into());
        push(format!("UID:{}@baphometbabes.com", e.id));
        push(format!("DTSTAMP:{}", utc_stamp(e.created_at)));
        // All-day event: a DATE-valued DTSTART with no DTEND is a single day.
        push(format!("DTSTART;VALUE=DATE:{start}"));
        push(format!("SUMMARY:{}", esc(&summary)));
        if !description.is_empty() {
            push(format!("DESCRIPTION:{}", esc(&description)));
        }
        push("END:VEVENT".into());
    }

    push("END:VCALENDAR".into());
    out
}
