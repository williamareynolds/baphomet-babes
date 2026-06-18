use crate::{
    AppState,
    auth::{require_admin, require_auth},
    error::{AppError, AppResult},
    models::AnnouncementDoc,
};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{Announcement, CreateAnnouncementRequest, UpdateAnnouncementRequest};
use uuid::Uuid;

const ANNOUNCEMENTS: &str = "announcements";

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, put};
    axum::Router::new()
        .route("/", get(list_announcements).post(create_announcement))
        .route("/{id}", put(update_announcement).delete(delete_announcement))
}

fn doc_to_announcement(d: AnnouncementDoc) -> Announcement {
    Announcement {
        id: d.id,
        title: d.title,
        body: d.body,
        poll_embed_url: d.poll_embed_url,
        created_at: d.created_at,
    }
}

async fn list_announcements(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Announcement>>> {
    require_auth(&state, &headers).await?;

    let mut docs: Vec<AnnouncementDoc> = state.db
        .fluent()
        .select()
        .from(ANNOUNCEMENTS)
        .obj()
        .query()
        .await
        .context("failed to list announcements")?;

    // Newest first.
    docs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(docs.into_iter().map(doc_to_announcement).collect()))
}

async fn create_announcement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateAnnouncementRequest>,
) -> AppResult<Json<Announcement>> {
    let claims = require_admin(&state, &headers).await?;

    if req.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }

    let id = Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let doc = AnnouncementDoc {
        id: id.clone(),
        title: req.title.clone(),
        body: req.body.clone(),
        poll_embed_url: req.poll_embed_url.clone(),
        created_by: claims.sub,
        created_at: now,
    };

    let _: AnnouncementDoc = state.db
        .fluent()
        .insert()
        .into(ANNOUNCEMENTS)
        .document_id(&id)
        .object(&doc)
        .execute()
        .await
        .context("failed to create announcement")?;

    // Notify the announcements channel (persist + best-effort push). Never let a
    // notification hiccup fail the post itself.
    if let Err(e) = crate::routes::notifications::dispatch(
        &state,
        shared::CHANNEL_ANNOUNCEMENTS,
        &doc.title,
        &doc.body,
        Some("/"),
    ).await {
        tracing::warn!("announcement notification failed: {e:#}");
    }

    Ok(Json(doc_to_announcement(doc)))
}

async fn update_announcement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateAnnouncementRequest>,
) -> AppResult<Json<Announcement>> {
    require_admin(&state, &headers).await?;

    let existing: Option<AnnouncementDoc> = state.db
        .fluent()
        .select()
        .by_id_in(ANNOUNCEMENTS)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch announcement")?;

    let existing = existing.ok_or(AppError::NotFound)?;

    let updated = AnnouncementDoc {
        id: existing.id.clone(),
        title: req.title.unwrap_or(existing.title),
        body: req.body.unwrap_or(existing.body),
        poll_embed_url: req.poll_embed_url.or(existing.poll_embed_url),
        created_by: existing.created_by,
        created_at: existing.created_at,
    };

    let _: AnnouncementDoc = state.db
        .fluent()
        .update()
        .in_col(ANNOUNCEMENTS)
        .document_id(&id)
        .object(&updated)
        .execute()
        .await
        .context("failed to update announcement")?;

    Ok(Json(doc_to_announcement(updated)))
}

async fn delete_announcement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<()> {
    require_admin(&state, &headers).await?;

    let exists: Option<AnnouncementDoc> = state.db
        .fluent()
        .select()
        .by_id_in(ANNOUNCEMENTS)
        .obj()
        .one(&id)
        .await
        .context("failed to check announcement")?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    state.db
        .fluent()
        .delete()
        .from(ANNOUNCEMENTS)
        .document_id(&id)
        .execute()
        .await
        .context("failed to delete announcement")?;

    Ok(())
}
