use crate::{
    AppState,
    auth::{require_admin, require_auth},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use libsql::params;
use shared::{CreateEventRequest, Event, UpdateEventRequest};
use uuid::Uuid;

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, put};
    axum::Router::new()
        .route("/", get(list_events).post(create_event))
        .route("/:id", put(update_event).delete(delete_event))
}

async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Event>>> {
    require_auth(&headers, &state.jwt_secret)?;
    let conn = state.db.connect()?;
    let mut rows = conn
        .query(
            "SELECT id, event_type, title, date, description, poll_embed_url
             FROM movie_nights ORDER BY date ASC",
            (),
        )
        .await?;

    let mut events = Vec::new();
    while let Some(row) = rows.next().await? {
        events.push(Event {
            id: row.get(0)?,
            event_type: row.get(1)?,
            title: row.get(2)?,
            date: row.get(3)?,
            description: row.get(4)?,
            poll_embed_url: row.get(5)?,
        });
    }
    Ok(Json(events))
}

async fn create_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateEventRequest>,
) -> AppResult<Json<Event>> {
    require_admin(&headers, &state.jwt_secret)?;

    if req.event_type != "main" && req.event_type != "special" {
        return Err(AppError::BadRequest(
            "event_type must be 'main' or 'special'".into(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let conn = state.db.connect()?;
    conn.execute(
        "INSERT INTO movie_nights (id, event_type, title, date, description, poll_embed_url, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id.clone(),
            req.event_type.clone(),
            req.title.clone(),
            req.date.clone(),
            req.description.clone(),
            req.poll_embed_url.clone(),
            now
        ],
    )
    .await?;

    Ok(Json(Event {
        id,
        event_type: req.event_type,
        title: req.title,
        date: req.date,
        description: req.description,
        poll_embed_url: req.poll_embed_url,
    }))
}

async fn update_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateEventRequest>,
) -> AppResult<Json<Event>> {
    require_admin(&headers, &state.jwt_secret)?;

    let conn = state.db.connect()?;
    let mut rows = conn
        .query(
            "SELECT id, event_type, title, date, description, poll_embed_url
             FROM movie_nights WHERE id = ?1",
            params![id.clone()],
        )
        .await?;
    let row = rows.next().await?.ok_or(AppError::NotFound)?;
    let existing = Event {
        id: row.get(0)?,
        event_type: row.get(1)?,
        title: row.get(2)?,
        date: row.get(3)?,
        description: row.get(4)?,
        poll_embed_url: row.get(5)?,
    };

    let updated = Event {
        id: existing.id.clone(),
        event_type: req.event_type.unwrap_or(existing.event_type),
        title: req.title.unwrap_or(existing.title),
        date: req.date.unwrap_or(existing.date),
        description: req.description.or(existing.description),
        poll_embed_url: req.poll_embed_url.or(existing.poll_embed_url),
    };

    conn.execute(
        "UPDATE movie_nights SET event_type=?1, title=?2, date=?3, description=?4, poll_embed_url=?5
         WHERE id=?6",
        params![
            updated.event_type.clone(),
            updated.title.clone(),
            updated.date.clone(),
            updated.description.clone(),
            updated.poll_embed_url.clone(),
            updated.id.clone()
        ],
    )
    .await?;

    Ok(Json(updated))
}

async fn delete_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<()> {
    require_admin(&headers, &state.jwt_secret)?;
    let conn = state.db.connect()?;
    let changed = conn
        .execute("DELETE FROM movie_nights WHERE id = ?1", params![id])
        .await?;
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}
