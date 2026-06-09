use crate::{AppState, auth::require_admin, error::{AppError, AppResult}, models::InviteCodeDoc};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{CreateInviteRequest, InviteCode};
use uuid::Uuid;

const INVITES: &str = "invite_codes";

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{delete, post};
    axum::Router::new()
        .route("/", post(create_invite).get(list_invites))
        .route("/{id}", delete(delete_invite))
}

async fn create_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateInviteRequest>,
) -> AppResult<Json<InviteCode>> {
    let claims = require_admin(&headers, &state.jwt_secret)?;

    match (claims.role.as_str(), req.role.as_str()) {
        ("superadmin", "admin" | "member") => {}
        (_, "member") => {}
        _ => return Err(AppError::Forbidden),
    }

    let id = Uuid::new_v4().to_string();
    let code = Uuid::new_v4().simple().to_string()[..12].to_uppercase();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let doc = InviteCodeDoc {
        id: id.clone(),
        code: code.clone(),
        role: req.role.clone(),
        created_by: claims.sub.clone(),
        used: false,
        used_by: None,
        created_at: now,
    };

    let _: InviteCodeDoc = state.db
        .fluent()
        .insert()
        .into(INVITES)
        .document_id(&id)
        .object(&doc)
        .execute()
        .await
        .context("failed to create invite code")?;

    Ok(Json(InviteCode {
        id,
        code,
        role: req.role,
        created_by: claims.sub,
        used: false,
        created_at: now,
    }))
}

async fn list_invites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<InviteCode>>> {
    require_admin(&headers, &state.jwt_secret)?;

    let docs: Vec<InviteCodeDoc> = state.db
        .fluent()
        .select()
        .from(INVITES)
        .obj()
        .query()
        .await
        .context("failed to list invite codes")?;

    let codes = docs.into_iter().map(|d| InviteCode {
        id: d.id,
        code: d.code,
        role: d.role,
        created_by: d.created_by,
        used: d.used,
        created_at: d.created_at,
    }).collect();

    Ok(Json(codes))
}

async fn delete_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<()>> {
    let claims = require_admin(&headers, &state.jwt_secret)?;

    let doc: Option<InviteCodeDoc> = state.db
        .fluent()
        .select()
        .by_id_in(INVITES)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch invite code")?;

    let doc = doc.ok_or(AppError::NotFound)?;

    if doc.used {
        return Err(AppError::BadRequest("cannot delete used invite code".into()));
    }

    // Admins can only delete member codes; superadmin can delete any
    if claims.role == "admin" && doc.role != "member" {
        return Err(AppError::Forbidden);
    }

    state.db
        .fluent()
        .delete()
        .from(INVITES)
        .document_id(&id)
        .execute()
        .await
        .context("failed to delete invite code")?;

    Ok(Json(()))
}
