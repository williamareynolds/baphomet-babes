use crate::{
    AppState,
    auth::require_auth,
    error::{AppError, AppResult},
    models::ProfileDoc,
};
use anyhow::Context;
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use shared::{Profile, UpdateProfileRequest};

const PROFILES: &str = "profiles";

/// Mounted at /profile
pub fn profile_router() -> axum::Router<AppState> {
    use axum::routing::get;
    axum::Router::new()
        .route("/me", get(get_my_profile).put(update_my_profile))
}

/// Mounted at /members
pub fn members_router() -> axum::Router<AppState> {
    use axum::routing::get;
    axum::Router::new()
        .route("/", get(list_members))
        .route("/{id}", get(get_member).put(admin_update_profile))
}

fn doc_to_profile(d: ProfileDoc) -> Profile {
    Profile {
        user_id: d.user_id,
        username: d.username,
        display_name: d.display_name,
        bio: d.bio,
        pronouns: d.pronouns,
        avatar_url: d.avatar_url,
        email: d.email,
        phone: d.phone,
        links: d.links,
        is_public: d.is_public,
        updated_at: d.updated_at,
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn empty_profile(user_id: &str, username: &str) -> ProfileDoc {
    ProfileDoc {
        user_id: user_id.to_string(),
        username: username.to_string(),
        display_name: None,
        bio: None,
        pronouns: None,
        avatar_url: None,
        email: None,
        phone: None,
        links: vec![],
        is_public: false,
        updated_at: now(),
    }
}

async fn get_my_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Profile>> {
    let claims = require_auth(&state, &headers).await?;

    let existing: Option<ProfileDoc> = state.db
        .fluent()
        .select()
        .by_id_in(PROFILES)
        .obj()
        .one(&claims.sub)
        .await
        .context("failed to fetch profile")?;

    if let Some(doc) = existing {
        return Ok(Json(doc_to_profile(doc)));
    }

    // Auto-create on first access
    let user: Option<crate::models::UserDoc> = state.db
        .fluent()
        .select()
        .by_id_in("users")
        .obj()
        .one(&claims.sub)
        .await
        .context("failed to fetch user")?;

    let username = user.map(|u| u.username).unwrap_or_default();
    let doc = empty_profile(&claims.sub, &username);

    let _: ProfileDoc = state.db
        .fluent()
        .insert()
        .into(PROFILES)
        .document_id(&claims.sub)
        .object(&doc)
        .execute()
        .await
        .context("failed to create profile")?;

    Ok(Json(doc_to_profile(doc)))
}

async fn update_my_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateProfileRequest>,
) -> AppResult<Json<Profile>> {
    let claims = require_auth(&state, &headers).await?;

    let existing: Option<ProfileDoc> = state.db
        .fluent()
        .select()
        .by_id_in(PROFILES)
        .obj()
        .one(&claims.sub)
        .await
        .context("failed to fetch profile")?;

    let existing = existing.ok_or(AppError::NotFound)?;

    let updated = ProfileDoc {
        display_name: req.display_name.or(existing.display_name),
        bio: req.bio.or(existing.bio),
        pronouns: req.pronouns.or(existing.pronouns),
        avatar_url: req.avatar_url.or(existing.avatar_url),
        email: req.email.or(existing.email),
        phone: req.phone.or(existing.phone),
        links: req.links.unwrap_or(existing.links),
        is_public: req.is_public.unwrap_or(existing.is_public),
        updated_at: now(),
        ..existing
    };

    let _: ProfileDoc = state.db
        .fluent()
        .update()
        .in_col(PROFILES)
        .document_id(&claims.sub)
        .object(&updated)
        .execute()
        .await
        .context("failed to update profile")?;

    Ok(Json(doc_to_profile(updated)))
}

async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Profile>>> {
    require_auth(&state, &headers).await?;

    let docs: Vec<ProfileDoc> = state.db
        .fluent()
        .select()
        .from(PROFILES)
        .obj()
        .query()
        .await
        .context("failed to list profiles")?;

    let mut profiles: Vec<Profile> = docs
        .into_iter()
        .filter(|d| d.is_public)
        .map(doc_to_profile)
        .collect();

    profiles.sort_by(|a, b| a.username.cmp(&b.username));
    Ok(Json(profiles))
}

async fn get_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Profile>> {
    let claims = require_auth(&state, &headers).await?;

    let doc: Option<ProfileDoc> = state.db
        .fluent()
        .select()
        .by_id_in(PROFILES)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch profile")?;

    let doc = doc.ok_or(AppError::NotFound)?;

    let is_own = claims.sub == id;
    let is_admin = claims.role == "admin" || claims.role == "superadmin";

    if !doc.is_public && !is_own && !is_admin {
        return Err(AppError::NotFound);
    }

    Ok(Json(doc_to_profile(doc)))
}

/// Admins: set is_public=false only. Superadmins: full update.
async fn admin_update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateProfileRequest>,
) -> AppResult<Json<Profile>> {
    let claims = require_auth(&state, &headers).await?;

    let is_superadmin = claims.role == "superadmin";
    let is_admin = claims.role == "admin" || is_superadmin;

    if !is_admin {
        return Err(AppError::Forbidden);
    }

    let existing: Option<ProfileDoc> = state.db
        .fluent()
        .select()
        .by_id_in(PROFILES)
        .obj()
        .one(&id)
        .await
        .context("failed to fetch profile")?;

    let existing = existing.ok_or(AppError::NotFound)?;

    let updated = if is_superadmin {
        ProfileDoc {
            display_name: req.display_name.or(existing.display_name),
            bio: req.bio.or(existing.bio),
            pronouns: req.pronouns.or(existing.pronouns),
            avatar_url: req.avatar_url.or(existing.avatar_url),
            email: req.email.or(existing.email),
            phone: req.phone.or(existing.phone),
            links: req.links.unwrap_or(existing.links),
            is_public: req.is_public.unwrap_or(existing.is_public),
            updated_at: now(),
            ..existing
        }
    } else {
        // Admin: can only make profile private, never public
        let is_public = req.is_public.unwrap_or(existing.is_public);
        if is_public && !existing.is_public {
            return Err(AppError::Forbidden);
        }
        ProfileDoc { is_public, updated_at: now(), ..existing }
    };

    let _: ProfileDoc = state.db
        .fluent()
        .update()
        .in_col(PROFILES)
        .document_id(&id)
        .object(&updated)
        .execute()
        .await
        .context("failed to update profile")?;

    Ok(Json(doc_to_profile(updated)))
}

/// Call during user registration to bootstrap an empty profile.
pub async fn create_profile_for_user(state: &AppState, user_id: &str, username: &str) -> anyhow::Result<()> {
    let doc = empty_profile(user_id, username);
    let _: ProfileDoc = state.db
        .fluent()
        .insert()
        .into(PROFILES)
        .document_id(user_id)
        .object(&doc)
        .execute()
        .await
        .context("failed to create profile on register")?;
    Ok(())
}
