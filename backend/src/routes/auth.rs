use crate::{AppState, auth::create_token, error::{AppError, AppResult}, models::{InviteCodeDoc, UserDoc}};
use anyhow::Context;
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Json, extract::State};
use shared::{AuthResponse, LoginRequest, RegisterRequest, UserInfo};
use uuid::Uuid;

const USERS: &str = "users";
const INVITES: &str = "invite_codes";

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
        user: UserInfo { id, email: req.email, username: req.username, role },
    }))
}
