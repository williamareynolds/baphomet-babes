pub mod app_check;
pub mod auth;
pub mod error;
pub mod models;
pub mod routes;

use std::sync::Arc;
use axum::{Router, http::{HeaderValue, Method}, response::IntoResponse, routing::get};
use firestore::FirestoreDb;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor};

use app_check::AppCheck;

#[derive(Clone)]
pub struct AppState {
    pub db: FirestoreDb,
    pub jwt_secret: String,
    pub superadmin_invite_code: String,
    /// When present, every non-health request must carry a valid App Check
    /// token. `None` disables enforcement (dev, tests, pre-rollout).
    pub app_check: Option<AppCheck>,
}

/// Rate limit knobs — relaxed in tests, strict in production.
#[derive(Clone, Copy)]
pub struct RateLimit {
    pub per_second: u64,
    pub burst: u32,
}

impl Default for RateLimit {
    fn default() -> Self {
        RateLimit { per_second: 2, burst: 8 }
    }
}

pub fn build_app(state: AppState, allowed_origins: Option<&str>, rate_limit: RateLimit) -> Router {
    let cors = if let Some(origins_str) = allowed_origins {
        let origins: Vec<HeaderValue> = origins_str
            .split(',')
            .map(|s| s.trim().parse::<HeaderValue>().expect("invalid origin in ALLOWED_ORIGINS"))
            .collect();
        tracing::info!("CORS: restricting to {} origin(s)", origins.len());
        CorsLayer::new()
            .allow_origin(AllowOrigin::predicate(move |origin, _| origins.contains(origin)))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers(tower_http::cors::Any)
    } else {
        tracing::warn!("ALLOWED_ORIGINS not set — allowing all origins (dev mode)");
        CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers(tower_http::cors::Any)
    };

    // SmartIpKeyExtractor checks x-forwarded-for / x-real-ip first (Cloud Run sits
    // behind a proxy), falling back to peer addr via ConnectInfo.
    let auth_governor = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(rate_limit.per_second)
            .burst_size(rate_limit.burst)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    let governor_layer = GovernorLayer::new(auth_governor).error_handler(|err| {
        use tower_governor::GovernorError;
        let (status, msg) = match &err {
            // The crate computes wait_time against a fresh clock, yielding
            // nonsense (~process uptime). With our config the true wait is a
            // few seconds, so don't echo the broken number.
            GovernorError::TooManyRequests { .. } => (
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "too many requests — try again in a few seconds".to_string(),
            ),
            GovernorError::UnableToExtractKey => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "rate limiter could not identify client".to_string(),
            ),
            GovernorError::Other { .. } => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "rate limiter error".to_string(),
            ),
        };
        tracing::warn!(%status, "governor: {msg}");
        (status, axum::Json(shared::ErrorResponse { error: msg })).into_response()
    });

    // App Check runs inside CORS (so preflight is handled first) but outside the
    // route handlers, gating everything except /health when enforcement is on.
    let app_check_layer =
        axum::middleware::from_fn_with_state(state.clone(), app_check::middleware);

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/auth", routes::auth::router().layer(governor_layer))
        .nest("/events", routes::events::router())
        .nest("/invites", routes::invites::router())
        .nest("/profile", routes::profile::profile_router())
        .nest("/members", routes::profile::members_router())
        .layer(app_check_layer)
        .with_state(state)
        .layer(cors)
}
