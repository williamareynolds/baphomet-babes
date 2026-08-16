use crate::{
    AppState,
    auth::{require_admin, require_auth},
    error::{AppError, AppResult},
    models::{EventDoc, ProfileDoc, RsvpDoc},
};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{CreateEventRequest, Event, Rsvp, RsvpRequest, UpdateEventRequest};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const EVENTS: &str = "movie_nights";
const RSVPS: &str = "event_rsvps";
const PROFILES: &str = "profiles";

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Today's date as "YYYY-MM-DD" in UTC, for the RSVP-deadline cutoff. Good enough
/// for a date-granularity deadline; a member RSVPing within a few hours of
/// midnight Central could see a one-day skew, which the client-side check (local
/// date) smooths over for the common case.
fn today_utc() -> String {
    date_utc(0)
}

/// "YYYY-MM-DD" for the UTC day `offset_days` away from today.
fn date_utc(offset_days: i64) -> String {
    civil_from_days(now().div_euclid(86_400) + offset_days)
}

/// "YYYY-MM-DD" for a count of days since the Unix epoch, via Howard Hinnant's
/// days->civil algorithm so we need no date dependency.
fn civil_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route("/", get(list_events).post(create_event))
        .route("/{id}", put(update_event).delete(delete_event))
        .route("/{id}/rsvp", post(rsvp))
        .route("/{id}/rsvps", get(list_rsvps))
        .route("/poll-reminders", post(poll_reminders))
}

/// How many days ahead of a poll's deadline the closing-soon nudge goes out.
/// Two gives people a weekend day to act on it without the reminder landing so
/// early it gets forgotten.
const REMINDER_LEAD_DAYS: i64 = 2;

/// Whether this event is due a closing-soon nudge, given today's date and the
/// far edge of the reminder window (both "YYYY-MM-DD" — ISO dates sort
/// chronologically, so string comparison is date comparison).
///
/// Split out from the handler so the windowing rules are unit-testable without
/// a database.
pub fn needs_poll_reminder(e: &EventDoc, today: &str, cutoff: &str) -> bool {
    // A date means voting already resolved into a screening.
    if e.date.is_some() {
        return false;
    }
    // Nothing to vote on.
    if e.poll_embed_url.is_none() {
        return false;
    }
    // Already nudged for this deadline. Moving the deadline clears the stamp.
    if e.poll_reminder_sent_at != 0 {
        return false;
    }
    match e.poll_deadline.as_deref() {
        // Past deadlines are skipped rather than nudged late: a reminder to vote
        // in a poll that already closed is worse than no reminder.
        Some(d) => d >= today && d <= cutoff,
        None => false,
    }
}

/// Notification copy for an admin edit that changed *when* the movie plays, or
/// `None` if the edit left that alone.
///
/// Setting a date is the moment voting resolves into a plan, which is the thing
/// members are actually waiting on, and a later move is the thing that wrecks
/// their evening — so both announce. Clearing a date back to TBD stays quiet:
/// that's an admin correcting themselves mid-edit, not news anyone can act on.
pub fn date_change_notice(before: Option<&str>, updated: &EventDoc) -> Option<(String, String)> {
    let after = updated.date.as_deref()?;
    if before == Some(after) {
        return None;
    }

    let mut body = match before {
        Some(_) => format!("Moved to {after}."),
        None => format!("It's happening {after}."),
    };
    // The RSVP ask rides along, since a newly dated screening is exactly when
    // people are willing to commit.
    if let Some(deadline) = updated.rsvp_deadline.as_deref() {
        body.push_str(&format!(" RSVP by {deadline}."));
    }

    let heading = match before {
        Some(_) => format!("Movie night moved: {}", updated.title),
        None => format!("Date set: {}", updated.title),
    };
    Some((heading, body))
}

/// Compare two secrets without leaking their common prefix through timing.
fn secret_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Send closing-soon nudges for every poll inside the reminder window.
///
/// Called by Cloud Scheduler once a day, authorized by a shared secret rather
/// than a member session — there is no user behind it. Idempotent by the
/// `poll_reminder_sent_at` stamp, so running it twice in a day, or replaying it,
/// sends nothing the second time.
async fn poll_reminders(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let Some(expected) = state.reminder_secret.as_deref() else {
        return Err(AppError::Forbidden);
    };
    let provided = headers
        .get("X-Reminder-Secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !secret_eq(provided, expected) {
        return Err(AppError::Forbidden);
    }

    let today = today_utc();
    let cutoff = date_utc(REMINDER_LEAD_DAYS);

    let events: Vec<EventDoc> = state.db
        .fluent()
        .select()
        .from(EVENTS)
        .obj()
        .query()
        .await
        .context("failed to load events for poll reminders")?;

    let due: Vec<EventDoc> = events
        .into_iter()
        .filter(|e| needs_poll_reminder(e, &today, &cutoff))
        .collect();

    let mut reminded = 0usize;
    for e in due {
        let deadline = e.poll_deadline.clone().unwrap_or_default();
        let body = if deadline == today {
            format!("Voting closes today. Get your ranking in for {}.", e.title)
        } else {
            format!("Voting closes {deadline}. Get your ranking in for {}.", e.title)
        };

        if let Err(err) = crate::routes::notifications::dispatch(
            &state,
            shared::CHANNEL_MOVIE_NIGHT,
            "Last call to vote",
            &body,
            Some("/movie-nights"),
            None,
        )
        .await
        {
            // Leave the stamp unset so the next daily run retries this one.
            tracing::warn!("poll reminder dispatch failed for {}: {err:#}", e.id);
            continue;
        }

        let stamped = EventDoc { poll_reminder_sent_at: now(), ..e };
        let _: EventDoc = state.db
            .fluent()
            .update()
            .in_col(EVENTS)
            .document_id(&stamped.id)
            .object(&stamped)
            .execute()
            .await
            .context("failed to stamp poll reminder")?;
        reminded += 1;
    }

    tracing::info!("poll reminders sent={reminded} today={today} cutoff={cutoff}");
    Ok(Json(serde_json::json!({ "reminded": reminded })))
}

fn doc_to_event(d: EventDoc, rsvp_count: i64, my_rsvp: bool) -> Event {
    Event {
        id: d.id,
        event_type: d.event_type,
        title: d.title,
        date: d.date,
        description: d.description,
        poll_embed_url: d.poll_embed_url,
        poster_url: d.poster_url,
        rsvp_deadline: d.rsvp_deadline,
        poll_deadline: d.poll_deadline,
        rsvp_count,
        my_rsvp,
    }
}

/// Resolve a member's display label (display name preferred, username fallback)
/// for denormalizing onto an RSVP, mirroring the chat author convention.
async fn author_label(state: &AppState, user_id: &str) -> AppResult<String> {
    let profile: Option<ProfileDoc> = state
        .db
        .fluent()
        .select()
        .by_id_in(PROFILES)
        .obj()
        .one(user_id)
        .await
        .context("failed to load member profile")?;
    Ok(profile
        .map(|p| p.display_name.filter(|s| !s.is_empty()).unwrap_or(p.username))
        .unwrap_or_else(|| "Someone".to_string()))
}

async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Event>>> {
    let claims = require_auth(&state, &headers).await?;

    let mut docs: Vec<EventDoc> = state.db
        .fluent()
        .select()
        .from(EVENTS)
        .obj()
        .query()
        .await
        .context("failed to list events")?;

    // One scan of the RSVP collection (small at our scale) yields both the
    // per-event going counts and the caller's own RSVP set — no per-event query
    // and no stored counter to drift.
    let rsvps: Vec<RsvpDoc> = state.db
        .fluent()
        .select()
        .from(RSVPS)
        .obj()
        .query()
        .await
        .context("failed to list rsvps")?;

    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut mine: HashSet<String> = HashSet::new();
    for r in &rsvps {
        *counts.entry(r.event_id.clone()).or_insert(0) += 1;
        if r.user_id == claims.sub {
            mine.insert(r.event_id.clone());
        }
    }

    docs.sort_by(|a, b| a.date.cmp(&b.date));
    let out = docs
        .into_iter()
        .map(|d| {
            let count = counts.get(&d.id).copied().unwrap_or(0);
            let my = mine.contains(&d.id);
            doc_to_event(d, count, my)
        })
        .collect();
    Ok(Json(out))
}

async fn create_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateEventRequest>,
) -> AppResult<Json<Event>> {
    require_admin(&state, &headers).await?;

    if req.event_type != "main" && req.event_type != "special" {
        return Err(AppError::BadRequest("event_type must be 'main' or 'special'".into()));
    }

    let id = Uuid::new_v4().to_string();

    let doc = EventDoc {
        id: id.clone(),
        event_type: req.event_type.clone(),
        title: req.title.clone(),
        date: req.date.clone().filter(|d| !d.is_empty()),
        description: req.description.clone(),
        poll_embed_url: req.poll_embed_url.clone(),
        poster_url: req.poster_url.clone(),
        rsvp_deadline: req.rsvp_deadline.clone().filter(|d| !d.is_empty()),
        poll_deadline: req.poll_deadline.clone().filter(|d| !d.is_empty()),
        poll_reminder_sent_at: 0,
        created_at: now(),
    };

    let _: EventDoc = state.db
        .fluent()
        .insert()
        .into(EVENTS)
        .document_id(&id)
        .object(&doc)
        .execute()
        .await
        .context("failed to create event")?;

    // Notify the movie-night channel (persist + best-effort push).
    let when = doc.date.clone().unwrap_or_else(|| "Date TBD".to_string());
    let body = match &doc.description {
        Some(d) if !d.is_empty() => format!("{} — {}", when, d),
        _ => when,
    };
    if let Err(e) = crate::routes::notifications::dispatch(
        &state,
        shared::CHANNEL_MOVIE_NIGHT,
        &format!("New movie night: {}", doc.title),
        &body,
        Some("/movie-nights"),
        None,
    ).await {
        tracing::warn!("event notification failed: {e:#}");
    }

    Ok(Json(doc_to_event(doc, 0, false)))
}

async fn update_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateEventRequest>,
) -> AppResult<Json<Event>> {
    require_admin(&state, &headers).await?;

    let existing: Option<EventDoc> = state.db
        .fluent()
        .select()
        .by_id_in(EVENTS)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch event")?;

    let existing = existing.ok_or(AppError::NotFound)?;
    // Captured before the merge below consumes `existing`.
    let before_date = existing.date.clone();

    // Same clear/set/keep semantics as the other optional dates.
    let new_poll_deadline = match &req.poll_deadline {
        Some(d) if d.is_empty() => None,
        Some(d) => Some(d.clone()),
        None => existing.poll_deadline.clone(),
    };
    // Moving the deadline re-arms the reminder: an admin who pushes voting out
    // by a week means to nudge against the new date, not to have the earlier
    // send suppress it forever.
    let poll_reminder_sent_at = if new_poll_deadline == existing.poll_deadline {
        existing.poll_reminder_sent_at
    } else {
        0
    };

    let updated = EventDoc {
        id: existing.id.clone(),
        event_type: req.event_type.unwrap_or(existing.event_type),
        title: req.title.unwrap_or(existing.title),
        // Some("") clears the date, Some(d) sets it, None leaves it unchanged.
        date: match req.date {
            Some(d) if d.is_empty() => None,
            Some(d) => Some(d),
            None => existing.date,
        },
        description: req.description.or(existing.description),
        poll_embed_url: req.poll_embed_url.or(existing.poll_embed_url),
        poster_url: req.poster_url.or(existing.poster_url),
        // Same Some("")-clears / Some(d)-sets / None-keeps semantics as date.
        rsvp_deadline: match req.rsvp_deadline {
            Some(d) if d.is_empty() => None,
            Some(d) => Some(d),
            None => existing.rsvp_deadline,
        },
        poll_deadline: new_poll_deadline,
        poll_reminder_sent_at,
        created_at: existing.created_at,
    };

    let _: EventDoc = state.db
        .fluent()
        .update()
        .in_col(EVENTS)
        .document_id(&id)
        .object(&updated)
        .execute()
        .await
        .context("failed to update event")?;

    // Best-effort, same as the create path: a mail or push problem must not fail
    // the edit the admin just made.
    if let Some((title, body)) = date_change_notice(before_date.as_deref(), &updated) {
        if let Err(e) = crate::routes::notifications::dispatch(
            &state,
            shared::CHANNEL_MOVIE_NIGHT,
            &title,
            &body,
            Some("/movie-nights"),
            None,
        )
        .await
        {
            tracing::warn!("event date notification failed: {e:#}");
        }
    }

    let count = count_rsvps(&state, &id).await?;
    Ok(Json(doc_to_event(updated, count, false)))
}

async fn delete_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<()> {
    require_admin(&state, &headers).await?;

    let exists: Option<EventDoc> = state.db
        .fluent()
        .select()
        .by_id_in(EVENTS)
        .obj()
        .one(&id)
        .await
        .context("failed to check event")?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    state.db
        .fluent()
        .delete()
        .from(EVENTS)
        .document_id(&id)
        .execute()
        .await
        .context("failed to delete event")?;

    // Best-effort cleanup of the event's RSVPs so they don't linger orphaned.
    for r in event_rsvps(&state, &id).await.unwrap_or_default() {
        let _ = state.db
            .fluent()
            .delete()
            .from(RSVPS)
            .document_id(&r.id)
            .execute()
            .await;
    }

    Ok(())
}

/// All "going" RSVP docs for one event.
async fn event_rsvps(state: &AppState, event_id: &str) -> AppResult<Vec<RsvpDoc>> {
    let rsvps: Vec<RsvpDoc> = state.db
        .fluent()
        .select()
        .from(RSVPS)
        .filter(|q| q.field("event_id").eq(event_id))
        .obj()
        .query()
        .await
        .context("failed to query rsvps")?;
    Ok(rsvps)
}

async fn count_rsvps(state: &AppState, event_id: &str) -> AppResult<i64> {
    Ok(event_rsvps(state, event_id).await?.len() as i64)
}

/// Member RSVPs (or cancels) for an event. Idempotent: a deterministic doc id
/// means re-RSVPing is a no-op and double-cancel is harmless.
async fn rsvp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RsvpRequest>,
) -> AppResult<Json<Event>> {
    let claims = require_auth(&state, &headers).await?;

    let event: Option<EventDoc> = state.db
        .fluent()
        .select()
        .by_id_in(EVENTS)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch event")?;
    let event = event.ok_or(AppError::NotFound)?;

    // Enforce the deadline server-side (the UI also disables past it).
    if let Some(deadline) = event.rsvp_deadline.as_deref() {
        if today_utc().as_str() > deadline {
            return Err(AppError::BadRequest("the RSVP deadline has passed".into()));
        }
    }

    let doc_id = format!("{id}_{}", claims.sub);

    if req.going {
        let author = author_label(&state, &claims.sub).await?;
        let doc = RsvpDoc {
            id: doc_id.clone(),
            event_id: id.clone(),
            user_id: claims.sub.clone(),
            author,
            created_at: now(),
        };
        let _: RsvpDoc = state.db
            .fluent()
            .update() // upsert: creates-or-overwrites the doc at this id
            .in_col(RSVPS)
            .document_id(&doc_id)
            .object(&doc)
            .execute()
            .await
            .context("failed to save rsvp")?;
    } else {
        let _ = state.db
            .fluent()
            .delete()
            .from(RSVPS)
            .document_id(&doc_id)
            .execute()
            .await;
    }

    let count = count_rsvps(&state, &id).await?;
    Ok(Json(doc_to_event(event, count, req.going)))
}

/// Admin-only: who's RSVP'd "going" for an event (members only see the count).
async fn list_rsvps(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<Rsvp>>> {
    require_admin(&state, &headers).await?;

    let mut rsvps = event_rsvps(&state, &id).await?;
    rsvps.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let out = rsvps
        .into_iter()
        .map(|r| Rsvp { user_id: r.user_id, author: r.author, created_at: r.created_at })
        .collect();
    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An event mid-vote: poll set, no date yet, never nudged.
    fn voting(deadline: Option<&str>) -> EventDoc {
        EventDoc {
            id: "e1".into(),
            event_type: "main".into(),
            title: "The Wicker Man".into(),
            date: None,
            description: None,
            poll_embed_url: Some("https://rcv123.org/p/1".into()),
            poster_url: None,
            rsvp_deadline: None,
            poll_deadline: deadline.map(String::from),
            poll_reminder_sent_at: 0,
            created_at: 0,
        }
    }

    const TODAY: &str = "2026-08-07";
    const CUTOFF: &str = "2026-08-09";

    #[test]
    fn nudges_a_poll_closing_inside_the_window() {
        assert!(needs_poll_reminder(&voting(Some("2026-08-08")), TODAY, CUTOFF));
    }

    #[test]
    fn window_includes_both_ends() {
        assert!(needs_poll_reminder(&voting(Some(TODAY)), TODAY, CUTOFF));
        assert!(needs_poll_reminder(&voting(Some(CUTOFF)), TODAY, CUTOFF));
    }

    #[test]
    fn skips_deadlines_outside_the_window() {
        // Too far out — it'll qualify on a later daily run.
        assert!(!needs_poll_reminder(&voting(Some("2026-08-10")), TODAY, CUTOFF));
        // Already closed: a late nudge is worse than none.
        assert!(!needs_poll_reminder(&voting(Some("2026-08-06")), TODAY, CUTOFF));
    }

    #[test]
    fn skips_events_with_no_deadline() {
        assert!(!needs_poll_reminder(&voting(None), TODAY, CUTOFF));
    }

    #[test]
    fn skips_scheduled_events() {
        // A date means voting resolved into a screening; the poll is moot.
        let mut e = voting(Some("2026-08-08"));
        e.date = Some("2026-09-01".into());
        assert!(!needs_poll_reminder(&e, TODAY, CUTOFF));
    }

    #[test]
    fn skips_events_with_no_poll() {
        let mut e = voting(Some("2026-08-08"));
        e.poll_embed_url = None;
        assert!(!needs_poll_reminder(&e, TODAY, CUTOFF));
    }

    #[test]
    fn skips_events_already_nudged() {
        let mut e = voting(Some("2026-08-08"));
        e.poll_reminder_sent_at = 1_754_000_000;
        assert!(!needs_poll_reminder(&e, TODAY, CUTOFF));
    }

    /// The same event once a date landed on it.
    fn dated(date: &str) -> EventDoc {
        EventDoc { date: Some(date.into()), ..voting(Some("2026-08-08")) }
    }

    #[test]
    fn announces_a_newly_set_date() {
        let (title, body) = date_change_notice(None, &dated("2026-09-01")).unwrap();
        assert_eq!(title, "Date set: The Wicker Man");
        assert_eq!(body, "It's happening 2026-09-01.");
    }

    #[test]
    fn announces_a_moved_date() {
        let (title, body) =
            date_change_notice(Some("2026-09-01"), &dated("2026-09-08")).unwrap();
        assert_eq!(title, "Movie night moved: The Wicker Man");
        assert_eq!(body, "Moved to 2026-09-08.");
    }

    #[test]
    fn rsvp_deadline_rides_along_when_set() {
        let mut e = dated("2026-09-01");
        e.rsvp_deadline = Some("2026-08-30".into());
        let (_, body) = date_change_notice(None, &e).unwrap();
        assert_eq!(body, "It's happening 2026-09-01. RSVP by 2026-08-30.");
    }

    #[test]
    fn stays_quiet_when_the_date_did_not_change() {
        // Editing the title or poster must not re-announce the same date.
        assert!(date_change_notice(Some("2026-09-01"), &dated("2026-09-01")).is_none());
        // Still undated after the edit — nothing to say.
        assert!(date_change_notice(None, &voting(Some("2026-08-08"))).is_none());
        // Cleared back to TBD: an admin fixing themselves, not news.
        assert!(date_change_notice(Some("2026-09-01"), &voting(None)).is_none());
    }

    #[test]
    fn secret_compare_matches_only_exact() {
        assert!(secret_eq("s3cret", "s3cret"));
        assert!(!secret_eq("s3cret", "s3crea"));
        assert!(!secret_eq("s3cret", "s3cre"));   // prefix
        assert!(!secret_eq("s3cret", "s3cretx")); // extension
        assert!(!secret_eq("", "s3cret"));
    }

    #[test]
    fn date_offsets_roll_over_month_and_year_boundaries() {
        // Anchored on fixed day counts rather than "now", so this can't drift.
        assert_eq!(civil_from_days(0), "1970-01-01");
        let today = 20_672; // 2026-08-07
        assert_eq!(civil_from_days(today), "2026-08-07");
        assert_eq!(civil_from_days(today + REMINDER_LEAD_DAYS), "2026-08-09");
        assert_eq!(civil_from_days(today + 25), "2026-09-01"); // month end
        assert_eq!(civil_from_days(today + 146), "2026-12-31");
        assert_eq!(civil_from_days(today + 147), "2027-01-01"); // year end
        // Leap day: 2028 is a leap year.
        assert_eq!(civil_from_days(21_243), "2028-02-29");
        assert_eq!(civil_from_days(21_244), "2028-03-01");
    }
}
