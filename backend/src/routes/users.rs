use crate::{AppState, auth::require_superadmin, error::{AppError, AppResult}, models::UserDoc};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{UpdateUserRequest, UserSummary};

const USERS: &str = "users";

/// Mounted at /users — superadmin-only control panel.
pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, put};
    axum::Router::new()
        .route("/", get(list_users))
        .route("/{id}", put(update_user))
}

fn doc_to_summary(d: UserDoc) -> UserSummary {
    UserSummary {
        id: d.id,
        email: d.email,
        username: d.username,
        role: d.role,
        disabled: d.disabled,
        created_at: d.created_at,
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

    let mut users: Vec<UserSummary> = docs.into_iter().map(doc_to_summary).collect();
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

    Ok(Json(doc_to_summary(updated)))
}
