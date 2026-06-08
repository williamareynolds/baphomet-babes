mod auth;
mod db;
mod error;
mod models;
mod routes;

use axum::{Router, http::{HeaderValue, Method}, routing::get};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<libsql::Database>,
    pub jwt_secret: String,
    pub member_invite_code: String,
    pub admin_invite_code: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_url = std::env::var("TURSO_URL").expect("TURSO_URL required");
    let db_token = std::env::var("TURSO_TOKEN").expect("TURSO_TOKEN required");
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET required");
    let member_invite_code =
        std::env::var("MEMBER_INVITE_CODE").expect("MEMBER_INVITE_CODE required");
    let admin_invite_code =
        std::env::var("ADMIN_INVITE_CODE").expect("ADMIN_INVITE_CODE required");
    let allowed_origin = std::env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| "*".to_string());

    let db = libsql::Builder::new_remote(db_url, db_token)
        .build()
        .await
        .expect("DB connect failed");

    let conn = db.connect().expect("DB connection failed");
    db::migrate(&conn).await.expect("migration failed");

    let state = AppState {
        db: Arc::new(db),
        jwt_secret,
        member_invite_code,
        admin_invite_code,
    };

    let cors = if allowed_origin == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(allowed_origin.parse::<HeaderValue>().expect("invalid ALLOWED_ORIGIN"))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers(Any)
    };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/auth", routes::auth::router())
        .nest("/events", routes::events::router())
        .with_state(state)
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
