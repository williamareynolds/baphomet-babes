//! Group chat: a single whole-group room. Messages are persisted and, like every
//! other notification source, fan out to the (new) chat channel — minus the
//! author's own devices. The feed returns the most recent messages in
//! chronological order.

use crate::{
    AppState,
    auth::require_auth,
    error::{AppError, AppResult},
    models::{ChatMessageDoc, ProfileDoc},
};
use anyhow::Context;
use axum::{Json, extract::State, http::HeaderMap};
use firestore::{FirestoreQueryDirection, path};
use shared::{ChatMessage, SendChatRequest};
use uuid::Uuid;

const CHAT: &str = "chat_messages";
const PROFILES: &str = "profiles";
/// Most recent messages returned to the room view.
const MESSAGE_LIMIT: usize = 50;
/// Reject anything longer to keep one message from blowing up the feed.
const MAX_BODY: usize = 2000;

pub fn router() -> axum::Router<AppState> {
    use axum::routing::get;
    axum::Router::new().route("/", get(list_messages).post(send_message))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn doc_to_message(d: ChatMessageDoc) -> ChatMessage {
    ChatMessage { id: d.id, user_id: d.user_id, author: d.author, body: d.body, created_at: d.created_at }
}

async fn list_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<ChatMessage>>> {
    require_auth(&state, &headers).await?;

    // Bound the read to the newest MESSAGE_LIMIT server-side so Firestore reads
    // never scale with the full chat history. Returned oldest-first for display.
    let mut docs: Vec<ChatMessageDoc> = state.db
        .fluent()
        .select()
        .from(CHAT)
        .order_by([(path!(ChatMessageDoc::created_at), FirestoreQueryDirection::Descending)])
        .limit(MESSAGE_LIMIT as u32)
        .obj()
        .query()
        .await
        .context("failed to list chat messages")?;

    docs.reverse();
    Ok(Json(docs.into_iter().map(doc_to_message).collect()))
}

async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SendChatRequest>,
) -> AppResult<Json<ChatMessage>> {
    let claims = require_auth(&state, &headers).await?;

    let body = req.body.trim();
    if body.is_empty() {
        return Err(AppError::BadRequest("message is required".into()));
    }
    if body.len() > MAX_BODY {
        return Err(AppError::BadRequest("message is too long".into()));
    }

    // Denormalize the author label once at write time (display name preferred,
    // username fallback) so the feed needs no per-message profile joins.
    let profile: Option<ProfileDoc> = state.db
        .fluent()
        .select()
        .by_id_in(PROFILES)
        .obj()
        .one(&claims.sub)
        .await
        .context("failed to load author profile")?;
    let author = profile
        .map(|p| p.display_name.filter(|s| !s.is_empty()).unwrap_or(p.username))
        .unwrap_or_else(|| "Someone".to_string());

    let doc = ChatMessageDoc {
        id: Uuid::new_v4().to_string(),
        user_id: claims.sub.clone(),
        author: author.clone(),
        body: body.to_string(),
        created_at: now(),
    };

    let _: ChatMessageDoc = state.db
        .fluent()
        .insert()
        .into(CHAT)
        .document_id(&doc.id)
        .object(&doc)
        .execute()
        .await
        .context("failed to send chat message")?;

    // Push to the chat channel, excluding the author's own devices. Push-only
    // (no inbox record) so chatter doesn't bury the announcements feed.
    crate::routes::notifications::push_only(
        &state,
        shared::CHANNEL_CHAT,
        &author,
        body,
        Some("/chat"),
        Some(&claims.sub),
    );

    Ok(Json(doc_to_message(doc)))
}
