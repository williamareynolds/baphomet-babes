use crate::{AppState, auth::require_superadmin, error::{AppError, AppResult}, models::{PasswordResetTokenDoc, PushTokenDoc, UserDoc}};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{UpdateUserRequest, UserSummary};
use std::collections::HashMap;
use uuid::Uuid;

const USERS: &str = "users";
const PUSH_TOKENS: &str = "push_tokens";
const PASSWORD_RESET_TOKENS: &str = "password_reset_tokens";

/// Mounted at /users — superadmin-only control panel.
pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route("/", get(list_users))
        .route("/{id}", put(update_user))
        .route("/{id}/password-reset", post(send_password_reset))
}

fn doc_to_summary(d: UserDoc, device_count: i64) -> UserSummary {
    UserSummary {
        id: d.id,
        email: d.email,
        username: d.username,
        role: d.role,
        disabled: d.disabled,
        created_at: d.created_at,
        device_count,
    }
}

async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<UserSummary>>> {
    require_superadmin(&state, &headers).await?;

    let docs: Vec<UserDoc> = state.db
        .fluent()
        .select()
        .from(USERS)
        .obj()
        .query()
        .await
        .context("failed to list users")?;

    // One scan of the push-token collection (small at our scale) yields each
    // user's enrolled-device count — one token doc per device.
    let tokens: Vec<PushTokenDoc> = state.db
        .fluent()
        .select()
        .from(PUSH_TOKENS)
        .obj()
        .query()
        .await
        .context("failed to list push tokens")?;
    let mut counts: HashMap<String, i64> = HashMap::new();
    for t in &tokens {
        *counts.entry(t.user_id.clone()).or_insert(0) += 1;
    }

    let mut users: Vec<UserSummary> = docs
        .into_iter()
        .map(|d| {
            let n = counts.get(&d.id).copied().unwrap_or(0);
            doc_to_summary(d, n)
        })
        .collect();
    users.sort_by(|a, b| a.username.to_lowercase().cmp(&b.username.to_lowercase()));
    Ok(Json(users))
}

async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> AppResult<Json<UserSummary>> {
    let claims = require_superadmin(&state, &headers).await?;

    // Guard against self-lockout: a superadmin can't demote or disable themselves.
    if claims.sub == id {
        return Err(AppError::BadRequest("cannot change your own role or status".into()));
    }

    if let Some(role) = &req.role {
        if !matches!(role.as_str(), "superadmin" | "admin" | "member") {
            return Err(AppError::BadRequest("invalid role".into()));
        }
    }

    let existing: Option<UserDoc> = state.db
        .fluent()
        .select()
        .by_id_in(USERS)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch user")?;

    let existing = existing.ok_or(AppError::NotFound)?;

    let updated = UserDoc {
        role: req.role.unwrap_or(existing.role),
        disabled: req.disabled.unwrap_or(existing.disabled),
        ..existing
    };

    let _: UserDoc = state.db
        .fluent()
        .update()
        .in_col(USERS)
        .document_id(&id)
        .object(&updated)
        .execute()
        .await
        .context("failed to update user")?;

    let tokens: Vec<PushTokenDoc> = state.db
        .fluent()
        .select()
        .from(PUSH_TOKENS)
        .filter(|q| q.field("user_id").eq(&id))
        .obj()
        .query()
        .await
        .context("failed to count push tokens")?;

    Ok(Json(doc_to_summary(updated, tokens.len() as i64)))
}

/// Mint a one-time reset link and mail it to the member. The superadmin never
/// sees or sets the new password — only the member, via the emailed link,
/// does that (see `reset_password` in `routes::auth`).
async fn send_password_reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<()>> {
    require_superadmin(&state, &headers).await?;

    let Some(email) = &state.email else {
        return Err(AppError::BadRequest("email is not configured".into()));
    };

    let user: Option<UserDoc> = state.db
        .fluent()
        .select()
        .by_id_in(USERS)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch user")?;
    let user = user.ok_or(AppError::NotFound)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let token = Uuid::new_v4().simple().to_string();

    let _: PasswordResetTokenDoc = state.db
        .fluent()
        .insert()
        .into(PASSWORD_RESET_TOKENS)
        .document_id(&token)
        .object(&PasswordResetTokenDoc {
            token: token.clone(),
            user_id: user.id.clone(),
            created_at: now,
        })
        .execute()
        .await
        .context("failed to mint reset token")?;

    let link = format!("{}/reset-password?token={token}", state.public_base_url.trim_end_matches('/'));
    let (html, text) = render_reset_email(&link);

    match email.send(&user.email, "Reset your Baphomet Babes password", &html, &text, None).await {
        Ok(crate::email::SendOutcome::Sent) => Ok(Json(())),
        Ok(crate::email::SendOutcome::Rejected(why)) => Err(AppError::BadRequest(format!("email rejected: {why}"))),
        Err(e) => Err(AppError::Internal(e)),
    }
}

fn render_reset_email(link: &str) -> (String, String) {
    let html = format!(
        r#"<!doctype html><html><body style="margin:0;padding:0;background:#12090c;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background:#12090c;padding:24px 12px;">
<tr><td align="center">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:520px;background:#1b1013;border-radius:8px;padding:28px;">
  <tr><td style="font-family:'IBM Plex Mono',monospace;font-size:11px;letter-spacing:3px;text-transform:uppercase;color:#c41e3a;padding-bottom:18px;">Baphomet Babes</td></tr>
  <tr><td style="font-family:Georgia,serif;font-size:21px;color:#f3e9ec;padding-bottom:10px;">Reset your password</td></tr>
  <tr><td style="font-family:Georgia,serif;font-size:15px;line-height:1.6;color:#d8c9ce;padding-bottom:22px;">An admin started a password reset for your account. Click below to choose a new password. This link works once and expires in an hour — if you didn't expect this, you can ignore it.</td></tr>
  <tr><td><a href="{link}" style="display:inline-block;background:#c41e3a;color:#ffffff;text-decoration:none;font-family:Georgia,serif;font-size:15px;padding:11px 22px;border-radius:4px;">Choose a new password</a></td></tr>
</table>
</td></tr></table></body></html>"#
    );
    let text = format!(
        "Reset your password\n\nAn admin started a password reset for your account. Use the link below to choose a new password. This link works once and expires in an hour — if you didn't expect this, you can ignore it.\n\n{link}\n"
    );
    (html, text)
}
