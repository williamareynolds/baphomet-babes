//! Gatherings: club get-togethers with a stated date, time and place.
//!
//! Deliberately separate from movie nights. A screening's date comes *out* of a
//! poll, so `Event` models it as optional and the whole page is built around
//! voting; a gathering states its date, time and location up front and never
//! votes. Sharing one type would have put those requirements in validation
//! rather than the type, and dragged gatherings through the voting flow.
//!
//! RSVP visibility matches movie nights: the count is public — it tells a member
//! whether anyone's going — while the names are admin-only, so attendance isn't
//! broadcast to the whole club.
//!
//! Only admins post gatherings: creating one notifies all ~30 members, which is
//! not a per-member privilege.

use crate::{
    AppState,
    auth::{require_admin, require_auth},
    error::{AppError, AppResult},
    models::{GatheringDoc, GatheringRsvpDoc, ProfileDoc},
};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use base64::Engine;
use shared::{
    CreateGatheringRequest, Gathering, GatheringPlace, GeocodeRequest, GeocodeResponse, Rsvp,
    RsvpRequest, UpdateGatheringRequest, UploadImageRequest, UploadImageResponse,
};
use uuid::Uuid;

const GATHERINGS: &str = "gatherings";
const GATHERING_RSVPS: &str = "gathering_rsvps";
const PROFILES: &str = "profiles";

/// Free-text fields are trimmed and capped so one entry can't balloon a doc.
const MAX_TEXT: usize = 2_000;
const MAX_ADDRESS: usize = 500;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route("/", get(list_gatherings).post(create_gathering))
        .route("/{id}", put(update_gathering).delete(delete_gathering))
        .route("/{id}/rsvp", post(rsvp))
        .route("/{id}/rsvps", get(list_rsvps))
        .route("/cover", post(upload_cover))
        .route("/geocode", post(geocode))
}

/// Store a cover image and hand back its public URL, which the caller then
/// saves on the gathering. Admin-only, like posting a gathering — this writes
/// to a public bucket and should not be an arbitrary-member capability.
async fn upload_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UploadImageRequest>,
) -> AppResult<Json<UploadImageResponse>> {
    require_admin(&state, &headers).await?;

    // Validate the request before consulting server config: a bad content type
    // is the caller's problem whether or not storage happens to be wired up,
    // and "uploads aren't configured" would be a misleading answer to it.
    let Some(ext) = crate::storage::extension_for(req.content_type.trim()) else {
        return Err(AppError::BadRequest(
            "cover must be a JPEG, PNG, WebP or GIF image".into(),
        ));
    };

    let Some(media) = &state.media else {
        return Err(AppError::BadRequest(
            "image uploads aren't configured on this server".into(),
        ));
    };

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(req.data_base64.as_bytes())
        .map_err(|_| AppError::BadRequest("cover image was not valid base64".into()))?;

    if bytes.is_empty() {
        return Err(AppError::BadRequest("cover image was empty".into()));
    }
    if bytes.len() > crate::storage::MAX_BYTES {
        return Err(AppError::BadRequest(format!(
            "cover image is too large (max {} MB)",
            crate::storage::MAX_BYTES / (1024 * 1024)
        )));
    }

    // UUID name: the bucket is public-read, so object names carry the privacy.
    let name = format!("gatherings/{}.{ext}", Uuid::new_v4());
    let url = media
        .upload(&name, req.content_type.trim(), bytes)
        .await
        .context("failed to store cover image")?;

    Ok(Json(UploadImageResponse { url }))
}

/// Turn a typed address into a pin. A miss is `found: false`, not an error:
/// the form leaves the pin editable either way, so geocoding is a convenience
/// and never a gate on posting.
async fn geocode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GeocodeRequest>,
) -> AppResult<Json<GeocodeResponse>> {
    require_admin(&state, &headers).await?;

    Ok(Json(match crate::geocode::lookup(&req.query).await {
        Some(l) => GeocodeResponse {
            found: true,
            lat: Some(l.lat),
            lng: Some(l.lng),
            display_name: Some(l.display_name),
        },
        None => GeocodeResponse { found: false, lat: None, lng: None, display_name: None },
    }))
}

fn doc_to_gathering(d: GatheringDoc, rsvp_count: i64, my_rsvp: bool) -> Gathering {
    Gathering {
        id: d.id,
        title: d.title,
        description: d.description,
        starts_at: d.starts_at,
        ends_at: d.ends_at,
        address: d.address,
        lat: d.lat,
        lng: d.lng,
        cover_url: d.cover_url,
        created_by: d.created_by,
        created_at: d.created_at,
        rsvp_count,
        my_rsvp,
    }
}

fn trimmed(s: Option<String>, cap: usize) -> Option<String> {
    s.map(|v| v.trim().chars().take(cap).collect::<String>())
        .filter(|v| !v.is_empty())
}

/// Resolve a member's display label for denormalizing onto an RSVP, mirroring
/// the movie-night and chat convention.
async fn author_label(state: &AppState, user_id: &str) -> AppResult<String> {
    let profile: Option<ProfileDoc> = state.db
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

async fn gathering_rsvps(state: &AppState, gathering_id: &str) -> AppResult<Vec<GatheringRsvpDoc>> {
    let rsvps: Vec<GatheringRsvpDoc> = state.db
        .fluent()
        .select()
        .from(GATHERING_RSVPS)
        .filter(|q| q.field("gathering_id").eq(gathering_id))
        .obj()
        .query()
        .await
        .context("failed to query gathering rsvps")?;
    Ok(rsvps)
}

async fn count_rsvps(state: &AppState, gathering_id: &str) -> AppResult<i64> {
    Ok(gathering_rsvps(state, gathering_id).await?.len() as i64)
}

async fn load(state: &AppState, id: &str) -> AppResult<GatheringDoc> {
    let doc: Option<GatheringDoc> = state.db
        .fluent()
        .select()
        .by_id_in(GATHERINGS)
        .obj()
        .one(id)
        .await
        .context("failed to fetch gathering")?;
    doc.ok_or(AppError::NotFound)
}

/// Soonest first — a gathering list is about what's coming up, unlike the
/// movie-night archive which reads backwards through past screenings.
async fn list_gatherings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Gathering>>> {
    let claims = require_auth(&state, &headers).await?;

    let mut docs: Vec<GatheringDoc> = state.db
        .fluent()
        .select()
        .from(GATHERINGS)
        .obj()
        .query()
        .await
        .context("failed to list gatherings")?;
    docs.sort_by(|a, b| a.starts_at.cmp(&b.starts_at));

    // One RSVP query for the whole list rather than one per gathering.
    let all: Vec<GatheringRsvpDoc> = state.db
        .fluent()
        .select()
        .from(GATHERING_RSVPS)
        .obj()
        .query()
        .await
        .context("failed to load gathering rsvps")?;

    let out = docs
        .into_iter()
        .map(|d| {
            let count = all.iter().filter(|r| r.gathering_id == d.id).count() as i64;
            let mine = all
                .iter()
                .any(|r| r.gathering_id == d.id && r.user_id == claims.sub);
            doc_to_gathering(d, count, mine)
        })
        .collect();
    Ok(Json(out))
}

async fn create_gathering(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateGatheringRequest>,
) -> AppResult<Json<Gathering>> {
    let claims = require_admin(&state, &headers).await?;

    let title = req.title.trim().chars().take(200).collect::<String>();
    let address = trimmed(req.address.clone(), MAX_ADDRESS);
    let ends_at = req.ends_at.clone().filter(|e| !e.is_empty());

    shared::validate_gathering(
        &title,
        &req.starts_at,
        ends_at.as_deref(),
        GatheringPlace { address: address.as_deref(), lat: req.lat, lng: req.lng },
    )
    .map_err(AppError::BadRequest)?;

    let id = Uuid::new_v4().to_string();
    let doc = GatheringDoc {
        id: id.clone(),
        title: title.clone(),
        description: trimmed(req.description.clone(), MAX_TEXT),
        starts_at: req.starts_at.clone(),
        ends_at,
        address,
        lat: req.lat,
        lng: req.lng,
        cover_url: trimmed(req.cover_url.clone(), MAX_ADDRESS),
        created_by: claims.sub,
        created_at: now(),
    };

    let _: GatheringDoc = state.db
        .fluent()
        .insert()
        .into(GATHERINGS)
        .document_id(&id)
        .object(&doc)
        .execute()
        .await
        .context("failed to create gathering")?;

    // Notify the gatherings channel (persist + best-effort push and email).
    let when = pretty_when(&doc.starts_at);
    let where_ = doc.address.clone().unwrap_or_else(|| "see the map".to_string());
    if let Err(e) = crate::routes::notifications::dispatch(
        &state,
        shared::CHANNEL_GATHERINGS,
        &format!("New gathering: {}", doc.title),
        &format!("{when} — {where_}"),
        Some("/gatherings"),
        None,
    )
    .await
    {
        tracing::warn!("gathering notification failed: {e:#}");
    }

    Ok(Json(doc_to_gathering(doc, 0, false)))
}

/// "2026-09-01T18:30" -> "2026-09-01 at 18:30". Deliberately dumb: the client
/// renders the pretty version, this is only for notification copy.
fn pretty_when(starts_at: &str) -> String {
    match starts_at.split_once('T') {
        Some((date, time)) => format!("{date} at {time}"),
        None => starts_at.to_string(),
    }
}

async fn update_gathering(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateGatheringRequest>,
) -> AppResult<Json<Gathering>> {
    require_admin(&state, &headers).await?;
    let existing = load(&state, &id).await?;

    let title = req.title.clone().unwrap_or(existing.title.clone());
    let title = title.trim().chars().take(200).collect::<String>();
    let starts_at = req.starts_at.clone().unwrap_or(existing.starts_at.clone());
    // Some("") clears the end time, Some(v) sets it, None keeps it.
    let ends_at = match &req.ends_at {
        Some(e) if e.is_empty() => None,
        Some(e) => Some(e.clone()),
        None => existing.ends_at.clone(),
    };
    let address = match &req.address {
        Some(a) if a.trim().is_empty() => None,
        Some(a) => trimmed(Some(a.clone()), MAX_ADDRESS),
        None => existing.address.clone(),
    };
    let (lat, lng) = if req.clear_pin {
        (None, None)
    } else {
        match (req.lat, req.lng) {
            (Some(la), Some(ln)) => (Some(la), Some(ln)),
            _ => (existing.lat, existing.lng),
        }
    };

    shared::validate_gathering(
        &title,
        &starts_at,
        ends_at.as_deref(),
        GatheringPlace { address: address.as_deref(), lat, lng },
    )
    .map_err(AppError::BadRequest)?;

    let updated = GatheringDoc {
        id: existing.id.clone(),
        title,
        description: match &req.description {
            Some(d) if d.trim().is_empty() => None,
            Some(d) => trimmed(Some(d.clone()), MAX_TEXT),
            None => existing.description.clone(),
        },
        starts_at,
        ends_at,
        address,
        lat,
        lng,
        cover_url: match &req.cover_url {
            Some(c) if c.trim().is_empty() => None,
            Some(c) => trimmed(Some(c.clone()), MAX_ADDRESS),
            None => existing.cover_url.clone(),
        },
        created_by: existing.created_by.clone(),
        created_at: existing.created_at,
    };

    let _: GatheringDoc = state.db
        .fluent()
        .update()
        .in_col(GATHERINGS)
        .document_id(&id)
        .object(&updated)
        .execute()
        .await
        .context("failed to update gathering")?;

    let count = count_rsvps(&state, &id).await?;
    Ok(Json(doc_to_gathering(updated, count, false)))
}

async fn delete_gathering(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<()> {
    require_admin(&state, &headers).await?;
    load(&state, &id).await?;

    state.db
        .fluent()
        .delete()
        .from(GATHERINGS)
        .document_id(&id)
        .execute()
        .await
        .context("failed to delete gathering")?;

    // Drop the RSVPs too, so a re-used id can't inherit a stale guest list.
    for r in gathering_rsvps(&state, &id).await? {
        let _ = state.db
            .fluent()
            .delete()
            .from(GATHERING_RSVPS)
            .document_id(&r.id)
            .execute()
            .await;
    }
    Ok(())
}

/// Member RSVPs (or cancels). Idempotent: the deterministic doc id makes
/// re-RSVPing a no-op and double-cancel harmless.
async fn rsvp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RsvpRequest>,
) -> AppResult<Json<Gathering>> {
    let claims = require_auth(&state, &headers).await?;
    let gathering = load(&state, &id).await?;

    let doc_id = format!("{id}_{}", claims.sub);
    if req.going {
        let author = author_label(&state, &claims.sub).await?;
        let doc = GatheringRsvpDoc {
            id: doc_id.clone(),
            gathering_id: id.clone(),
            user_id: claims.sub.clone(),
            author,
            created_at: now(),
        };
        let _: GatheringRsvpDoc = state.db
            .fluent()
            .update() // upsert
            .in_col(GATHERING_RSVPS)
            .document_id(&doc_id)
            .object(&doc)
            .execute()
            .await
            .context("failed to save gathering rsvp")?;
    } else {
        let _ = state.db
            .fluent()
            .delete()
            .from(GATHERING_RSVPS)
            .document_id(&doc_id)
            .execute()
            .await;
    }

    let count = count_rsvps(&state, &id).await?;
    Ok(Json(doc_to_gathering(gathering, count, req.going)))
}

/// Admin-only: who's going. Members see only the count, which the gathering
/// itself already carries.
async fn list_rsvps(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<Rsvp>>> {
    require_admin(&state, &headers).await?;

    let mut rsvps = gathering_rsvps(&state, &id).await?;
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

    #[test]
    fn notification_copy_reads_as_a_time() {
        assert_eq!(pretty_when("2026-09-01T18:30"), "2026-09-01 at 18:30");
        // Anything unexpected passes through rather than panicking.
        assert_eq!(pretty_when("sometime"), "sometime");
    }

    #[test]
    fn free_text_is_trimmed_and_capped() {
        assert_eq!(trimmed(Some("  hi  ".into()), 10), Some("hi".into()));
        assert_eq!(trimmed(Some("   ".into()), 10), None);
        assert_eq!(trimmed(None, 10), None);
        assert_eq!(trimmed(Some("abcdef".into()), 3), Some("abc".into()));
    }
}
