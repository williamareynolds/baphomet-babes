use crate::{
    AppState,
    auth::create_token,
    error::{AppError, AppResult},
    models::UserRecord,
};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Json, extract::State};
use libsql::params;
use shared::{AuthResponse, LoginRequest, RegisterRequest, UserInfo};
use uuid::Uuid;

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
    let conn = state.db.connect()?;
    let mut rows = conn
        .query(
            "SELECT id, email, username, password_hash, role FROM users WHERE email = ?1",
            params![req.email.clone()],
        )
        .await?;

    let row = rows
        .next()
        .await?
        .ok_or_else(|| AppError::Auth("invalid credentials".into()))?;

    let user = UserRecord {
        id: row.get(0)?,
        email: row.get(1)?,
        username: row.get(2)?,
        password_hash: row.get(3)?,
        role: row.get(4)?,
    };

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
        },
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

    let conn = state.db.connect()?;
    conn.execute(
        "INSERT INTO users (id, email, username, password_hash, role, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id.clone(), req.email.clone(), req.username.clone(), hash, role, now],
    )
    .await
    .map_err(|_| AppError::BadRequest("email already registered".into()))?;

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
