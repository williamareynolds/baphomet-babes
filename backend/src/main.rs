use std::net::SocketAddr;
use backend::{AppState, RateLimit, build_app};
use firestore::FirestoreDb;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // gcloud-sdk's rustls requires a process-level crypto provider; pin ring
    // so another dependency enabling aws-lc-rs can't make selection ambiguous.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring provider");

    let gcp_project = std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required");
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET required");
    let superadmin_invite_code = std::env::var("SUPERADMIN_INVITE_CODE").expect("SUPERADMIN_INVITE_CODE required");
    let allowed_origins = std::env::var("ALLOWED_ORIGINS").ok();

    // App Check enforcement is opt-in. Leave it off until token-sending
    // frontends are deployed and confirmed, then set APP_CHECK_ENFORCE=true.
    let app_check = std::env::var("APP_CHECK_ENFORCE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
        .then(|| {
            let pn = std::env::var("APP_CHECK_PROJECT_NUMBER")
                .expect("APP_CHECK_PROJECT_NUMBER required when APP_CHECK_ENFORCE=true");
            tracing::info!("App Check enforcement ENABLED for project {pn}");
            backend::app_check::AppCheck::new(pn)
        });

    // FCM push: on by default in production (FCM_ENABLE=true), where the GCP
    // metadata server can mint OAuth tokens for the runtime service account.
    // Off in local dev — sends become no-ops.
    let fcm = std::env::var("FCM_ENABLE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
        .then(|| {
            tracing::info!("FCM push enabled for project {gcp_project}");
            backend::fcm::Fcm::new(&gcp_project)
        });

    let db = FirestoreDb::new(&gcp_project)
        .await
        .expect("failed to connect to Firestore");

    let state = AppState { db, jwt_secret, superadmin_invite_code, app_check, fcm };
    let app = build_app(state, allowed_origins.as_deref(), RateLimit::default());

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
