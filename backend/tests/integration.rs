//! Integration tests — run the real router against the Firestore emulator.
//!
//! Requires `FIRESTORE_EMULATOR_HOST` (e.g. 127.0.0.1:8787). Without it every
//! test no-ops with a notice so the suite stays green where the emulator is
//! unavailable. Run via `just test-integration`.

use axum::{Router, body::Body, http::{Request, StatusCode}};
use backend::{AppState, RateLimit, build_app};
use firestore::FirestoreDb;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const BOOTSTRAP: &str = "boot-code-for-tests";
const JWT_SECRET: &str = "integration-test-secret";

fn emulator_available() -> bool {
    if std::env::var("FIRESTORE_EMULATOR_HOST").is_ok() {
        true
    } else {
        eprintln!("skipping: FIRESTORE_EMULATOR_HOST not set");
        false
    }
}

/// Fresh app on an isolated project (emulator namespaces data per project id).
async fn test_app(name: &str) -> Router {
    let project = format!("bb-test-{name}-{}", std::process::id());
    let db = FirestoreDb::new(&project).await.expect("emulator connection");
    let state = AppState {
        db,
        jwt_secret: JWT_SECRET.into(),
        superadmin_invite_code: BOOTSTRAP.into(),
        app_check: None,
    };
    // Effectively unlimited so functional tests never trip the governor.
    build_app(state, None, RateLimit { per_second: 1, burst: 1_000_000 })
}

fn req(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        // Governor on /auth keys by client IP
        .header("x-forwarded-for", "10.1.2.3");
    if let Some(t) = token {
        builder = builder.header("Authorization", format!("Bearer {t}"));
    }
    match body {
        Some(v) => builder
            .header("Content-Type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn send(app: &Router, r: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(r).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::String(
            String::from_utf8_lossy(&bytes).into_owned(),
        ))
    };
    (status, value)
}

async fn register(app: &Router, email: &str, username: &str, code: &str) -> (StatusCode, Value) {
    send(app, req("POST", "/auth/register", None, Some(json!({
        "email": email,
        "username": username,
        "password": "hunter2hunter2",
        "invite_code": code,
    }))))
    .await
}

/// Bootstrap a superadmin and return their token.
async fn bootstrap_superadmin(app: &Router) -> String {
    let (status, body) = register(app, "root@test.com", "root", BOOTSTRAP).await;
    assert_eq!(status, StatusCode::OK, "bootstrap failed: {body}");
    body["token"].as_str().unwrap().to_string()
}

/// Create an invite as `admin_token` and register a fresh user with it.
async fn invite_and_register(app: &Router, admin_token: &str, email: &str, username: &str, role: &str) -> (String, String) {
    let (status, invite) = send(app, req("POST", "/invites", Some(admin_token), Some(json!({ "role": role })))).await;
    assert_eq!(status, StatusCode::OK, "invite failed: {invite}");
    let code = invite["code"].as_str().unwrap();
    let (status, body) = register(app, email, username, code).await;
    assert_eq!(status, StatusCode::OK, "register failed: {body}");
    (
        body["token"].as_str().unwrap().to_string(),
        body["user"]["id"].as_str().unwrap().to_string(),
    )
}

#[tokio::test]
async fn health_check() {
    if !emulator_available() { return; }
    let app = test_app("health").await;
    let (status, body) = send(&app, req("GET", "/health", None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, Value::String("ok".into()));
}

#[tokio::test]
async fn superadmin_bootstrap_works_only_once() {
    if !emulator_available() { return; }
    let app = test_app("bootstrap").await;

    let (status, body) = register(&app, "first@test.com", "first", BOOTSTRAP).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["role"], "superadmin");

    // Second use of the bootstrap code must be rejected.
    let (status, body) = register(&app, "second@test.com", "second", BOOTSTRAP).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn invalid_invite_code_rejected() {
    if !emulator_available() { return; }
    let app = test_app("badinvite").await;
    let (status, body) = register(&app, "x@test.com", "x", "not-a-real-code").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "invalid invite code");
}

#[tokio::test]
async fn login_roundtrip_and_wrong_password() {
    if !emulator_available() { return; }
    let app = test_app("login").await;
    bootstrap_superadmin(&app).await;

    let (status, body) = send(&app, req("POST", "/auth/login", None, Some(json!({
        "email": "root@test.com",
        "password": "hunter2hunter2",
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
    assert_eq!(body["user"]["username"], "root");

    let (status, body) = send(&app, req("POST", "/auth/login", None, Some(json!({
        "email": "root@test.com",
        "password": "wrong",
    })))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "invalid credentials");
}

#[tokio::test]
async fn invite_lifecycle() {
    if !emulator_available() { return; }
    let app = test_app("invites").await;
    let root = bootstrap_superadmin(&app).await;

    // Member registered through an invite cannot mint invites.
    let (member_token, _) = invite_and_register(&app, &root, "m@test.com", "member1", "member").await;
    let (status, _) = send(&app, req("POST", "/invites", Some(&member_token), Some(json!({ "role": "member" })))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A used invite cannot be reused.
    let (status, invite) = send(&app, req("POST", "/invites", Some(&root), Some(json!({ "role": "member" })))).await;
    assert_eq!(status, StatusCode::OK);
    let code = invite["code"].as_str().unwrap();
    let (status, _) = register(&app, "a@test.com", "a", code).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = register(&app, "b@test.com", "b", code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "invite code already used");
}

#[tokio::test]
async fn invite_delete_and_role_rules() {
    if !emulator_available() { return; }
    let app = test_app("invitedelete").await;
    let root = bootstrap_superadmin(&app).await;
    let (admin, _) = invite_and_register(&app, &root, "admin@test.com", "admin1", "admin").await;

    // Admin may not mint admin invites; superadmin may.
    let (status, _) = send(&app, req("POST", "/invites", Some(&admin), Some(json!({ "role": "admin" })))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Superadmin mints an admin invite; admin cannot delete it, superadmin can.
    let (status, admin_invite) = send(&app, req("POST", "/invites", Some(&root), Some(json!({ "role": "admin" })))).await;
    assert_eq!(status, StatusCode::OK);
    let admin_invite_id = admin_invite["id"].as_str().unwrap();

    let (status, _) = send(&app, req("DELETE", &format!("/invites/{admin_invite_id}"), Some(&admin), None)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(&app, req("DELETE", &format!("/invites/{admin_invite_id}"), Some(&root), None)).await;
    assert_eq!(status, StatusCode::OK, "delete via /invites/:id must route (axum 0.7 colon params)");

    // Deleted invite is unusable.
    let code = admin_invite["code"].as_str().unwrap();
    let (status, _) = register(&app, "ghost@test.com", "ghost", code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn duplicate_email_rejected() {
    if !emulator_available() { return; }
    let app = test_app("dupemail").await;
    let root = bootstrap_superadmin(&app).await;

    let (status, invite) = send(&app, req("POST", "/invites", Some(&root), Some(json!({ "role": "member" })))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = register(&app, "root@test.com", "imposter", invite["code"].as_str().unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "email already registered");
}

#[tokio::test]
async fn events_crud_and_permissions() {
    if !emulator_available() { return; }
    let app = test_app("events").await;
    let root = bootstrap_superadmin(&app).await;
    let (member, _) = invite_and_register(&app, &root, "m@test.com", "member1", "member").await;

    // Member cannot create events.
    let event = json!({ "event_type": "main", "title": "Movie", "date": "2026-07-01" });
    let (status, _) = send(&app, req("POST", "/events", Some(&member), Some(event.clone()))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Admin (superadmin) creates, member lists.
    let (status, created) = send(&app, req("POST", "/events", Some(&root), Some(event))).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_str().unwrap();

    let (status, list) = send(&app, req("GET", "/events", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Anonymous cannot list.
    let (status, _) = send(&app, req("GET", "/events", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Bad event_type rejected.
    let (status, body) = send(&app, req("POST", "/events", Some(&root), Some(json!({
        "event_type": "party", "title": "x", "date": "2026-01-01"
    })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("event_type"));

    // Update then delete.
    let (status, updated) = send(&app, req("PUT", &format!("/events/{id}"), Some(&root), Some(json!({
        "title": "Movie (rescheduled)"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["title"], "Movie (rescheduled)");

    let (status, _) = send(&app, req("DELETE", &format!("/events/{id}"), Some(&root), None)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, list) = send(&app, req("GET", "/events", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn profile_lifecycle_and_visibility() {
    if !emulator_available() { return; }
    let app = test_app("profiles").await;
    let root = bootstrap_superadmin(&app).await;
    let (alice, alice_id) = invite_and_register(&app, &root, "alice@test.com", "alice", "member").await;
    let (bob, _) = invite_and_register(&app, &root, "bob@test.com", "bob", "member").await;

    // Profile auto-created on register, private by default.
    let (status, profile) = send(&app, req("GET", "/profile/me", Some(&alice), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(profile["username"], "alice");
    assert_eq!(profile["is_public"], false);

    // Private profile: hidden from other members, visible to admin and self.
    let (status, _) = send(&app, req("GET", &format!("/members/{alice_id}"), Some(&bob), None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(&app, req("GET", &format!("/members/{alice_id}"), Some(&root), None)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&app, req("GET", &format!("/members/{alice_id}"), Some(&alice), None)).await;
    assert_eq!(status, StatusCode::OK);

    // Directory empty while everyone is private.
    let (status, list) = send(&app, req("GET", "/members", Some(&bob), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().is_empty());

    // Alice publishes her profile with details.
    let (status, updated) = send(&app, req("PUT", "/profile/me", Some(&alice), Some(json!({
        "bio": "crafts + cosmos",
        "pronouns": "she/her",
        "links": [{ "label": "Site", "url": "https://alice.example" }],
        "is_public": true
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["is_public"], true);
    assert_eq!(updated["bio"], "crafts + cosmos");

    // Now in the directory and visible to Bob.
    let (status, list) = send(&app, req("GET", "/members", Some(&bob), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["username"], "alice");

    // Anonymous directory access denied.
    let (status, _) = send(&app, req("GET", "/members", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_powers_over_profiles() {
    if !emulator_available() { return; }
    let app = test_app("adminprofiles").await;
    let root = bootstrap_superadmin(&app).await;
    let (admin, _) = invite_and_register(&app, &root, "admin@test.com", "admin1", "admin").await;
    let (alice, alice_id) = invite_and_register(&app, &root, "alice@test.com", "alice", "member").await;
    let (bob, _) = invite_and_register(&app, &root, "bob@test.com", "bob", "member").await;

    // Alice goes public.
    let (status, _) = send(&app, req("PUT", "/profile/me", Some(&alice), Some(json!({ "is_public": true })))).await;
    assert_eq!(status, StatusCode::OK);

    // Member cannot touch another profile.
    let (status, _) = send(&app, req("PUT", &format!("/members/{alice_id}"), Some(&bob), Some(json!({ "is_public": false })))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Admin can force-private…
    let (status, body) = send(&app, req("PUT", &format!("/members/{alice_id}"), Some(&admin), Some(json!({ "is_public": false })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_public"], false);

    // …but cannot force-public or edit fields.
    let (status, _) = send(&app, req("PUT", &format!("/members/{alice_id}"), Some(&admin), Some(json!({ "is_public": true })))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = send(&app, req("PUT", &format!("/members/{alice_id}"), Some(&admin), Some(json!({ "bio": "vandalized" })))).await;
    // Admin path ignores non-visibility fields.
    assert_eq!(status, StatusCode::OK);
    assert_ne!(body["bio"], "vandalized");

    // Superadmin can edit any field.
    let (status, body) = send(&app, req("PUT", &format!("/members/{alice_id}"), Some(&root), Some(json!({
        "bio": "updated by root", "is_public": true
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["bio"], "updated by root");
    assert_eq!(body["is_public"], true);
}

#[tokio::test]
async fn rate_limiter_returns_json_429() {
    if !emulator_available() { return; }
    let project = format!("bb-test-ratelimit-{}", std::process::id());
    let db = FirestoreDb::new(&project).await.expect("emulator connection");
    let state = AppState {
        db,
        jwt_secret: JWT_SECRET.into(),
        superadmin_invite_code: BOOTSTRAP.into(),
        app_check: None,
    };
    let app = build_app(state, None, RateLimit { per_second: 1, burst: 2 });

    let login = || req("POST", "/auth/login", None, Some(json!({
        "email": "x@test.com", "password": "irrelevant"
    })));

    // Burst of 2 allowed, third hits the limiter.
    let (s1, _) = send(&app, login()).await;
    let (s2, _) = send(&app, login()).await;
    assert_ne!(s1, StatusCode::TOO_MANY_REQUESTS);
    assert_ne!(s2, StatusCode::TOO_MANY_REQUESTS);
    let (s3, body) = send(&app, login()).await;
    assert_eq!(s3, StatusCode::TOO_MANY_REQUESTS);
    assert!(body["error"].as_str().unwrap().contains("too many requests"));

    // Request with no identifiable client IP → JSON 500 from the governor.
    let no_ip = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({ "email": "x@test.com", "password": "x" }).to_string()))
        .unwrap();
    let (status, body) = send(&app, no_ip).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "rate limiter could not identify client");
}

#[tokio::test]
async fn cors_allows_configured_origin_only() {
    if !emulator_available() { return; }
    let project = format!("bb-test-cors-{}", std::process::id());
    let db = FirestoreDb::new(&project).await.expect("emulator connection");
    let state = AppState {
        db,
        jwt_secret: JWT_SECRET.into(),
        superadmin_invite_code: BOOTSTRAP.into(),
        app_check: None,
    };
    let app = build_app(
        state,
        Some("https://baphometbabes.com,https://movienight.baphometbabes.com"),
        RateLimit { per_second: 1, burst: 1_000_000 },
    );

    let preflight = |origin: &str| {
        Request::builder()
            .method("OPTIONS")
            .uri("/events")
            .header("Origin", origin)
            .header("Access-Control-Request-Method", "GET")
            .body(Body::empty())
            .unwrap()
    };

    let resp = app.clone().oneshot(preflight("https://baphometbabes.com")).await.unwrap();
    assert_eq!(
        resp.headers().get("access-control-allow-origin").map(|v| v.to_str().unwrap()),
        Some("https://baphometbabes.com"),
    );

    let resp = app.clone().oneshot(preflight("https://evil.example")).await.unwrap();
    assert!(resp.headers().get("access-control-allow-origin").is_none());
}

/// With App Check enforcement ON, anything that didn't come from our attested
/// frontend (curl, bots, scripts) is rejected before reaching a handler — even
/// with a perfectly valid JWT. This is the core anti-abuse guarantee.
#[tokio::test]
async fn app_check_blocks_direct_api_access() {
    if !emulator_available() { return; }
    let project = format!("bb-test-appcheck-{}", std::process::id());
    let db = FirestoreDb::new(&project).await.expect("emulator connection");
    let state = AppState {
        db,
        jwt_secret: JWT_SECRET.into(),
        superadmin_invite_code: BOOTSTRAP.into(),
        // Real verifier pointed at our project number. Bogus/missing tokens are
        // rejected without ever contacting Google's JWKS endpoint, so this test
        // needs no network.
        app_check: Some(backend::app_check::AppCheck::new("780823612423")),
    };
    let app = build_app(state, None, RateLimit { per_second: 1, burst: 1_000_000 });

    // Health is exempt — Cloud Run liveness probes carry no token.
    let (status, _) = send(&app, req("GET", "/health", None, None)).await;
    assert_eq!(status, StatusCode::OK);

    // No App Check header → blocked before the handler runs.
    let (status, body) = send(&app, req("POST", "/auth/login", None, Some(json!({
        "email": "x@test.com", "password": "x"
    })))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "missing app check token");

    // Garbage App Check token → rejected at verification.
    let bogus = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("Content-Type", "application/json")
        .header("x-forwarded-for", "10.1.2.3")
        .header("x-firebase-appcheck", "not.a.valid.token")
        .body(Body::from(json!({ "email": "x@test.com", "password": "x" }).to_string()))
        .unwrap();
    let (status, body) = send(&app, bogus).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "app check verification failed");

    // A genuine, valid JWT does NOT bypass App Check on a protected route —
    // proves the gate sits in front of auth, not behind it.
    let token = backend::auth::create_token("u", "superadmin", JWT_SECRET).unwrap();
    let (status, _) = send(&app, req("GET", "/members", Some(&token), None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
