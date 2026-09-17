//! Scoped permission grants ("RBAC-lite") — separate from the superadmin-only
//! `/users` role-tier panel. Deliberately lighter data (no email, no disabled
//! status, no device count) and admin-reachable rather than superadmin-only:
//! any admin should be able to hand a member a scoped capability like
//! `book_club_manager` without needing the full account-management panel.

use crate::{
    AppState,
    auth::require_admin,
    error::AppResult,
    models::{ProfileDoc, UserDoc},
};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{MemberPermissions, UpdateMemberPermissionRequest};

const USERS: &str = "users";
const PROFILES: &str = "profiles";

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, put};
    axum::Router::new()
        .route("/", get(list_roles))
        .route("/{id}", put(update_permission))
}

async fn display_name(state: &AppState, user_id: &str) -> AppResult<Option<String>> {
    let profile: Option<ProfileDoc> = state.db
        .fluent()
        .select()
        .by_id_in(PROFILES)
        .obj()
        .one(user_id)
        .await
        .context("failed to load member profile")?;
    Ok(profile.and_then(|p| p.display_name).filter(|s| !s.is_empty()))
}

async fn list_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<MemberPermissions>>> {
    require_admin(&state, &headers).await?;

    let mut docs: Vec<UserDoc> = state.db
        .fluent()
        .select()
        .from(USERS)
        .filter(|q| q.field("disabled").eq(false))
        .obj()
        .query()
        .await
        .context("failed to list users")?;
    docs.sort_by(|a, b| a.username.to_lowercase().cmp(&b.username.to_lowercase()));

    let mut out = Vec::with_capacity(docs.len());
    for d in docs {
        let display_name = display_name(&state, &d.id).await?;
        out.push(MemberPermissions {
            user_id: d.id,
            username: d.username,
            display_name,
            permissions: d.permissions,
        });
    }
    Ok(Json(out))
}

/// Grant or revoke one permission key on one member. No self-lockout guard —
/// unlike a role-tier change, adding or removing a scoped grant can't lock
/// anyone out of the account itself.
async fn update_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateMemberPermissionRequest>,
) -> AppResult<Json<MemberPermissions>> {
    require_admin(&state, &headers).await?;

    let existing: Option<UserDoc> = state.db
        .fluent()
        .select()
        .by_id_in(USERS)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch user")?;
    let mut existing = existing.ok_or(crate::error::AppError::NotFound)?;

    if req.granted {
        if !existing.permissions.iter().any(|p| p == &req.permission) {
            existing.permissions.push(req.permission.clone());
        }
    } else {
        existing.permissions.retain(|p| p != &req.permission);
    }

    let updated = existing;
    let _: UserDoc = state.db
        .fluent()
        .update()
        .in_col(USERS)
        .document_id(&id)
        .object(&updated)
        .execute()
        .await
        .context("failed to update permissions")?;

    let display_name = display_name(&state, &id).await?;
    Ok(Json(MemberPermissions {
        user_id: updated.id,
        username: updated.username,
        display_name,
        permissions: updated.permissions,
    }))
}
