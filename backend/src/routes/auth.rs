use crate::{AppState, auth::create_token, error::{AppError, AppResult}, models::UserDoc};
use anyhow::Context;
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Json, extract::State};
use shared::{AuthResponse, LoginRequest, RegisterRequest, UserInfo};
use uuid::Uuid;

const USERS: &str = "users";

pub fn router() -> axum::Router<AppState> {
    use axum::routing::post;
    axum::Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
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

    let parsed = PasswordHash::new(&user.password_hash)
        .map_err(|_| AppError::Auth("invalid credentials".into()))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed)
        .map_err(|_| AppError::Auth("invalid credentials".into()))?;

    let token = create_token(&user.id, &user.role, &state.jwt_secret)?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo { id: user.id, email: user.email, username: user.username, role: user.role },
    }))
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<AuthResponse>> {
    let role = match req.invite_code.as_str() {
        c if c == state.admin_invite_code => "admin",
        c if c == state.member_invite_code => "member",
        _ => return Err(AppError::Auth("invalid invite code".into())),
    };

    // Check email not already taken
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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let user_doc = UserDoc {
        id: id.clone(),
        email: req.email.clone(),
        username: req.username.clone(),
        password_hash: hash,
        role: role.to_string(),
        created_at: now,
    };

    let _: UserDoc = state.db
        .fluent()
        .insert()
        .into(USERS)
        .document_id(&id)
        .object(&user_doc)
        .execute()
        .await
        .context("failed to create user")?;

    let token = create_token(&id, role, &state.jwt_secret)?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id,
            email: req.email,
            username: req.username,
            role: role.to_string(),
        },
    }))
}
