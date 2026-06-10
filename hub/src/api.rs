use shared::{AuthResponse, LoginRequest};

#[cfg(feature = "production")]
pub const API_BASE: &str = "https://movie-night-api-r6vuubbgla-uc.a.run.app";

#[cfg(not(feature = "production"))]
pub const API_BASE: &str = "http://localhost:8080";

async fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
    path: &str,
    body: &B,
    token: Option<&str>,
) -> Result<T, String> {
    let mut req = gloo_net::http::Request::post(&format!("{API_BASE}{path}"))
        .header("Content-Type", "application/json");
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let resp = req
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        let err: shared::ErrorResponse = resp
            .json()
            .await
            .unwrap_or(shared::ErrorResponse { error: "unknown error".into() });
        return Err(err.error);
    }
    resp.json().await.map_err(|e| e.to_string())
}

pub async fn login(req: LoginRequest) -> Result<AuthResponse, String> {
    post_json("/auth/login", &req, None).await
}
