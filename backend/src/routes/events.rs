use crate::{
    AppState,
    auth::{require_admin, require_auth},
    error::{AppError, AppResult},
    models::EventDoc,
};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{CreateEventRequest, Event, UpdateEventRequest};
use uuid::Uuid;

const EVENTS: &str = "movie_nights";

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, put};
    axum::Router::new()
        .route("/", get(list_events).post(create_event))
        .route("/:id", put(update_event).delete(delete_event))
}

fn doc_to_event(d: EventDoc) -> Event {
    Event {
        id: d.id,
        event_type: d.event_type,
        title: d.title,
        date: d.date,
        description: d.description,
        poll_embed_url: d.poll_embed_url,
        poster_url: d.poster_url,
    }
}

async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Event>>> {
    require_auth(&headers, &state.jwt_secret)?;

    let mut docs: Vec<EventDoc> = state.db
        .fluent()
        .select()
        .from(EVENTS)
        .obj()
        .query()
        .await
        .context("failed to list events")?;

    docs.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(Json(docs.into_iter().map(doc_to_event).collect()))
}

async fn create_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateEventRequest>,
) -> AppResult<Json<Event>> {
    require_admin(&headers, &state.jwt_secret)?;

    if req.event_type != "main" && req.event_type != "special" {
        return Err(AppError::BadRequest("event_type must be 'main' or 'special'".into()));
    }

    let id = Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let doc = EventDoc {
        id: id.clone(),
        event_type: req.event_type.clone(),
        title: req.title.clone(),
        date: req.date.clone(),
        description: req.description.clone(),
        poll_embed_url: req.poll_embed_url.clone(),
        poster_url: req.poster_url.clone(),
        created_at: now,
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

    Ok(Json(doc_to_event(doc)))
}

async fn update_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateEventRequest>,
) -> AppResult<Json<Event>> {
    require_admin(&headers, &state.jwt_secret)?;

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
        date: req.date.unwrap_or(existing.date),
        description: req.description.or(existing.description),
        poll_embed_url: req.poll_embed_url.or(existing.poll_embed_url),
        poster_url: req.poster_url.or(existing.poster_url),
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

    Ok(Json(doc_to_event(updated)))
}

async fn delete_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<()> {
    require_admin(&headers, &state.jwt_secret)?;

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

    Ok(())
}
