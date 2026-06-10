mod auth;
mod error;
mod models;
mod routes;

use axum::{Router, http::{HeaderValue, Method}, routing::get};
use firestore::FirestoreDb;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub db: FirestoreDb,
    pub jwt_secret: String,
    pub superadmin_invite_code: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let gcp_project = std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required");
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET required");
    let superadmin_invite_code = std::env::var("SUPERADMIN_INVITE_CODE").expect("SUPERADMIN_INVITE_CODE required");
    let allowed_origins = std::env::var("ALLOWED_ORIGINS").unwrap_or_else(|_| "*".to_string());

    let db = FirestoreDb::new(&gcp_project)
        .await
        .expect("failed to connect to Firestore");

    let state = AppState { db, jwt_secret, superadmin_invite_code };

    let cors = if allowed_origins == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers(Any)
    } else {
        let origins: Vec<HeaderValue> = allowed_origins
            .split(',')
            .map(|s| s.trim().parse::<HeaderValue>().expect("invalid origin in ALLOWED_ORIGINS"))
            .collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::predicate(move |origin, _| origins.contains(origin)))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers(Any)
    };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/auth", routes::auth::router())
        .nest("/events", routes::events::router())
        .nest("/invites", routes::invites::router())
        .with_state(state)
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
