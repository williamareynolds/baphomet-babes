//! Notifications: the inbox feed, per-user channel preferences, FCM device-token
//! registration, and the admin broadcast tool.
//!
//! Every notification is persisted (so members can browse an inbox) and, when
//! FCM is configured, pushed to subscribed devices. Pushing is best-effort and
//! happens in the background so it never blocks or fails the originating action.

use std::collections::{HashMap, HashSet};

use crate::{
    AppState,
    auth::{require_admin, require_auth},
    error::{AppError, AppResult},
    fcm::SendOutcome,
    models::{NotifPrefsDoc, NotificationDoc, PushTokenDoc, UserDoc},
};
use anyhow::Context;
use axum::{Json, extract::State, http::HeaderMap};
use shared::{
    BroadcastRequest, CHANNEL_GENERAL, CHANNEL_TEST, Notification, NotificationPrefs,
    RegisterPushTokenRequest, TestPushResponse, UpdateNotificationPrefs,
};
use uuid::Uuid;

const NOTIFICATIONS: &str = "notifications";
const PUSH_TOKENS: &str = "push_tokens";
const NOTIF_PREFS: &str = "notif_prefs";
/// Most recent notifications retained in the inbox view.
const FEED_LIMIT: usize = 30;

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route("/", get(list_feed))
        .route("/clear", post(clear_feed))
        .route("/token", put(register_token).delete(unregister_token))
        .route("/prefs", get(get_prefs).put(update_prefs))
        .route("/broadcast", post(broadcast))
        .route("/test", post(test_push))
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

/// Default subscription for a member with no saved prefs, derived from
/// `NotificationPrefs::default()` (announcements/general/movie on, chat off).
fn channel_default(channel: &str) -> bool {
    let d = NotificationPrefs::default();
    match channel {
        shared::CHANNEL_ANNOUNCEMENTS => d.announcements,
        shared::CHANNEL_GENERAL => d.general,
        shared::CHANNEL_MOVIE_NIGHT => d.movie_night,
        shared::CHANNEL_CHAT => d.chat,
        shared::CHANNEL_MOUNTAIN_BIKE => d.mountain_bike,
        shared::CHANNEL_TEST => d.test,
        _ => false,
    }
}

fn prefs_for(channel: &str, p: &NotifPrefsDoc) -> bool {
    match channel {
        shared::CHANNEL_ANNOUNCEMENTS => p.announcements,
        shared::CHANNEL_GENERAL => p.general,
        shared::CHANNEL_MOVIE_NIGHT => p.movie_night,
        shared::CHANNEL_CHAT => p.chat,
        shared::CHANNEL_MOUNTAIN_BIKE => p.mountain_bike,
        shared::CHANNEL_TEST => p.test,
        _ => false,
    }
}

// ---- inbox feed ----

async fn list_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Notification>>> {
    let claims = require_auth(&state, &headers).await?;
    let cleared_at = load_prefs(&state, &claims.sub).await?.cleared_at;

    let mut docs: Vec<NotificationDoc> = state.db
        .fluent()
        .select()
        .from(NOTIFICATIONS)
        .obj()
        .query()
        .await
        .context("failed to list notifications")?;

    docs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let feed: Vec<Notification> = docs
        .into_iter()
        .filter(|d| d.created_at > cleared_at)
        .take(FEED_LIMIT)
        .map(doc_to_notification)
        .collect();
    Ok(Json(feed))
}

/// Clear the caller's inbox: advance their watermark to now, hiding everything
/// up to this moment. Shared notification records are untouched (other members
/// keep theirs); new notifications after this still appear.
async fn clear_feed(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<()> {
    let claims = require_auth(&state, &headers).await?;
    let existing = load_prefs(&state, &claims.sub).await?;
    let updated = NotifPrefsDoc { cleared_at: now(), ..existing };

    let _: NotifPrefsDoc = state.db
        .fluent()
        .update()
        .in_col(NOTIF_PREFS)
        .document_id(&claims.sub)
        .object(&updated)
        .execute()
        .await
        .context("failed to clear notifications")?;
    Ok(())
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
        chat: false,
        mountain_bike: false,
        test: true,
        cleared_at: 0,
        email_announcements: false,
        email_general: false,
        email_movie_night: true,
        email_mountain_bike: false,
    }))
}

/// The stored flags as the API's nested shape.
fn email_prefs(p: &NotifPrefsDoc) -> shared::EmailPrefs {
    shared::EmailPrefs {
        announcements: p.email_announcements,
        general: p.email_general,
        movie_night: p.email_movie_night,
        mountain_bike: p.email_mountain_bike,
    }
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
        chat: p.chat,
        mountain_bike: p.mountain_bike,
        test: p.test,
        email: email_prefs(&p),
    }))
}

async fn update_prefs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateNotificationPrefs>,
) -> AppResult<Json<NotificationPrefs>> {
    let claims = require_auth(&state, &headers).await?;
    let existing = load_prefs(&state, &claims.sub).await?;

    // Email flags arrive nested and partial: an absent `email` object leaves
    // every email setting alone, exactly like an absent push flag does.
    let e = req.email.unwrap_or_default();
    let updated = NotifPrefsDoc {
        user_id: claims.sub.clone(),
        announcements: req.announcements.unwrap_or(existing.announcements),
        general: req.general.unwrap_or(existing.general),
        movie_night: req.movie_night.unwrap_or(existing.movie_night),
        chat: req.chat.unwrap_or(existing.chat),
        mountain_bike: req.mountain_bike.unwrap_or(existing.mountain_bike),
        test: req.test.unwrap_or(existing.test),
        cleared_at: existing.cleared_at,
        email_announcements: e.announcements.unwrap_or(existing.email_announcements),
        email_general: e.general.unwrap_or(existing.email_general),
        email_movie_night: e.movie_night.unwrap_or(existing.email_movie_night),
        email_mountain_bike: e.mountain_bike.unwrap_or(existing.email_mountain_bike),
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
        chat: updated.chat,
        mountain_bike: updated.mountain_bike,
        test: updated.test,
        email: email_prefs(&updated),
    }))
}

// ---- admin broadcast (General or Test channel) ----

async fn broadcast(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BroadcastRequest>,
) -> AppResult<()> {
    require_admin(&state, &headers).await?;
    if req.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }
    match req.channel.as_deref().unwrap_or(CHANNEL_GENERAL) {
        CHANNEL_GENERAL => {
            dispatch(&state, CHANNEL_GENERAL, &req.title, &req.body, Some("/notifications"), None)
                .await?;
        }
        // Test broadcasts exercise the push pipeline only: no inbox entry, and
        // the fanout delivers solely to admins/superadmins.
        CHANNEL_TEST => {
            push_only(&state, CHANNEL_TEST, &req.title, &req.body, Some("/notifications"), None);
        }
        other => {
            return Err(AppError::BadRequest(format!("unknown broadcast channel: {other}")));
        }
    }
    Ok(())
}

// ---- self-serve test push ----

/// Send a test notification to the caller's own devices, synchronously, and
/// report exactly what happened. This is the end-to-end probe members (and we)
/// use to verify the delivery path — it bypasses channel prefs on purpose:
/// asking for a test IS the opt-in.
async fn test_push(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<TestPushResponse>> {
    let claims = require_auth(&state, &headers).await?;

    let tokens: Vec<PushTokenDoc> = state.db
        .fluent()
        .select()
        .from(PUSH_TOKENS)
        .obj()
        .query()
        .await
        .context("failed to load push tokens")?;
    let mine: Vec<PushTokenDoc> =
        tokens.into_iter().filter(|t| t.user_id == claims.sub).collect();
    let devices = mine.len();

    let Some(fcm) = &state.fcm else {
        return Ok(Json(TestPushResponse {
            devices,
            sent: 0,
            detail: Some("push is disabled on this server".into()),
        }));
    };

    let mut sent = 0usize;
    let mut detail: Option<String> = None;
    for t in mine {
        match fcm
            .send(
                &t.token,
                "Test notification",
                "Push notifications are working on this device. 🤘",
                Some("/profile"),
            )
            .await
        {
            Ok(SendOutcome::Sent) => sent += 1,
            Ok(SendOutcome::Stale) => {
                detail = Some("a stale device registration was removed; re-enable push on that device".into());
                let _ = state.db
                    .fluent()
                    .delete()
                    .from(PUSH_TOKENS)
                    .document_id(&t.token)
                    .execute()
                    .await;
            }
            Err(e) => {
                tracing::warn!("test push failed: {e:#}");
                detail = Some(format!("send failed: {e}"));
            }
        }
    }
    tracing::info!("test push user={} devices={devices} sent={sent}", claims.sub);
    Ok(Json(TestPushResponse { devices, sent, detail }))
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
    exclude_user: Option<&str>,
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
        let exclude_user = exclude_user.map(|s| s.to_string());
        tokio::spawn(async move {
            if let Err(e) = fanout(&state, &channel, &title, &body, url.as_deref(), exclude_user.as_deref()).await {
                tracing::warn!("push fanout failed: {e:#}");
            }
        });
    }

    // Email rides alongside push rather than instead of it: the two have
    // separate per-channel preferences, so a member can take a channel by
    // either, both, or neither. Backgrounded and best-effort for the same
    // reason push is — a slow mail API must not fail the action that triggered
    // the notification.
    if state.email.is_some() {
        let state = state.clone();
        let channel = channel.to_string();
        let title = title.to_string();
        let body = body.to_string();
        let url = url.map(|s| s.to_string());
        let exclude_user = exclude_user.map(|s| s.to_string());
        tokio::spawn(async move {
            if let Err(e) = email_fanout(&state, &channel, &title, &body, url.as_deref(), exclude_user.as_deref()).await {
                tracing::warn!("email fanout failed: {e:#}");
            }
        });
    }
    Ok(())
}

/// Email `channel`'s notification to every member who wants that channel by
/// mail. Each message carries its own unsubscribe link, so this sends one
/// request per recipient.
pub(crate) async fn email_fanout(
    state: &AppState,
    channel: &str,
    title: &str,
    body: &str,
    url: Option<&str>,
    exclude_user: Option<&str>,
) -> anyhow::Result<()> {
    let Some(email) = &state.email else { return Ok(()) };

    let users: Vec<UserDoc> = state.db
        .fluent()
        .select()
        .from("users")
        .obj()
        .query()
        .await
        .context("failed to load users for email fanout")?;

    let prefs: Vec<NotifPrefsDoc> = state.db
        .fluent()
        .select()
        .from(NOTIF_PREFS)
        .obj()
        .query()
        .await
        .context("failed to load notif prefs for email fanout")?;
    let prefs: HashMap<String, NotifPrefsDoc> =
        prefs.into_iter().map(|p| (p.user_id.clone(), p)).collect();

    let (mut sent, mut rejected, mut failed) = (0usize, 0usize, 0usize);
    for u in users {
        if u.disabled || u.email.is_empty() {
            continue;
        }
        if exclude_user == Some(u.id.as_str()) {
            continue;
        }
        // No prefs doc means defaults, which is how a member who has never
        // touched settings still gets the movie-night nudge.
        let wanted = prefs
            .get(&u.id)
            .map(|p| shared::EmailPrefs {
                announcements: p.email_announcements,
                general: p.email_general,
                movie_night: p.email_movie_night,
                mountain_bike: p.email_mountain_bike,
            })
            .unwrap_or_default();
        if !wanted.allows(channel) {
            continue;
        }

        let token = match crate::routes::email::token_for(state, &u.id).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("no unsubscribe token for {}: {e:#}", u.id);
                failed += 1;
                continue;
            }
        };
        let unsub = crate::routes::email::unsubscribe_url(&state.public_base_url, &token, channel);
        let (html, text) = render(&state.public_base_url, title, body, url, &unsub);

        match email.send(&u.email, title, &html, &text, Some(&unsub)).await {
            Ok(crate::email::SendOutcome::Sent) => sent += 1,
            Ok(crate::email::SendOutcome::Rejected(why)) => {
                rejected += 1;
                tracing::warn!("email rejected for {}: {why}", u.id);
            }
            Err(e) => {
                failed += 1;
                tracing::warn!("email send error: {e:#}");
            }
        }
    }
    tracing::info!("email fanout channel={channel} sent={sent} rejected={rejected} failed={failed}");
    Ok(())
}

/// Render one notification as (html, text).
///
/// `url` is a hub-relative path on the notification (e.g. `/movie-nights`);
/// mail needs it absolute, so it's joined onto the public base URL here.
pub(crate) fn render(
    base_url: &str,
    title: &str,
    body: &str,
    url: Option<&str>,
    unsubscribe_url: &str,
) -> (String, String) {
    let base = base_url.trim_end_matches('/');
    let link = url.map(|u| format!("{base}{u}")).unwrap_or_else(|| base.to_string());

    let html = format!(
        r#"<!doctype html><html><body style="margin:0;padding:0;background:#12090c;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background:#12090c;padding:24px 12px;">
<tr><td align="center">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:520px;background:#1b1013;border-radius:8px;padding:28px;">
  <tr><td style="font-family:'IBM Plex Mono',monospace;font-size:11px;letter-spacing:3px;text-transform:uppercase;color:#c41e3a;padding-bottom:18px;">Baphomet Babes</td></tr>
  <tr><td style="font-family:Georgia,serif;font-size:21px;color:#f3e9ec;padding-bottom:10px;">{title}</td></tr>
  <tr><td style="font-family:Georgia,serif;font-size:15px;line-height:1.6;color:#d8c9ce;padding-bottom:22px;">{body}</td></tr>
  <tr><td><a href="{link}" style="display:inline-block;background:#c41e3a;color:#ffffff;text-decoration:none;font-family:Georgia,serif;font-size:15px;padding:11px 22px;border-radius:4px;">Open the app</a></td></tr>
  <tr><td style="font-family:Georgia,serif;font-size:12px;color:#7d6d72;padding-top:26px;">
    <a href="{unsubscribe_url}" style="color:#7d6d72;">Unsubscribe from these emails</a>
  </td></tr>
</table>
</td></tr></table></body></html>"#
    );

    let text = format!(
        "{title}\n\n{body}\n\n{link}\n\n—\nUnsubscribe: {unsubscribe_url}\n"
    );

    (html, text)
}

/// Push a notification to subscribed devices WITHOUT persisting it to the inbox.
/// Used for high-volume sources (group chat) that would otherwise flood the
/// capped feed — the chat page is its own history. Best-effort and backgrounded.
pub fn push_only(
    state: &AppState,
    channel: &str,
    title: &str,
    body: &str,
    url: Option<&str>,
    exclude_user: Option<&str>,
) {
    if state.fcm.is_none() {
        return;
    }
    let state = state.clone();
    let channel = channel.to_string();
    let title = title.to_string();
    let body = body.to_string();
    let url = url.map(|s| s.to_string());
    let exclude_user = exclude_user.map(|s| s.to_string());
    tokio::spawn(async move {
        if let Err(e) = fanout(&state, &channel, &title, &body, url.as_deref(), exclude_user.as_deref()).await {
            tracing::warn!("push fanout failed: {e:#}");
        }
    });
}

/// Push directly to specific members' devices, bypassing channel preferences —
/// for attendee-scoped updates (someone who joined a ride implicitly opted into
/// hearing about it). Not persisted to the inbox: the shared feed has no
/// per-user targeting, so persisting would show it to everyone. Best-effort
/// and backgrounded, like the channel fanout.
pub fn push_to_users(
    state: &AppState,
    user_ids: Vec<String>,
    title: &str,
    body: &str,
    url: Option<&str>,
) {
    if state.fcm.is_none() || user_ids.is_empty() {
        return;
    }
    let state = state.clone();
    let title = title.to_string();
    let body = body.to_string();
    let url = url.map(|s| s.to_string());
    tokio::spawn(async move {
        if let Err(e) = fanout_users(&state, &user_ids, &title, &body, url.as_deref()).await {
            tracing::warn!("targeted push failed: {e:#}");
        }
    });
}

/// Send to every device belonging to one of `user_ids`, pruning dead tokens.
async fn fanout_users(
    state: &AppState,
    user_ids: &[String],
    title: &str,
    body: &str,
    url: Option<&str>,
) -> anyhow::Result<()> {
    let Some(fcm) = &state.fcm else { return Ok(()) };

    let targets: HashSet<&str> = user_ids.iter().map(|s| s.as_str()).collect();
    let tokens: Vec<PushTokenDoc> = state.db
        .fluent()
        .select()
        .from(PUSH_TOKENS)
        .obj()
        .query()
        .await
        .context("failed to load push tokens")?;

    let (mut sent, mut stale, mut failed) = (0usize, 0usize, 0usize);
    for t in tokens {
        if !targets.contains(t.user_id.as_str()) {
            continue;
        }
        match fcm.send(&t.token, title, body, url).await {
            Ok(SendOutcome::Sent) => sent += 1,
            Ok(SendOutcome::Stale) => {
                stale += 1;
                let _ = state.db
                    .fluent()
                    .delete()
                    .from(PUSH_TOKENS)
                    .document_id(&t.token)
                    .execute()
                    .await;
            }
            Err(e) => {
                failed += 1;
                tracing::warn!("FCM send error: {e:#}");
            }
        }
    }
    tracing::info!(
        "targeted push users={} sent={sent} stale={stale} failed={failed}",
        user_ids.len()
    );
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
    exclude_user: Option<&str>,
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

    // The test channel is restricted by role, not just preference: only
    // admin/superadmin devices may receive it, whatever their prefs say.
    let admin_only: Option<HashSet<String>> = if channel == CHANNEL_TEST {
        let users: Vec<UserDoc> = state.db
            .fluent()
            .select()
            .from("users")
            .obj()
            .query()
            .await
            .context("failed to load users for test-channel fanout")?;
        Some(
            users
                .into_iter()
                .filter(|u| u.role == "admin" || u.role == "superadmin")
                .map(|u| u.id)
                .collect(),
        )
    } else {
        None
    };

    let total = tokens.len();
    let (mut sent, mut stale, mut failed, mut skipped) = (0usize, 0usize, 0usize, 0usize);
    for t in tokens {
        // Don't push a message back to its own author's devices.
        if exclude_user == Some(t.user_id.as_str()) {
            skipped += 1;
            continue;
        }
        if let Some(allowed) = &admin_only {
            if !allowed.contains(&t.user_id) {
                skipped += 1;
                continue;
            }
        }
        // No prefs doc → fall back to the per-channel defaults (chat off, the
        // rest on), so an unsaved member isn't pushed every chat message.
        let enabled = match prefs.get(&t.user_id) {
            Some(p) => prefs_for(channel, p),
            None => channel_default(channel),
        };
        if !enabled {
            skipped += 1;
            continue;
        }
        match fcm.send(&t.token, title, body, url).await {
            Ok(SendOutcome::Sent) => sent += 1,
            Ok(SendOutcome::Stale) => {
                stale += 1;
                let _ = state.db
                    .fluent()
                    .delete()
                    .from(PUSH_TOKENS)
                    .document_id(&t.token)
                    .execute()
                    .await;
            }
            Err(e) => {
                failed += 1;
                tracing::warn!("FCM send error: {e:#}");
            }
        }
    }
    tracing::info!(
        "push fanout channel={channel} tokens={total} sent={sent} stale={stale} failed={failed} skipped={skipped}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://baphometbabes.com";
    const UNSUB: &str = "https://baphometbabes.com/email/unsubscribe/abc123?channel=movie_night";

    #[test]
    fn render_makes_the_deep_link_absolute() {
        // The notification carries a hub-relative path; an inbox can't resolve
        // that, so it has to be joined onto the public base URL.
        let (html, text) = render(BASE, "Last call to vote", "Voting closes Friday.", Some("/movie-nights"), UNSUB);
        assert!(html.contains("https://baphometbabes.com/movie-nights"), "{html}");
        assert!(text.contains("https://baphometbabes.com/movie-nights"), "{text}");
    }

    #[test]
    fn render_falls_back_to_the_site_root_without_a_path() {
        let (html, text) = render(BASE, "Title", "Body", None, UNSUB);
        assert!(html.contains(r#"href="https://baphometbabes.com""#), "{html}");
        assert!(text.contains("https://baphometbabes.com"), "{text}");
    }

    #[test]
    fn render_tolerates_a_trailing_slash_on_the_base() {
        // Otherwise the link comes out as ".com//movie-nights".
        let (html, _) = render("https://baphometbabes.com/", "T", "B", Some("/movie-nights"), UNSUB);
        assert!(html.contains("https://baphometbabes.com/movie-nights"), "{html}");
        assert!(!html.contains(".com//movie-nights"), "{html}");
    }

    #[test]
    fn render_carries_the_unsubscribe_link_in_both_parts() {
        // A member reading the plain-text alternative still needs a way out.
        let (html, text) = render(BASE, "Title", "Body", Some("/x"), UNSUB);
        assert!(html.contains(UNSUB), "{html}");
        assert!(text.contains(UNSUB), "{text}");
    }

    #[test]
    fn render_includes_title_and_body() {
        let (html, text) = render(BASE, "Last call to vote", "Voting closes Friday.", None, UNSUB);
        for part in [&html, &text] {
            assert!(part.contains("Last call to vote"), "{part}");
            assert!(part.contains("Voting closes Friday."), "{part}");
        }
    }
}
