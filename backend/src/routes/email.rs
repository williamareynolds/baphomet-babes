//! Unsubscribe links for club email.
//!
//! These have to work with no session at all — someone reading mail on a device
//! that never logged into the hub still gets to opt out. Authorization is a
//! per-user capability token in the URL, the same idiom as the calendar feed
//! (see [`crate::routes::calendar`]): opaque, revocable, and scoped to one
//! thing — turning email off.
//!
//! `GET` only *offers* the unsubscribe; the `POST` performs it. That split
//! matters because Gmail and Outlook prefetch links in mail to scan them, and a
//! GET that mutated state would silently unsubscribe members who never clicked.
//! The `List-Unsubscribe-Post` header advertises the same POST, so a mail
//! client's native unsubscribe button hits the correct verb too.

use crate::{AppState, error::{AppError, AppResult}, models::{EmailTokenDoc, NotifPrefsDoc}};
use anyhow::Context;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::Html,
    routing::get,
};
use serde::Deserialize;
use uuid::Uuid;

const EMAIL_TOKENS: &str = "email_tokens";
const NOTIF_PREFS: &str = "notif_prefs";

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/unsubscribe/{token}",
        get(unsubscribe_page).post(unsubscribe),
    )
}

/// This member's unsubscribe token, minting one on first use. Called from the
/// email fan-out, so a member who has never been emailed gets a token the first
/// time they are.
pub async fn token_for(state: &AppState, user_id: &str) -> anyhow::Result<String> {
    let existing: Option<EmailTokenDoc> = state.db
        .fluent()
        .select()
        .by_id_in(EMAIL_TOKENS)
        .obj()
        .one(user_id)
        .await
        .context("failed to load email token")?;

    if let Some(d) = existing {
        return Ok(d.token);
    }

    let doc = EmailTokenDoc {
        user_id: user_id.to_string(),
        token: Uuid::new_v4().simple().to_string(),
        created_at: now(),
    };
    let _: EmailTokenDoc = state.db
        .fluent()
        .insert()
        .into(EMAIL_TOKENS)
        .document_id(user_id)
        .object(&doc)
        .execute()
        .await
        .context("failed to mint email token")?;
    Ok(doc.token)
}

/// Absolute unsubscribe URL for one member and one channel.
pub fn unsubscribe_url(base_url: &str, token: &str, channel: &str) -> String {
    format!("{}/email/unsubscribe/{token}?channel={channel}", base_url.trim_end_matches('/'))
}

#[derive(Deserialize)]
pub struct ChannelQuery {
    /// Which channel to drop. Absent means "all of them", which is what a mail
    /// client's one-click button should do when it has no channel context.
    channel: Option<String>,
}

/// Human label for a channel, used in the confirmation copy.
fn label(channel: Option<&str>) -> &'static str {
    match channel {
        Some(shared::CHANNEL_MOVIE_NIGHT) => "movie night & voting",
        Some(shared::CHANNEL_ANNOUNCEMENTS) => "announcements",
        Some(shared::CHANNEL_GENERAL) => "general",
        Some(shared::CHANNEL_MOUNTAIN_BIKE) => "mountain bike rides",
        Some(shared::CHANNEL_GATHERINGS) => "gatherings",
        _ => "all club",
    }
}

async fn user_for_token(state: &AppState, token: &str) -> AppResult<String> {
    let found: Vec<EmailTokenDoc> = state.db
        .fluent()
        .select()
        .from(EMAIL_TOKENS)
        .filter(|q| q.field("token").eq(token))
        .obj()
        .query()
        .await
        .context("failed to look up email token")?;
    found
        .into_iter()
        .next()
        .map(|d| d.user_id)
        .ok_or(AppError::NotFound)
}

/// Confirmation page. Reads nothing, changes nothing — the button POSTs.
async fn unsubscribe_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(q): Query<ChannelQuery>,
) -> AppResult<Html<String>> {
    // Resolve the token so a dead link says so up front instead of after a
    // click, but do not touch preferences here.
    user_for_token(&state, &token).await?;

    let what = label(q.channel.as_deref());
    let action = match &q.channel {
        Some(c) => format!("/email/unsubscribe/{token}?channel={c}"),
        None => format!("/email/unsubscribe/{token}"),
    };
    Ok(Html(page(&format!(
        r#"<p>Turn off <strong>{what}</strong> email for this address?</p>
           <form method="post" action="{action}">
             <button type="submit">Unsubscribe</button>
           </form>
           <p class="muted">Push notifications and everything in the app stay as they are.
           You can turn email back on any time from your profile.</p>"#
    ))))
}

/// Perform the unsubscribe. Idempotent: unsubscribing twice is a no-op.
async fn unsubscribe(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(q): Query<ChannelQuery>,
) -> AppResult<Html<String>> {
    let user_id = user_for_token(&state, &token).await?;

    let existing: Option<NotifPrefsDoc> = state.db
        .fluent()
        .select()
        .by_id_in(NOTIF_PREFS)
        .obj()
        .one(&user_id)
        .await
        .context("failed to load prefs to unsubscribe")?;

    // No prefs doc yet means the member is on defaults; write one that carries
    // the opt-out so it survives.
    let mut prefs = existing.unwrap_or(NotifPrefsDoc {
        user_id: user_id.clone(),
        announcements: true,
        general: true,
        movie_night: true,
        chat: false,
        mountain_bike: false,
        test: true,
        gatherings: true,
        cleared_at: 0,
        email_announcements: false,
        email_general: false,
        email_movie_night: true,
        email_mountain_bike: false,
        email_gatherings: true,
    });

    match q.channel.as_deref() {
        Some(shared::CHANNEL_MOVIE_NIGHT) => prefs.email_movie_night = false,
        Some(shared::CHANNEL_ANNOUNCEMENTS) => prefs.email_announcements = false,
        Some(shared::CHANNEL_GENERAL) => prefs.email_general = false,
        Some(shared::CHANNEL_MOUNTAIN_BIKE) => prefs.email_mountain_bike = false,
        Some(shared::CHANNEL_GATHERINGS) => prefs.email_gatherings = false,
        // Unknown channel, or none at all (one-click from a mail client): drop
        // every email channel. Better to over-honor an opt-out than under-honor.
        _ => {
            prefs.email_movie_night = false;
            prefs.email_announcements = false;
            prefs.email_general = false;
            prefs.email_mountain_bike = false;
            prefs.email_gatherings = false;
        }
    }

    let _: NotifPrefsDoc = state.db
        .fluent()
        .update()
        .in_col(NOTIF_PREFS)
        .document_id(&user_id)
        .object(&prefs)
        .execute()
        .await
        .context("failed to save unsubscribe")?;

    let what = label(q.channel.as_deref());
    Ok(Html(page(&format!(
        r#"<p><strong>Done.</strong> You're unsubscribed from {what} email.</p>
           <p class="muted">Changed your mind? Turn it back on under Notifications in your profile.</p>
           <p><a href="{}">Open Baphomet Babes</a></p>"#,
        state.public_base_url
    ))))
}

/// Minimal standalone page — no app shell, since whoever lands here may not be
/// logged in and may never have opened the hub.
fn page(body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Baphomet Babes — Email</title>
<style>
  body {{ margin:0; min-height:100vh; display:flex; align-items:center; justify-content:center;
         background:#12090c; color:#f3e9ec; font-family:Georgia,'Times New Roman',serif; padding:1.5rem; }}
  main {{ max-width:32rem; }}
  h1 {{ font-size:1.1rem; letter-spacing:0.18em; text-transform:uppercase; color:#c41e3a;
        font-family:'IBM Plex Mono',ui-monospace,monospace; margin:0 0 1.25rem; }}
  p {{ line-height:1.6; }}
  .muted {{ color:#ad9ea4; font-size:0.9rem; }}
  button {{ background:#c41e3a; color:#fff; border:0; border-radius:0.25rem; cursor:pointer;
            padding:0.7rem 1.4rem; font-size:1rem; margin:0.5rem 0 1rem; }}
  a {{ color:#c41e3a; }}
</style>
</head><body><main><h1>Baphomet Babes</h1>{body}</main></body></html>"#
    )
}
