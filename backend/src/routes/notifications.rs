//! Notifications: the inbox feed, per-user channel preferences, FCM device-token
//! registration, and the admin broadcast tool.
//!
//! Every notification is persisted (so members can browse an inbox) and, when
//! FCM is configured, pushed to subscribed devices. Pushing is best-effort and
//! happens in the background so it never blocks or fails the originating action.

use std::collections::HashMap;

use crate::{
    AppState,
    auth::{require_admin, require_auth},
    error::{AppError, AppResult},
    fcm::SendOutcome,
    models::{NotifPrefsDoc, NotificationDoc, PushTokenDoc},
};
use anyhow::Context;
use axum::{Json, extract::State, http::HeaderMap};
use shared::{
    BroadcastRequest, CHANNEL_GENERAL, Notification, NotificationPrefs, RegisterPushTokenRequest,
    UpdateNotificationPrefs,
};
use uuid::Uuid;

const NOTIFICATIONS: &str = "notifications";
const PUSH_TOKENS: &str = "push_tokens";
const NOTIF_PREFS: &str = "notif_prefs";

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route("/", get(list_feed))
        .route("/token", put(register_token).delete(unregister_token))
        .route("/prefs", get(get_prefs).put(update_prefs))
        .route("/broadcast", post(broadcast))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn doc_to_notification(d: NotificationDoc) -> Notification {
    Notification {
        id: d.id,
        channel: d.channel,
        title: d.title,
        body: d.body,
        url: d.url,
        created_at: d.created_at,
    }
}

fn prefs_for(channel: &str, p: &NotifPrefsDoc) -> bool {
    match channel {
        shared::CHANNEL_ANNOUNCEMENTS => p.announcements,
        shared::CHANNEL_GENERAL => p.general,
        shared::CHANNEL_MOVIE_NIGHT => p.movie_night,
        _ => false,
    }
}

// ---- inbox feed ----

async fn list_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Notification>>> {
    require_auth(&state, &headers).await?;

    let mut docs: Vec<NotificationDoc> = state.db
        .fluent()
        .select()
        .from(NOTIFICATIONS)
        .obj()
        .query()
        .await
        .context("failed to list notifications")?;

    docs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(docs.into_iter().map(doc_to_notification).collect()))
}

// ---- device tokens ----

async fn register_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterPushTokenRequest>,
) -> AppResult<()> {
    let claims = require_auth(&state, &headers).await?;
    if req.token.trim().is_empty() {
        return Err(AppError::BadRequest("token is required".into()));
    }

    // Doc id = token, so re-registering the same device is idempotent.
    let doc = PushTokenDoc {
        token: req.token.clone(),
        user_id: claims.sub,
        created_at: now(),
    };
    let _: PushTokenDoc = state.db
        .fluent()
        .update()
        .in_col(PUSH_TOKENS)
        .document_id(&req.token)
        .object(&doc)
        .execute()
        .await
        .context("failed to register push token")?;
    Ok(())
}

async fn unregister_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterPushTokenRequest>,
) -> AppResult<()> {
    require_auth(&state, &headers).await?;
    state.db
        .fluent()
        .delete()
        .from(PUSH_TOKENS)
        .document_id(&req.token)
        .execute()
        .await
        .context("failed to unregister push token")?;
    Ok(())
}

// ---- preferences ----

async fn load_prefs(state: &AppState, user_id: &str) -> anyhow::Result<NotifPrefsDoc> {
    let existing: Option<NotifPrefsDoc> = state.db
        .fluent()
        .select()
        .by_id_in(NOTIF_PREFS)
        .obj()
        .one(user_id)
        .await
        .context("failed to fetch notif prefs")?;
    Ok(existing.unwrap_or(NotifPrefsDoc {
        user_id: user_id.to_string(),
        announcements: true,
        general: true,
        movie_night: true,
    }))
}

async fn get_prefs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<NotificationPrefs>> {
    let claims = require_auth(&state, &headers).await?;
    let p = load_prefs(&state, &claims.sub).await?;
    Ok(Json(NotificationPrefs {
        announcements: p.announcements,
        general: p.general,
        movie_night: p.movie_night,
    }))
}

async fn update_prefs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateNotificationPrefs>,
) -> AppResult<Json<NotificationPrefs>> {
    let claims = require_auth(&state, &headers).await?;
    let existing = load_prefs(&state, &claims.sub).await?;

    let updated = NotifPrefsDoc {
        user_id: claims.sub.clone(),
        announcements: req.announcements.unwrap_or(existing.announcements),
        general: req.general.unwrap_or(existing.general),
        movie_night: req.movie_night.unwrap_or(existing.movie_night),
    };

    let _: NotifPrefsDoc = state.db
        .fluent()
        .update()
        .in_col(NOTIF_PREFS)
        .document_id(&claims.sub)
        .object(&updated)
        .execute()
        .await
        .context("failed to update notif prefs")?;

    Ok(Json(NotificationPrefs {
        announcements: updated.announcements,
        general: updated.general,
        movie_night: updated.movie_night,
    }))
}

// ---- admin broadcast (General channel) ----

async fn broadcast(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BroadcastRequest>,
) -> AppResult<()> {
    require_admin(&state, &headers).await?;
    if req.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }
    dispatch(&state, CHANNEL_GENERAL, &req.title, &req.body, Some("/notifications")).await?;
    Ok(())
}

// ---- dispatch: persist + push ----

/// Persist a notification and (best-effort, in the background) push it to every
/// device whose owner is subscribed to `channel`. Called by the announcement,
/// event, and broadcast handlers.
pub async fn dispatch(
    state: &AppState,
    channel: &str,
    title: &str,
    body: &str,
    url: Option<&str>,
) -> anyhow::Result<()> {
    let doc = NotificationDoc {
        id: Uuid::new_v4().to_string(),
        channel: channel.to_string(),
        title: title.to_string(),
        body: body.to_string(),
        url: url.map(|s| s.to_string()),
        created_at: now(),
    };
    let _: NotificationDoc = state.db
        .fluent()
        .insert()
        .into(NOTIFICATIONS)
        .document_id(&doc.id)
        .object(&doc)
        .execute()
        .await
        .context("failed to persist notification")?;

    if state.fcm.is_some() {
        let state = state.clone();
        let channel = channel.to_string();
        let title = title.to_string();
        let body = body.to_string();
        let url = url.map(|s| s.to_string());
        tokio::spawn(async move {
            if let Err(e) = fanout(&state, &channel, &title, &body, url.as_deref()).await {
                tracing::warn!("push fanout failed: {e:#}");
            }
        });
    }
    Ok(())
}

/// Send `channel`'s notification to every subscribed device, pruning any token
/// FCM reports as dead.
async fn fanout(
    state: &AppState,
    channel: &str,
    title: &str,
    body: &str,
    url: Option<&str>,
) -> anyhow::Result<()> {
    let Some(fcm) = &state.fcm else { return Ok(()) };

    let tokens: Vec<PushTokenDoc> = state.db
        .fluent()
        .select()
        .from(PUSH_TOKENS)
        .obj()
        .query()
        .await
        .context("failed to load push tokens")?;
    if tokens.is_empty() {
        return Ok(());
    }

    let prefs: Vec<NotifPrefsDoc> = state.db
        .fluent()
        .select()
        .from(NOTIF_PREFS)
        .obj()
        .query()
        .await
        .context("failed to load notif prefs")?;
    let prefs: HashMap<String, NotifPrefsDoc> =
        prefs.into_iter().map(|p| (p.user_id.clone(), p)).collect();

    for t in tokens {
        // No prefs doc → all channels on by default.
        let enabled = match prefs.get(&t.user_id) {
            Some(p) => prefs_for(channel, p),
            None => true,
        };
        if !enabled {
            continue;
        }
        match fcm.send(&t.token, title, body, url).await {
            Ok(SendOutcome::Sent) => {}
            Ok(SendOutcome::Stale) => {
                let _ = state.db
                    .fluent()
                    .delete()
                    .from(PUSH_TOKENS)
                    .document_id(&t.token)
                    .execute()
                    .await;
            }
            Err(e) => tracing::warn!("FCM send error: {e:#}"),
        }
    }
    Ok(())
}
