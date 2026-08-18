//! Subscribable calendar (iCalendar / .ics) feed.
//!
//! Each member has a secret, revocable token (`/calendar/me`). The feed itself
//! lives at a public capability URL — `/calendar/{token}/baphomet-babes.ics` —
//! because Google/iCloud/Outlook fetch it anonymously (no auth header, no App
//! Check token). The token in the path is the only credential; the App Check
//! middleware exempts paths ending in `.ics` so the feed stays reachable.

use crate::{
    AppState,
    auth::{require_auth, require_superadmin},
    error::{AppError, AppResult},
    models::{CalendarTokenDoc, EventDoc, ExternalCalendarDoc, ProfileDoc},
};
use anyhow::Context;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};

use shared::{CalendarToken, CreateExternalCalendarRequest, ExternalCalendarLink};
use uuid::Uuid;

const EVENTS: &str = "movie_nights";
const CALENDAR_TOKENS: &str = "calendar_tokens";
const EXTERNAL_CALENDARS: &str = "external_calendars";
const PROFILES: &str = "profiles";

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{delete, get, post};
    axum::Router::new()
        .route("/me", get(my_token))
        .route("/me/regenerate", post(regenerate_token))
        .route("/external", get(list_external).post(create_external))
        .route("/external/{id}", delete(revoke_external))
        // Declared last: a literal segment would otherwise be shadowed by this
        // wildcard, and "external" is a plausible-looking token.
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

// ---- external (non-member) links, superadmin only ----

/// Every issued non-member link, newest first.
async fn list_external(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<ExternalCalendarLink>>> {
    require_superadmin(&state, &headers).await?;

    let mut docs: Vec<ExternalCalendarDoc> = state.db
        .fluent()
        .select()
        .from(EXTERNAL_CALENDARS)
        .obj()
        .query()
        .await
        .context("failed to list external calendar links")?;
    docs.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(Json(docs.into_iter().map(doc_to_link).collect()))
}

fn doc_to_link(d: ExternalCalendarDoc) -> ExternalCalendarLink {
    ExternalCalendarLink {
        id: d.id,
        name: d.name,
        phone: d.phone,
        token: d.token,
        created_at: d.created_at,
        created_by: d.created_by,
    }
}

/// Issue a link for someone without an account.
async fn create_external(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateExternalCalendarRequest>,
) -> AppResult<Json<ExternalCalendarLink>> {
    let claims = require_superadmin(&state, &headers).await?;

    shared::validate_external_calendar(&req.name, &req.phone)
        .map_err(AppError::BadRequest)?;

    let id = Uuid::new_v4().to_string();
    let doc = ExternalCalendarDoc {
        id: id.clone(),
        name: req.name.trim().to_string(),
        phone: req.phone.trim().to_string(),
        token: new_token(),
        created_at: now(),
        created_by: issuer_label(&state, &claims.sub).await,
    };

    let _: ExternalCalendarDoc = state.db
        .fluent()
        .insert()
        .into(EXTERNAL_CALENDARS)
        .document_id(&id)
        .object(&doc)
        .execute()
        .await
        .context("failed to create external calendar link")?;

    Ok(Json(doc_to_link(doc)))
}

/// Who issued a link, for the admin list. Falls back rather than failing the
/// whole request — a missing profile shouldn't block issuing a link.
async fn issuer_label(state: &AppState, user_id: &str) -> String {
    let profile: Option<ProfileDoc> = state
        .db
        .fluent()
        .select()
        .by_id_in(PROFILES)
        .obj()
        .one(user_id)
        .await
        .ok()
        .flatten();
    profile
        .map(|p| p.display_name.filter(|s| !s.is_empty()).unwrap_or(p.username))
        .unwrap_or_else(|| "an admin".to_string())
}

/// Revoke by deleting: the URL 404s on the next fetch, and we stop holding a
/// non-member's contact details.
async fn revoke_external(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<()> {
    require_superadmin(&state, &headers).await?;

    let existing: Option<ExternalCalendarDoc> = state.db
        .fluent()
        .select()
        .by_id_in(EXTERNAL_CALENDARS)
        .obj()
        .one(&id)
        .await
        .context("failed to load external calendar link")?;
    if existing.is_none() {
        return Err(AppError::NotFound);
    }

    state.db
        .fluent()
        .delete()
        .from(EXTERNAL_CALENDARS)
        .document_id(&id)
        .execute()
        .await
        .context("failed to revoke external calendar link")?;
    Ok(())
}

// ---- public ICS feed ----

/// Whether `token` is a live credential — a member's own token, or a link
/// issued to a non-member. Both feed the same calendar.
async fn token_is_valid(state: &AppState, token: &str) -> AppResult<bool> {
    let members: Vec<CalendarTokenDoc> = state.db
        .fluent()
        .select()
        .from(CALENDAR_TOKENS)
        .filter(|q| q.field("token").eq(token))
        .obj()
        .query()
        .await
        .context("failed to look up calendar token")?;
    if !members.is_empty() {
        return Ok(true);
    }

    let external: Vec<ExternalCalendarDoc> = state.db
        .fluent()
        .select()
        .from(EXTERNAL_CALENDARS)
        .filter(|q| q.field("token").eq(token))
        .obj()
        .query()
        .await
        .context("failed to look up external calendar link")?;
    Ok(!external.is_empty())
}

async fn feed(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Response> {
    // Authorize by token. Looked up via the `token` field (doc id is user id
    // for members, a link id for non-members).
    if !token_is_valid(&state, &token).await? {
        return Err(AppError::NotFound);
    }

    // Deliberately unordered at the query level: Firestore drops documents that
    // don't carry the order-by field, so ordering by `date` here silently
    // excluded every undated event — precisely the ones being voted on, whose
    // deadlines we now publish. Sorting in memory is free at our scale.
    let mut events: Vec<EventDoc> = state.db
        .fluent()
        .select()
        .from(EVENTS)
        .obj()
        .query()
        .await
        .context("failed to load events")?;
    events.sort_by(|a, b| a.date.cmp(&b.date));

    let body = build_ics(&events, &state.public_base_url);
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

/// A DISPLAY alarm at 9am on the morning of an all-day entry.
///
/// `RELATED=START` with a DATE-valued DTSTART means "9 hours after midnight
/// local", i.e. breakfast, not 3am. Worth knowing: Google Calendar strips
/// VALARM from *subscribed* feeds, so this fires for Apple Calendar
/// subscribers and is inert for Google ones — the visible all-day entry is the
/// part that works everywhere, and push/email remain the reliable nudge.
fn alarm(description: &str) -> Vec<String> {
    vec![
        "BEGIN:VALARM".into(),
        "ACTION:DISPLAY".into(),
        format!("DESCRIPTION:{}", esc(description)),
        "TRIGGER;RELATED=START:PT9H".into(),
        "END:VALARM".into(),
    ]
}

fn build_ics(events: &[EventDoc], base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
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
        // Deadlines get their own all-day entries, so the dates that need
        // someone to *act* are visible in a calendar rather than living only in
        // the app. An event being voted on has no date yet and would otherwise
        // produce nothing at all here.
        if e.date.is_none() {
            if let Some(start) = e.poll_deadline.as_deref().and_then(ics_date) {
                let mut description = format!("Voting closes today for {}.", e.title);
                if let Some(url) = &e.poll_embed_url {
                    description.push_str(&format!("\n\nPoll: {url}"));
                }
                description.push_str(&format!("\n\n{base}/movie-nights"));

                push("BEGIN:VEVENT".into());
                push(format!("UID:{}-poll@baphometbabes.com", e.id));
                push(format!("DTSTAMP:{}", utc_stamp(e.created_at)));
                push(format!("DTSTART;VALUE=DATE:{start}"));
                push(format!("SUMMARY:{}", esc(&format!("Voting closes: {}", e.title))));
                push(format!("DESCRIPTION:{}", esc(&description)));
                for line in alarm(&format!("Last day to vote for {}", e.title)) {
                    push(line);
                }
                push("END:VEVENT".into());
            }
        }

        if let Some(start) = e.rsvp_deadline.as_deref().and_then(ics_date) {
            let description = format!(
                "Last day to RSVP for {}.\n\n{base}/movie-nights",
                e.title
            );
            push("BEGIN:VEVENT".into());
            push(format!("UID:{}-rsvp@baphometbabes.com", e.id));
            push(format!("DTSTAMP:{}", utc_stamp(e.created_at)));
            push(format!("DTSTART;VALUE=DATE:{start}"));
            push(format!("SUMMARY:{}", esc(&format!("RSVP by today: {}", e.title))));
            push(format!("DESCRIPTION:{}", esc(&description)));
            for line in alarm(&format!("Last day to RSVP for {}", e.title)) {
                push(line);
            }
            push("END:VEVENT".into());
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://baphometbabes.com";

    fn event() -> EventDoc {
        EventDoc {
            id: "e1".into(),
            event_type: "main".into(),
            title: "The Wicker Man".into(),
            date: None,
            description: None,
            poll_embed_url: None,
            poster_url: None,
            rsvp_deadline: None,
            poll_deadline: None,
            poll_reminder_sent_at: 0,
            created_at: 0,
        }
    }

    #[test]
    fn an_event_being_voted_on_shows_its_voting_deadline() {
        let e = EventDoc {
            poll_deadline: Some("2026-09-01".into()),
            poll_embed_url: Some("https://rcv123.org/p/1".into()),
            ..event()
        };
        let ics = build_ics(&[e], BASE);

        assert!(ics.contains("UID:e1-poll@baphometbabes.com"));
        assert!(ics.contains("DTSTART;VALUE=DATE:20260901"));
        assert!(ics.contains("SUMMARY:Voting closes: The Wicker Man"));
        assert!(ics.contains("https://rcv123.org/p/1"));
        // 9am on the day, not midnight.
        assert!(ics.contains("TRIGGER;RELATED=START:PT9H"));
    }

    #[test]
    fn a_scheduled_event_drops_the_voting_deadline() {
        // Voting resolved into a screening; a "voting closes" entry would be
        // stale noise sitting in everyone's calendar.
        let e = EventDoc {
            date: Some("2026-09-15".into()),
            poll_deadline: Some("2026-09-01".into()),
            ..event()
        };
        let ics = build_ics(&[e], BASE);

        assert!(!ics.contains("-poll@baphometbabes.com"));
        assert!(ics.contains("DTSTART;VALUE=DATE:20260915"));
    }

    #[test]
    fn an_rsvp_deadline_gets_its_own_entry() {
        let e = EventDoc {
            date: Some("2026-09-15".into()),
            rsvp_deadline: Some("2026-09-10".into()),
            ..event()
        };
        let ics = build_ics(&[e], BASE);

        assert!(ics.contains("UID:e1-rsvp@baphometbabes.com"));
        assert!(ics.contains("DTSTART;VALUE=DATE:20260910"));
        assert!(ics.contains("SUMMARY:RSVP by today: The Wicker Man"));
        assert!(ics.contains("https://baphometbabes.com/movie-nights"));
    }

    #[test]
    fn deadline_entries_are_separate_from_the_screening() {
        let e = EventDoc {
            date: Some("2026-09-15".into()),
            rsvp_deadline: Some("2026-09-10".into()),
            ..event()
        };
        let ics = build_ics(&[e], BASE);
        assert_eq!(ics.matches("BEGIN:VEVENT").count(), 2);
        assert_eq!(ics.matches("END:VEVENT").count(), 2);
    }

    #[test]
    fn a_malformed_deadline_is_skipped_not_emitted_raw() {
        let e = EventDoc {
            poll_deadline: Some("next tuesday".into()),
            rsvp_deadline: Some("".into()),
            ..event()
        };
        let ics = build_ics(&[e], BASE);
        assert!(!ics.contains("BEGIN:VEVENT"));
        assert!(!ics.contains("next tuesday"));
    }

    #[test]
    fn text_values_are_escaped() {
        let e = EventDoc {
            title: "Alien; or, Commas, Everywhere".into(),
            poll_deadline: Some("2026-09-01".into()),
            ..event()
        };
        let ics = build_ics(&[e], BASE);
        assert!(ics.contains("SUMMARY:Voting closes: Alien\\; or\\, Commas\\, Everywhere"));
    }

    #[test]
    fn every_line_ends_crlf_as_rfc5545_requires() {
        let e = EventDoc {
            date: Some("2026-09-15".into()),
            poll_deadline: Some("2026-09-01".into()),
            rsvp_deadline: Some("2026-09-10".into()),
            ..event()
        };
        let ics = build_ics(&[e], BASE);
        for line in ics.split("\r\n").filter(|l| !l.is_empty()) {
            assert!(!line.contains('\n'), "bare LF in {line:?}");
        }
        assert!(ics.ends_with("END:VCALENDAR\r\n"));
    }
}
