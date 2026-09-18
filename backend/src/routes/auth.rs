use crate::{AppState, auth::create_token, error::{AppError, AppResult}, models::{InviteCodeDoc, PasswordResetTokenDoc, UserDoc}};
use anyhow::Context;
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Json, extract::{Path, State}};
use shared::{AuthResponse, LoginRequest, PasswordResetInfo, RegisterRequest, ResetPasswordRequest, UserInfo};
use uuid::Uuid;

const USERS: &str = "users";
const INVITES: &str = "invite_codes";
const PASSWORD_RESET_TOKENS: &str = "password_reset_tokens";

/// How long a superadmin-issued reset link stays valid before it must be
/// re-issued. Short enough that a stale email in an inbox isn't a standing
/// risk, long enough that someone doesn't have to click it the instant it
/// lands.
const RESET_TOKEN_TTL_SECS: i64 = 3600;

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/reset-password/{token}", get(reset_link_info).post(reset_password))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let users: Vec<UserDoc> = state.db
        .fluent()
        .select()
        .from(USERS)
        .filter(|q| q.field("email").eq(&req.email))
        .obj()
        .query()
        .await
        .context("failed to query users")?;

    let user = users.into_iter().next()
        .ok_or_else(|| AppError::Auth("invalid credentials".into()))?;

    if user.disabled {
        return Err(AppError::Auth("account disabled".into()));
    }

    let parsed = PasswordHash::new(&user.password_hash)
        .map_err(|_| AppError::Auth("invalid credentials".into()))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed)
        .map_err(|_| AppError::Auth("invalid credentials".into()))?;

    let token = create_token(&user.id, &user.role, &state.jwt_secret)?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            email: user.email,
            username: user.username,
            role: user.role,
            permissions: user.permissions,
        },
    }))
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<AuthResponse>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let (role, invite_doc): (String, Option<InviteCodeDoc>) =
        if req.invite_code == state.superadmin_invite_code {
            // One-time bootstrap — reject if superadmin already exists
            let existing: Vec<UserDoc> = state.db
                .fluent()
                .select()
                .from(USERS)
                .filter(|q| q.field("role").eq("superadmin"))
                .obj()
                .query()
                .await
                .context("failed to check superadmin")?;

            if !existing.is_empty() {
                return Err(AppError::Auth("invalid invite code".into()));
            }
            ("superadmin".to_string(), None)
        } else {
            let codes: Vec<InviteCodeDoc> = state.db
                .fluent()
                .select()
                .from(INVITES)
                .filter(|q| q.field("code").eq(&req.invite_code))
                .obj()
                .query()
                .await
                .context("failed to query invite codes")?;

            let doc = codes.into_iter().next()
                .ok_or_else(|| AppError::Auth("invalid invite code".into()))?;

            if doc.used {
                return Err(AppError::Auth("invite code already used".into()));
            }
            let role = doc.role.clone();
            (role, Some(doc))
        };

    let existing: Vec<UserDoc> = state.db
        .fluent()
        .select()
        .from(USERS)
        .filter(|q| q.field("email").eq(&req.email))
        .obj()
        .query()
        .await
        .context("failed to check email")?;

    if !existing.is_empty() {
        return Err(AppError::BadRequest("email already registered".into()));
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(req.password.as_bytes(), &salt)
        .map_err(|e| AppError::BadRequest(e.to_string()))?
        .to_string();

    let id = Uuid::new_v4().to_string();

    let _: UserDoc = state.db
        .fluent()
        .insert()
        .into(USERS)
        .document_id(&id)
        .object(&UserDoc {
            id: id.clone(),
            email: req.email.clone(),
            username: req.username.clone(),
            password_hash: hash,
            role: role.clone(),
            disabled: false,
            permissions: Vec::new(),
            created_at: now,
        })
        .execute()
        .await
        .context("failed to create user")?;

    if let Some(mut doc) = invite_doc {
        let doc_id = doc.id.clone();
        doc.used = true;
        doc.used_by = Some(id.clone());
        let _: InviteCodeDoc = state.db
            .fluent()
            .update()
            .in_col(INVITES)
            .document_id(&doc_id)
            .object(&doc)
            .execute()
            .await
            .context("failed to mark invite used")?;
    }

    crate::routes::profile::create_profile_for_user(&state, &id, &req.username)
        .await
        .context("failed to create profile")?;

    let token = create_token(&id, &role, &state.jwt_secret)?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo { id, email: req.email, username: req.username, role, permissions: Vec::new() },
    }))
}

/// Look up a reset token and confirm it hasn't expired, without consuming it.
/// Shared by the `GET` (peek, for the page to render) and `POST` (consume).
async fn load_valid_reset_token(state: &AppState, token: &str) -> AppResult<PasswordResetTokenDoc> {
    let doc: Option<PasswordResetTokenDoc> = state.db
        .fluent()
        .select()
        .by_id_in(PASSWORD_RESET_TOKENS)
        .obj()
        .one(token)
        .await
        .context("failed to look up reset token")?;
    let doc = doc.ok_or_else(|| AppError::Auth("invalid or expired reset link".into()))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    if now - doc.created_at > RESET_TOKEN_TTL_SECS {
        return Err(AppError::Auth("invalid or expired reset link".into()));
    }
    Ok(doc)
}

/// `GET` half of the reset link: read-only, so mail-client link scanners
/// prefetching it can't burn the token. Tells the page which account it's
/// resetting, mainly so the password field can carry a matching
/// `autocomplete="username"` field for password managers.
async fn reset_link_info(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Json<PasswordResetInfo>> {
    let doc = load_valid_reset_token(&state, &token).await?;

    let user: Option<UserDoc> = state.db
        .fluent()
        .select()
        .by_id_in(USERS)
        .obj()
        .one(&doc.user_id)
        .await
        .context("failed to fetch user")?;
    let user = user.ok_or(AppError::NotFound)?;

    Ok(Json(PasswordResetInfo { email: user.email }))
}

/// Consume a superadmin-issued reset link: the member picks their own new
/// password here, then is logged straight in, same as `login`/`register`.
/// One-time — the token doc is deleted as soon as it's found valid, so
/// replaying a captured link never works even if the rest of the request fails.
async fn reset_password(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(req): Json<ResetPasswordRequest>,
) -> AppResult<Json<AuthResponse>> {
    if req.password.len() < 8 {
        return Err(AppError::BadRequest("password must be at least 8 characters".into()));
    }

    let doc = load_valid_reset_token(&state, &token).await?;

    state.db
        .fluent()
        .delete()
        .from(PASSWORD_RESET_TOKENS)
        .document_id(&doc.token)
        .execute()
        .await
        .context("failed to consume reset token")?;

    let user: Option<UserDoc> = state.db
        .fluent()
        .select()
        .by_id_in(USERS)
        .obj()
        .one(&doc.user_id)
        .await
        .context("failed to fetch user")?;
    let user = user.ok_or(AppError::NotFound)?;

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(req.password.as_bytes(), &salt)
        .map_err(|e| AppError::BadRequest(e.to_string()))?
        .to_string();

    let updated = UserDoc { password_hash: hash, ..user };

    let _: UserDoc = state.db
        .fluent()
        .update()
        .in_col(USERS)
        .document_id(&updated.id)
        .object(&updated)
        .execute()
        .await
        .context("failed to save new password")?;

    let token = create_token(&updated.id, &updated.role, &state.jwt_secret)?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: updated.id,
            email: updated.email,
            username: updated.username,
            role: updated.role,
            permissions: updated.permissions,
        },
    }))
}
