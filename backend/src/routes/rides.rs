//! Mountain bike rides: any member can post that they're heading to a trail,
//! and others join with one tap. Modeled on the events/RSVP pair, with two
//! differences: creation is member-level (not admin), and attendee names are
//! visible to every member — knowing who you're riding with is the point.

use crate::{
    AppState,
    auth::require_auth,
    error::{AppError, AppResult},
    models::{ProfileDoc, RideAttendeeDoc, RideDoc},
};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{CreateRideRequest, RIDE_LOCATIONS, Ride, RsvpRequest};
use std::collections::HashMap;
use uuid::Uuid;

const RIDES: &str = "rides";
const ATTENDEES: &str = "ride_attendees";
const PROFILES: &str = "profiles";

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{delete, get, post};
    axum::Router::new()
        .route("/", get(list_rides).post(create_ride))
        .route("/{id}", delete(delete_ride))
        .route("/{id}/attend", post(attend))
}

/// "2026-07-04T09:00" -> "2026-07-04 09:00" for notification bodies. The client
/// renders the raw value properly; this just keeps push text readable.
fn pretty_dt(dt: &str) -> String {
    dt.replacen('T', " ", 1)
}

/// Validate a "YYYY-MM-DDTHH:MM" naive local datetime (what
/// `<input type="datetime-local">` produces).
fn valid_datetime(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 16
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && s.chars().enumerate().all(|(i, c)| matches!(i, 4 | 7 | 10 | 13) || c.is_ascii_digit())
}

fn doc_to_ride(d: RideDoc, attendees: Vec<String>, my_attending: bool) -> Ride {
    Ride {
        id: d.id,
        location: d.location,
        start_at: d.start_at,
        end_at: d.end_at,
        created_by: d.created_by,
        created_by_name: d.created_by_name,
        created_at: d.created_at,
        attendees,
        my_attending,
    }
}

/// Same display-label resolution as events/chat: display name, else username.
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

/// All attendee docs for one ride, in join order.
async fn ride_attendees(state: &AppState, ride_id: &str) -> AppResult<Vec<RideAttendeeDoc>> {
    let mut list: Vec<RideAttendeeDoc> = state.db
        .fluent()
        .select()
        .from(ATTENDEES)
        .filter(|q| q.field("ride_id").eq(ride_id))
        .obj()
        .query()
        .await
        .context("failed to query ride attendees")?;
    list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(list)
}

async fn load_ride(state: &AppState, id: &str) -> AppResult<RideDoc> {
    let ride: Option<RideDoc> = state.db
        .fluent()
        .select()
        .by_id_in(RIDES)
        .obj()
        .one(id)
        .await
        .context("failed to fetch ride")?;
    ride.ok_or(AppError::NotFound)
}

async fn list_rides(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Ride>>> {
    let claims = require_auth(&state, &headers).await?;

    let mut docs: Vec<RideDoc> = state.db
        .fluent()
        .select()
        .from(RIDES)
        .obj()
        .query()
        .await
        .context("failed to list rides")?;

    // One scan of the attendee collection (small at our scale) yields names and
    // the caller's own status for every ride — no per-ride query.
    let mut attendees: Vec<RideAttendeeDoc> = state.db
        .fluent()
        .select()
        .from(ATTENDEES)
        .obj()
        .query()
        .await
        .context("failed to list ride attendees")?;
    attendees.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let mut names: HashMap<String, Vec<String>> = HashMap::new();
    let mut mine: std::collections::HashSet<String> = std::collections::HashSet::new();
    for a in attendees {
        if a.user_id == claims.sub {
            mine.insert(a.ride_id.clone());
        }
        names.entry(a.ride_id).or_default().push(a.author);
    }

    docs.sort_by(|a, b| a.start_at.cmp(&b.start_at));
    let out = docs
        .into_iter()
        .map(|d| {
            let list = names.remove(&d.id).unwrap_or_default();
            let my = mine.contains(&d.id);
            doc_to_ride(d, list, my)
        })
        .collect();
    Ok(Json(out))
}

async fn create_ride(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateRideRequest>,
) -> AppResult<Json<Ride>> {
    let claims = require_auth(&state, &headers).await?;

    if !RIDE_LOCATIONS.contains(&req.location.as_str()) {
        return Err(AppError::BadRequest("unknown riding location".into()));
    }
    if !valid_datetime(&req.start_at) || !valid_datetime(&req.end_at) {
        return Err(AppError::BadRequest("start and end must be YYYY-MM-DDTHH:MM".into()));
    }
    if req.end_at <= req.start_at {
        return Err(AppError::BadRequest("the ride must end after it starts".into()));
    }

    let id = Uuid::new_v4().to_string();
    let author = author_label(&state, &claims.sub).await?;

    let doc = RideDoc {
        id: id.clone(),
        location: req.location.clone(),
        start_at: req.start_at.clone(),
        end_at: req.end_at.clone(),
        created_by: claims.sub.clone(),
        created_by_name: author.clone(),
        created_at: now(),
    };
    let _: RideDoc = state.db
        .fluent()
        .insert()
        .into(RIDES)
        .document_id(&id)
        .object(&doc)
        .execute()
        .await
        .context("failed to create ride")?;

    // The creator is automatically going.
    let att_id = format!("{id}_{}", claims.sub);
    let att = RideAttendeeDoc {
        id: att_id.clone(),
        ride_id: id.clone(),
        user_id: claims.sub.clone(),
        author: author.clone(),
        created_at: now(),
    };
    let _: RideAttendeeDoc = state.db
        .fluent()
        .insert()
        .into(ATTENDEES)
        .document_id(&att_id)
        .object(&att)
        .execute()
        .await
        .context("failed to save creator attendance")?;

    // Notify the mountain-bike channel (persist + best-effort push).
    let body = format!("{author} · {} – {}", pretty_dt(&doc.start_at), pretty_dt(&doc.end_at));
    if let Err(e) = crate::routes::notifications::dispatch(
        &state,
        shared::CHANNEL_MOUNTAIN_BIKE,
        &format!("New ride: {}", doc.location),
        &body,
        Some("/rides"),
        Some(&claims.sub),
    ).await {
        tracing::warn!("ride notification failed: {e:#}");
    }

    Ok(Json(doc_to_ride(doc, vec![author], true)))
}

/// The creator (or an admin) can take a ride down.
async fn delete_ride(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<()> {
    let claims = require_auth(&state, &headers).await?;
    let ride = load_ride(&state, &id).await?;

    let is_admin = claims.role == "admin" || claims.role == "superadmin";
    if ride.created_by != claims.sub && !is_admin {
        return Err(AppError::Forbidden);
    }

    state.db
        .fluent()
        .delete()
        .from(RIDES)
        .document_id(&id)
        .execute()
        .await
        .context("failed to delete ride")?;

    // Best-effort cleanup of the ride's attendance records.
    for a in ride_attendees(&state, &id).await.unwrap_or_default() {
        let _ = state.db
            .fluent()
            .delete()
            .from(ATTENDEES)
            .document_id(&a.id)
            .execute()
            .await;
    }

    Ok(())
}

/// Member joins (or bails on) a ride. Idempotent like event RSVPs. Joining
/// pushes a heads-up to everyone already going — they signed up to ride
/// together, so this bypasses channel prefs (and skips the inbox).
async fn attend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RsvpRequest>,
) -> AppResult<Json<Ride>> {
    let claims = require_auth(&state, &headers).await?;
    let ride = load_ride(&state, &id).await?;

    let doc_id = format!("{id}_{}", claims.sub);
    let existing: Option<RideAttendeeDoc> = state.db
        .fluent()
        .select()
        .by_id_in(ATTENDEES)
        .obj()
        .one(&doc_id)
        .await
        .context("failed to check attendance")?;

    if req.going {
        if existing.is_none() {
            let author = author_label(&state, &claims.sub).await?;
            let doc = RideAttendeeDoc {
                id: doc_id.clone(),
                ride_id: id.clone(),
                user_id: claims.sub.clone(),
                author: author.clone(),
                created_at: now(),
            };
            let _: RideAttendeeDoc = state.db
                .fluent()
                .update() // upsert
                .in_col(ATTENDEES)
                .document_id(&doc_id)
                .object(&doc)
                .execute()
                .await
                .context("failed to save attendance")?;

            let others: Vec<String> = ride_attendees(&state, &id)
                .await?
                .into_iter()
                .map(|a| a.user_id)
                .filter(|u| u != &claims.sub)
                .collect();
            crate::routes::notifications::push_to_users(
                &state,
                others,
                &format!("{author} joined your {} ride", ride.location),
                &format!("Riding {} – {}", pretty_dt(&ride.start_at), pretty_dt(&ride.end_at)),
                Some("/rides"),
            );
        }
    } else {
        let _ = state.db
            .fluent()
            .delete()
            .from(ATTENDEES)
            .document_id(&doc_id)
            .execute()
            .await;
    }

    let attendees = ride_attendees(&state, &id).await?;
    let names = attendees.iter().map(|a| a.author.clone()).collect();
    Ok(Json(doc_to_ride(ride, names, req.going)))
}
