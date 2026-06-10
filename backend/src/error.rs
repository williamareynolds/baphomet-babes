use axum::{http::StatusCode, response::IntoResponse, Json};
use shared::ErrorResponse;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    Auth(String),
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    BadRequest(String),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            AppError::Auth(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Internal(e) => {
                // {:#} prints the full anyhow context chain on one line
                tracing::error!("internal error: {e:#}");
                let msg = if cfg!(debug_assertions) {
                    format!("internal error: {e:#}")
                } else {
                    "internal server error".to_string()
                };
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };
        if status != StatusCode::INTERNAL_SERVER_ERROR {
            tracing::debug!(%status, "request error: {msg}");
        }
        (status, Json(ErrorResponse { error: msg })).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    async fn body_json(err: AppError) -> (StatusCode, serde_json::Value) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn auth_maps_to_401_json() {
        let (status, body) = body_json(AppError::Auth("invalid credentials".into())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "invalid credentials");
    }

    #[tokio::test]
    async fn not_found_maps_to_404_json() {
        let (status, body) = body_json(AppError::NotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "not found");
    }

    #[tokio::test]
    async fn forbidden_maps_to_403_json() {
        let (status, body) = body_json(AppError::Forbidden).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"], "forbidden");
    }

    #[tokio::test]
    async fn bad_request_maps_to_400_json() {
        let (status, body) = body_json(AppError::BadRequest("bad input".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "bad input");
    }

    #[tokio::test]
    async fn internal_maps_to_500_json() {
        let err = anyhow::anyhow!("root cause").context("failed to do thing");
        let (status, body) = body_json(AppError::Internal(err)).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        let msg = body["error"].as_str().unwrap();
        if cfg!(debug_assertions) {
            // dev builds expose the context chain for debugging
            assert!(msg.contains("failed to do thing"), "got: {msg}");
            assert!(msg.contains("root cause"), "got: {msg}");
        } else {
            assert_eq!(msg, "internal server error");
        }
    }
}
