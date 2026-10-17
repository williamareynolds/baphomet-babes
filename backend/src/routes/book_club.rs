//! Book Club: one book at a time. An admin sets a current pick and an ordered
//! queue of what's next; members join the club (ongoing membership, flips the
//! Book Club notification channel) and separately RSVP to the next meetup
//! (open to any member, join status notwithstanding — same as Gatherings).
//!
//! The queue is admin-only: members never see what's coming, only what's
//! current and what's already been read.

use crate::{
    AppState,
    auth::{require_auth, require_permission},
    error::{AppError, AppResult},
    models::{BookClubBookDoc, BookClubMeetupDoc, BookClubMeetupRsvpDoc, BookClubMemberDoc, ProfileDoc},
};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use base64::Engine;
use shared::{
    CreateBookClubBookRequest, GeocodeRequest, GeocodeResponse, MoveDirection, MoveQueueRequest,
    RsvpRequest, UpdateBookClubBookRequest, UpdateBookClubMeetupRequest, UploadImageRequest,
    UploadImageResponse,
};
use uuid::Uuid;

const BOOKS: &str = "book_club_books";
const MEMBERS: &str = "book_club_members";
const MEETUP: &str = "book_club_meetup";
const MEETUP_RSVPS: &str = "book_club_meetup_rsvps";
const PROFILES: &str = "profiles";
const MEETUP_DOC_ID: &str = "current";

const MAX_TEXT: usize = 2_000;
const MAX_ADDRESS: usize = 500;
const MAX_SHORT: usize = 200;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route("/current", get(get_current).put(update_current))
        .route("/meetup", get(get_meetup).put(update_meetup))
        .route("/meetup/rsvp", post(rsvp_meetup))
        .route("/archive", get(get_archive))
        .route("/join", post(join))
        .route("/leave", post(leave))
        .route("/queue", get(get_queue).post(add_to_queue))
        .route("/queue/{id}", put(update_queue_item).delete(remove_from_queue))
        .route("/queue/{id}/move", post(move_in_queue))
        .route("/finish", post(finish))
        .route("/cover", post(upload_cover))
        .route("/geocode", post(geocode))
}

fn trimmed(s: Option<String>, cap: usize) -> Option<String> {
    s.map(|v| v.trim().chars().take(cap).collect::<String>())
        .filter(|v| !v.is_empty())
}

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

fn doc_to_book(d: BookClubBookDoc) -> shared::BookClubBook {
    shared::BookClubBook {
        id: d.id,
        title: d.title,
        author: d.author,
        published: d.published,
        why: d.why,
        cover_url: d.cover_url,
        created_by: d.created_by,
        created_at: d.created_at,
        started_at: d.started_at,
        finished_at: d.finished_at,
    }
}

fn doc_to_meetup(d: BookClubMeetupDoc, going: Vec<String>, my_rsvp: bool) -> shared::BookClubMeetup {
    shared::BookClubMeetup {
        starts_at: d.starts_at,
        host: d.host,
        address: d.address,
        lat: d.lat,
        lng: d.lng,
        rsvp_count: going.len() as i64,
        my_rsvp,
        going,
    }
}

async fn load_current(state: &AppState) -> AppResult<Option<BookClubBookDoc>> {
    let docs: Vec<BookClubBookDoc> = state.db
        .fluent()
        .select()
        .from(BOOKS)
        .filter(|q| q.field("status").eq("current"))
        .obj()
        .query()
        .await
        .context("failed to load current book")?;
    Ok(docs.into_iter().next())
}

async fn load_queue(state: &AppState) -> AppResult<Vec<BookClubBookDoc>> {
    let mut docs: Vec<BookClubBookDoc> = state.db
        .fluent()
        .select()
        .from(BOOKS)
        .filter(|q| q.field("status").eq("queued"))
        .obj()
        .query()
        .await
        .context("failed to load queue")?;
    docs.sort_by_key(|d| d.position);
    Ok(docs)
}

async fn load_archive(state: &AppState) -> AppResult<Vec<BookClubBookDoc>> {
    let mut docs: Vec<BookClubBookDoc> = state.db
        .fluent()
        .select()
        .from(BOOKS)
        .filter(|q| q.field("status").eq("archived"))
        .obj()
        .query()
        .await
        .context("failed to load archive")?;
    docs.sort_by(|a, b| b.finished_at.cmp(&a.finished_at));
    Ok(docs)
}

async fn load_members(state: &AppState) -> AppResult<Vec<BookClubMemberDoc>> {
    let docs: Vec<BookClubMemberDoc> = state.db
        .fluent()
        .select()
        .from(MEMBERS)
        .obj()
        .query()
        .await
        .context("failed to load book club members")?;
    Ok(docs)
}

async fn load_meetup(state: &AppState) -> AppResult<Option<BookClubMeetupDoc>> {
    let doc: Option<BookClubMeetupDoc> = state.db
        .fluent()
        .select()
        .by_id_in(MEETUP)
        .obj()
        .one(MEETUP_DOC_ID)
        .await
        .context("failed to load meetup")?;
    Ok(doc)
}

async fn meetup_rsvps(state: &AppState) -> AppResult<Vec<BookClubMeetupRsvpDoc>> {
    let mut list: Vec<BookClubMeetupRsvpDoc> = state.db
        .fluent()
        .select()
        .from(MEETUP_RSVPS)
        .obj()
        .query()
        .await
        .context("failed to load meetup rsvps")?;
    list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(list)
}

async fn status_for(state: &AppState, user_id: &str) -> AppResult<Json<shared::BookClubStatus>> {
    let current = load_current(state).await?;
    let members = load_members(state).await?;
    let joined = members.iter().any(|m| m.user_id == user_id);
    Ok(Json(shared::BookClubStatus {
        current: current.map(doc_to_book),
        joined,
        member_count: members.len() as i64,
        reading_along: members.into_iter().map(|m| m.author).collect(),
    }))
}

// ---- public reads ----

async fn get_current(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<shared::BookClubStatus>> {
    let claims = require_auth(&state, &headers).await?;
    status_for(&state, &claims.sub).await
}

async fn get_meetup(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Option<shared::BookClubMeetup>>> {
    let claims = require_auth(&state, &headers).await?;
    let Some(doc) = load_meetup(&state).await? else { return Ok(Json(None)) };
    let rsvps = meetup_rsvps(&state).await?;
    let going: Vec<String> = rsvps.iter().map(|r| r.author.clone()).collect();
    let my = rsvps.iter().any(|r| r.user_id == claims.sub);
    Ok(Json(Some(doc_to_meetup(doc, going, my))))
}

async fn get_archive(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Vec<shared::BookClubBook>>> {
    require_auth(&state, &headers).await?;
    let docs = load_archive(&state).await?;
    Ok(Json(docs.into_iter().map(doc_to_book).collect()))
}

// ---- membership ----

/// Join the club: ongoing membership, not tied to any one meetup. Also flips
/// the Book Club notification channel on (push and email), per the design's
/// "Joining turns on Book Club notifications" copy.
async fn join(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<shared::BookClubStatus>> {
    let claims = require_auth(&state, &headers).await?;
    let author = author_label(&state, &claims.sub).await?;
    let doc = BookClubMemberDoc {
        id: claims.sub.clone(),
        user_id: claims.sub.clone(),
        author,
        joined_at: now(),
    };
    let _: BookClubMemberDoc = state.db
        .fluent()
        .update() // upsert
        .in_col(MEMBERS)
        .document_id(&claims.sub)
        .object(&doc)
        .execute()
        .await
        .context("failed to join book club")?;

    crate::routes::notifications::set_book_club_prefs(&state, &claims.sub, true).await?;
    status_for(&state, &claims.sub).await
}

/// Leave the club. Notifications go off; any standing meetup RSVP is kept —
/// leaving the club and cancelling a meetup are separate decisions.
async fn leave(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<shared::BookClubStatus>> {
    let claims = require_auth(&state, &headers).await?;
    let _ = state.db
        .fluent()
        .delete()
        .from(MEMBERS)
        .document_id(&claims.sub)
        .execute()
        .await;

    crate::routes::notifications::set_book_club_prefs(&state, &claims.sub, false).await?;
    status_for(&state, &claims.sub).await
}

// ---- meetup RSVP (open to any member, not gated by club membership) ----

async fn rsvp_meetup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RsvpRequest>,
) -> AppResult<Json<shared::BookClubMeetup>> {
    let claims = require_auth(&state, &headers).await?;
    let doc = load_meetup(&state).await?.ok_or(AppError::NotFound)?;

    if req.going {
        let author = author_label(&state, &claims.sub).await?;
        let rsvp = BookClubMeetupRsvpDoc {
            id: claims.sub.clone(),
            user_id: claims.sub.clone(),
            author,
            created_at: now(),
        };
        let _: BookClubMeetupRsvpDoc = state.db
            .fluent()
            .update() // upsert
            .in_col(MEETUP_RSVPS)
            .document_id(&claims.sub)
            .object(&rsvp)
            .execute()
            .await
            .context("failed to save meetup rsvp")?;
    } else {
        let _ = state.db
            .fluent()
            .delete()
            .from(MEETUP_RSVPS)
            .document_id(&claims.sub)
            .execute()
            .await;
    }

    let rsvps = meetup_rsvps(&state).await?;
    let going = rsvps.iter().map(|r| r.author.clone()).collect();
    Ok(Json(doc_to_meetup(doc, going, req.going)))
}

// ---- admin: queue ----

async fn get_queue(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Vec<shared::BookClubBook>>> {
    require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;
    let docs = load_queue(&state).await?;
    Ok(Json(docs.into_iter().map(doc_to_book).collect()))
}

async fn add_to_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateBookClubBookRequest>,
) -> AppResult<Json<shared::BookClubBook>> {
    let claims = require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;

    let title = req.title.trim().chars().take(MAX_SHORT).collect::<String>();
    let author = req.author.trim().chars().take(MAX_SHORT).collect::<String>();
    shared::validate_book_club_book(&title, &author).map_err(AppError::BadRequest)?;

    let queue = load_queue(&state).await?;
    let position = queue.last().map(|b| b.position + 1).unwrap_or(0);

    let id = Uuid::new_v4().to_string();
    let doc = BookClubBookDoc {
        id: id.clone(),
        title,
        author,
        published: trimmed(req.published.clone(), 50),
        why: trimmed(req.why.clone(), MAX_TEXT),
        cover_url: trimmed(req.cover_url.clone(), MAX_ADDRESS),
        status: "queued".to_string(),
        position,
        started_at: None,
        finished_at: None,
        created_by: claims.sub,
        created_at: now(),
    };
    let _: BookClubBookDoc = state.db
        .fluent()
        .insert()
        .into(BOOKS)
        .document_id(&id)
        .object(&doc)
        .execute()
        .await
        .context("failed to add book to queue")?;

    Ok(Json(doc_to_book(doc)))
}

async fn move_in_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<MoveQueueRequest>,
) -> AppResult<Json<Vec<shared::BookClubBook>>> {
    require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;
    let mut queue = load_queue(&state).await?;
    let idx = queue.iter().position(|b| b.id == id).ok_or(AppError::NotFound)?;
    let swap_idx = match req.direction {
        MoveDirection::Up => idx.checked_sub(1),
        MoveDirection::Down => (idx + 1 < queue.len()).then_some(idx + 1),
    };

    if let Some(j) = swap_idx {
        let pi = queue[idx].position;
        let pj = queue[j].position;
        queue[idx].position = pj;
        queue[j].position = pi;
        let doc_i = queue[idx].clone();
        let doc_j = queue[j].clone();
        let _: BookClubBookDoc = state.db
            .fluent()
            .update()
            .in_col(BOOKS)
            .document_id(&doc_i.id)
            .object(&doc_i)
            .execute()
            .await
            .context("failed to reorder queue")?;
        let _: BookClubBookDoc = state.db
            .fluent()
            .update()
            .in_col(BOOKS)
            .document_id(&doc_j.id)
            .object(&doc_j)
            .execute()
            .await
            .context("failed to reorder queue")?;
    }

    let queue = load_queue(&state).await?;
    Ok(Json(queue.into_iter().map(doc_to_book).collect()))
}

async fn update_queue_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateBookClubBookRequest>,
) -> AppResult<Json<shared::BookClubBook>> {
    require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;
    let queue = load_queue(&state).await?;
    let existing = queue.into_iter().find(|b| b.id == id).ok_or(AppError::NotFound)?;

    let title = req.title
        .map(|t| t.trim().chars().take(MAX_SHORT).collect::<String>())
        .unwrap_or(existing.title.clone());
    let author = req.author
        .map(|a| a.trim().chars().take(MAX_SHORT).collect::<String>())
        .unwrap_or(existing.author.clone());
    shared::validate_book_club_book(&title, &author).map_err(AppError::BadRequest)?;

    let published = match req.published {
        Some(p) if p.trim().is_empty() => None,
        Some(p) => trimmed(Some(p), 50),
        None => existing.published.clone(),
    };
    let why = match req.why {
        Some(w) if w.trim().is_empty() => None,
        Some(w) => trimmed(Some(w), MAX_TEXT),
        None => existing.why.clone(),
    };
    let cover_url = match req.cover_url {
        Some(c) if c.trim().is_empty() => None,
        Some(c) => trimmed(Some(c), MAX_ADDRESS),
        None => existing.cover_url.clone(),
    };

    let updated = BookClubBookDoc { title, author, published, why, cover_url, ..existing };
    let _: BookClubBookDoc = state.db
        .fluent()
        .update()
        .in_col(BOOKS)
        .document_id(&updated.id)
        .object(&updated)
        .execute()
        .await
        .context("failed to update queued book")?;
    Ok(Json(doc_to_book(updated)))
}

async fn remove_from_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<()> {
    require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;
    let queue = load_queue(&state).await?;
    if !queue.iter().any(|b| b.id == id) {
        return Err(AppError::NotFound);
    }
    state.db
        .fluent()
        .delete()
        .from(BOOKS)
        .document_id(&id)
        .execute()
        .await
        .context("failed to remove book from queue")?;
    Ok(())
}

// ---- admin: current book ----

async fn update_current(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateBookClubBookRequest>,
) -> AppResult<Json<shared::BookClubBook>> {
    require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;
    let existing = load_current(&state).await?.ok_or(AppError::NotFound)?;

    let title = req.title
        .map(|t| t.trim().chars().take(MAX_SHORT).collect::<String>())
        .unwrap_or(existing.title.clone());
    let author = req.author
        .map(|a| a.trim().chars().take(MAX_SHORT).collect::<String>())
        .unwrap_or(existing.author.clone());
    shared::validate_book_club_book(&title, &author).map_err(AppError::BadRequest)?;

    let published = match req.published {
        Some(p) if p.trim().is_empty() => None,
        Some(p) => trimmed(Some(p), 50),
        None => existing.published.clone(),
    };
    let why = match req.why {
        Some(w) if w.trim().is_empty() => None,
        Some(w) => trimmed(Some(w), MAX_TEXT),
        None => existing.why.clone(),
    };
    let cover_url = match req.cover_url {
        Some(c) if c.trim().is_empty() => None,
        Some(c) => trimmed(Some(c), MAX_ADDRESS),
        None => existing.cover_url.clone(),
    };

    let updated = BookClubBookDoc { title, author, published, why, cover_url, ..existing };
    let _: BookClubBookDoc = state.db
        .fluent()
        .update()
        .in_col(BOOKS)
        .document_id(&updated.id)
        .object(&updated)
        .execute()
        .await
        .context("failed to update current book")?;
    Ok(Json(doc_to_book(updated)))
}

/// Archive the current book (if any) and promote the front of the queue.
/// With no current book, this just promotes — the way the very first book
/// starts. With an empty queue, the club goes "between books".
async fn finish(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<shared::BookClubStatus>> {
    let claims = require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;

    if let Some(current) = load_current(&state).await? {
        let archived = BookClubBookDoc {
            status: "archived".to_string(),
            finished_at: Some(now()),
            position: 0,
            ..current
        };
        let _: BookClubBookDoc = state.db
            .fluent()
            .update()
            .in_col(BOOKS)
            .document_id(&archived.id)
            .object(&archived)
            .execute()
            .await
            .context("failed to archive current book")?;
    }

    let mut queue = load_queue(&state).await?;
    if !queue.is_empty() {
        let next = queue.remove(0);
        let promoted = BookClubBookDoc {
            status: "current".to_string(),
            started_at: Some(now()),
            position: 0,
            ..next
        };
        let _: BookClubBookDoc = state.db
            .fluent()
            .update()
            .in_col(BOOKS)
            .document_id(&promoted.id)
            .object(&promoted)
            .execute()
            .await
            .context("failed to promote next book")?;

        if let Err(e) = crate::routes::notifications::dispatch(
            &state,
            shared::CHANNEL_BOOK_CLUB,
            &format!("New book: {}", promoted.title),
            &format!("by {}", promoted.author),
            Some("/book-club"),
            None,
        )
        .await
        {
            tracing::warn!("book club notification failed: {e:#}");
        }
    }

    status_for(&state, &claims.sub).await
}

// ---- admin: meetup ----

async fn update_meetup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateBookClubMeetupRequest>,
) -> AppResult<Json<shared::BookClubMeetup>> {
    let claims = require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;
    let existing = load_meetup(&state).await?;

    let address = trimmed(req.address.clone(), MAX_ADDRESS);
    let (lat, lng) = if req.clear_pin {
        (None, None)
    } else {
        match (req.lat, req.lng) {
            (Some(la), Some(ln)) => (Some(la), Some(ln)),
            _ => existing.as_ref().map(|e| (e.lat, e.lng)).unwrap_or((None, None)),
        }
    };

    shared::validate_book_club_meetup(&req.starts_at, &req.host, address.as_deref(), lat, lng)
        .map_err(AppError::BadRequest)?;

    let doc = BookClubMeetupDoc {
        id: MEETUP_DOC_ID.to_string(),
        starts_at: req.starts_at,
        host: req.host.trim().chars().take(MAX_SHORT).collect(),
        address,
        lat,
        lng,
        updated_at: now(),
    };
    let _: BookClubMeetupDoc = state.db
        .fluent()
        .update() // upsert
        .in_col(MEETUP)
        .document_id(MEETUP_DOC_ID)
        .object(&doc)
        .execute()
        .await
        .context("failed to save meetup")?;

    let rsvps = meetup_rsvps(&state).await?;
    let going: Vec<String> = rsvps.iter().map(|r| r.author.clone()).collect();
    let my = rsvps.iter().any(|r| r.user_id == claims.sub);
    Ok(Json(doc_to_meetup(doc, going, my)))
}

// ---- admin: cover upload + geocode (same pattern as gatherings.rs) ----

async fn upload_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UploadImageRequest>,
) -> AppResult<Json<UploadImageResponse>> {
    require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;

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

    let name = format!("book-club/{}.{ext}", Uuid::new_v4());
    let url = media
        .upload(&name, req.content_type.trim(), bytes)
        .await
        .context("failed to store cover image")?;

    Ok(Json(UploadImageResponse { url }))
}

async fn geocode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GeocodeRequest>,
) -> AppResult<Json<GeocodeResponse>> {
    require_permission(&state, &headers, shared::PERMISSION_BOOK_CLUB_MANAGER).await?;

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
