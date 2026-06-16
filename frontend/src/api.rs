use shared::{AuthResponse, CreateEventRequest, CreateInviteRequest, Event, InviteCode, LoginRequest, RegisterRequest, UpdateEventRequest};

// In production, set via Trunk feature flag or env substitution
// After first deploy, either:
//   a) set this to raw Cloud Run URL: gcloud run services describe movie-night-api --region us-central1 --format 'value(status.url)'
//   b) map api.movienight.baphometbabes.com → Cloud Run (recommended, see DNS setup in README)
/// API base chosen at runtime from the page's hostname, so the URL can never be
/// baked in wrong by a build flag: any *.baphometbabes.com host uses the
/// deployed backend; everything else (localhost) uses the dev backend.
fn api_base() -> &'static str {
    let on_prod = web_sys::window()
        .and_then(|w| w.location().hostname().ok())
        .map(|h| h.ends_with("baphometbabes.com"))
        .unwrap_or(false);
    if on_prod {
        "https://movie-night-api-r6vuubbgla-uc.a.run.app"
    } else {
        "http://localhost:8080"
    }
}

/// Attach the Firebase App Check token when one is available. Absent in dev, so
/// this is a no-op there; in production every backend call carries it.
async fn attach_app_check(
    req: gloo_net::http::RequestBuilder,
) -> gloo_net::http::RequestBuilder {
    match auth_client::app_check_token().await {
        Some(t) => req.header("X-Firebase-AppCheck", &t),
        None => req,
    }
}

async fn get<T: serde::de::DeserializeOwned>(path: &str, token: &str) -> Result<T, String> {
    let req = gloo_net::http::Request::get(&format!("{}{path}", api_base()))
        .header("Authorization", &format!("Bearer {token}"));
    let resp = attach_app_check(req)
        .await
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        let err: shared::ErrorResponse = resp.json().await.unwrap_or(shared::ErrorResponse { error: "unknown error".into() });
        return Err(err.error);
    }
    resp.json().await.map_err(|e| e.to_string())
}

async fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
    path: &str,
    body: &B,
    token: Option<&str>,
) -> Result<T, String> {
    let mut req = gloo_net::http::Request::post(&format!("{}{path}", api_base()))
        .header("Content-Type", "application/json");
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let resp = attach_app_check(req)
        .await
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        let err: shared::ErrorResponse = resp.json().await.unwrap_or(shared::ErrorResponse { error: "unknown error".into() });
        return Err(err.error);
    }
    resp.json().await.map_err(|e| e.to_string())
}

#[allow(dead_code)]
async fn put_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
    path: &str,
    body: &B,
    token: &str,
) -> Result<T, String> {
    let req = gloo_net::http::Request::put(&format!("{}{path}", api_base()))
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"));
    let resp = attach_app_check(req)
        .await
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        let err: shared::ErrorResponse = resp.json().await.unwrap_or(shared::ErrorResponse { error: "unknown error".into() });
        return Err(err.error);
    }
    resp.json().await.map_err(|e| e.to_string())
}

async fn delete(path: &str, token: &str) -> Result<(), String> {
    let req = gloo_net::http::Request::delete(&format!("{}{path}", api_base()))
        .header("Authorization", &format!("Bearer {token}"));
    let resp = attach_app_check(req)
        .await
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err("delete failed".into());
    }
    Ok(())
}

pub async fn login(req: LoginRequest) -> Result<AuthResponse, String> {
    post_json("/auth/login", &req, None).await
}

pub async fn register(req: RegisterRequest) -> Result<AuthResponse, String> {
    post_json("/auth/register", &req, None).await
}

pub async fn fetch_events(token: &str) -> Result<Vec<Event>, String> {
    get("/events", token).await
}

pub async fn create_event(req: CreateEventRequest, token: &str) -> Result<Event, String> {
    post_json("/events", &req, Some(token)).await
}

pub async fn update_event(id: &str, req: UpdateEventRequest, token: &str) -> Result<Event, String> {
    put_json(&format!("/events/{id}"), &req, token).await
}

pub async fn delete_event(id: &str, token: &str) -> Result<(), String> {
    delete(&format!("/events/{id}"), token).await
}

pub async fn fetch_invites(token: &str) -> Result<Vec<InviteCode>, String> {
    get("/invites", token).await
}

pub async fn create_invite(req: CreateInviteRequest, token: &str) -> Result<InviteCode, String> {
    post_json("/invites", &req, Some(token)).await
}

pub async fn delete_invite(id: &str, token: &str) -> Result<(), String> {
    delete(&format!("/invites/{id}"), token).await
}
