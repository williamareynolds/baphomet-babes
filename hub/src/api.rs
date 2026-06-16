use shared::{AuthResponse, LoginRequest, RegisterRequest, Profile, UpdateProfileRequest};

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
        let err: shared::ErrorResponse = resp.json().await
            .unwrap_or(shared::ErrorResponse { error: "unknown error".into() });
        return Err(err.error);
    }
    resp.json().await.map_err(|e| e.to_string())
}

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
        let err: shared::ErrorResponse = resp.json().await
            .unwrap_or(shared::ErrorResponse { error: "unknown error".into() });
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
        let err: shared::ErrorResponse = resp.json().await
            .unwrap_or(shared::ErrorResponse { error: "unknown error".into() });
        return Err(err.error);
    }
    resp.json().await.map_err(|e| e.to_string())
}

pub async fn login(req: LoginRequest) -> Result<AuthResponse, String> {
    post_json("/auth/login", &req, None).await
}

pub async fn register(req: RegisterRequest) -> Result<AuthResponse, String> {
    post_json("/auth/register", &req, None).await
}

pub async fn get_my_profile(token: &str) -> Result<Profile, String> {
    get("/profile/me", token).await
}

pub async fn update_my_profile(req: UpdateProfileRequest, token: &str) -> Result<Profile, String> {
    put_json("/profile/me", &req, token).await
}

pub async fn list_members(token: &str) -> Result<Vec<Profile>, String> {
    get("/members", token).await
}

pub async fn get_member(id: &str, token: &str) -> Result<Profile, String> {
    get(&format!("/members/{id}"), token).await
}
