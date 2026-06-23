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
/// date) smooths over for the common case. Uses Howard Hinnant's days->civil
/// algorithm so we need no date dependency.
fn today_utc() -> String {
    let days = now().div_euclid(86_400);
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
