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
        fcm: None,
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
async fn invite_carries_contact_details() {
    if !emulator_available() { return; }
    let app = test_app("invitedetails").await;
    let root = bootstrap_superadmin(&app).await;

    // Full details round-trip on create and in the listing.
    let (status, full) = send(&app, req("POST", "/invites", Some(&root), Some(json!({
        "role": "member",
        "first_name": "  Ada  ",
        "last_name": "Lovelace",
        "phone": "555-0101",
    })))).await;
    assert_eq!(status, StatusCode::OK, "create failed: {full}");
    assert_eq!(full["first_name"], "Ada", "first name is trimmed");
    assert_eq!(full["last_name"], "Lovelace");
    assert_eq!(full["phone"], "555-0101");

    // Blank optional fields are dropped, not stored as empty strings.
    let (status, sparse) = send(&app, req("POST", "/invites", Some(&root), Some(json!({
        "role": "member",
        "first_name": "Grace",
        "last_name": "   ",
        "phone": "",
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sparse["first_name"], "Grace");
    assert!(sparse["last_name"].is_null(), "blank last name omitted");
    assert!(sparse["phone"].is_null(), "blank phone omitted");

    // Details survive into the listing.
    let (status, list) = send(&app, req("GET", "/invites", Some(&root), None)).await;
    assert_eq!(status, StatusCode::OK);
    let ada = list.as_array().unwrap().iter()
        .find(|c| c["first_name"] == "Ada")
        .expect("Ada's invite is listed");
    assert_eq!(ada["phone"], "555-0101");
}

#[tokio::test]
async fn revoke_unused_respects_role_and_keeps_used() {
    if !emulator_available() { return; }
    let app = test_app("revokeunused").await;
    let root = bootstrap_superadmin(&app).await;
    let (admin, _) = invite_and_register(&app, &root, "admin@test.com", "admin1", "admin").await;

    // Superadmin mints: one member code (consumed below), one spare member code,
    // and one admin code.
    let (_, used_invite) = send(&app, req("POST", "/invites", Some(&root), Some(json!({
        "role": "member", "first_name": "Used",
    })))).await;
    let used_code = used_invite["code"].as_str().unwrap().to_string();
    let (status, _) = register(&app, "used@test.com", "usedone", &used_code).await;
    assert_eq!(status, StatusCode::OK);

    for name in ["SpareOne", "SpareTwo"] {
        let (status, _) = send(&app, req("POST", "/invites", Some(&root), Some(json!({
            "role": "member", "first_name": name,
        })))).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, _) = send(&app, req("POST", "/invites", Some(&root), Some(json!({
        "role": "admin", "first_name": "FutureAdmin",
    })))).await;
    assert_eq!(status, StatusCode::OK);

    // Admin bulk-revoke clears only the unused MEMBER codes; the admin code and
    // the already-used code are left alone.
    let (status, n) = send(&app, req("DELETE", "/invites", Some(&admin), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(n.as_u64().unwrap(), 2, "two unused member codes revoked");

    let (_, list) = send(&app, req("GET", "/invites", Some(&root), None)).await;
    let codes = list.as_array().unwrap();
    assert!(codes.iter().any(|c| c["role"] == "admin"), "admin code survives");
    assert!(codes.iter().any(|c| c["used"] == true), "used code survives");
    assert!(!codes.iter().any(|c| c["role"] == "member" && c["used"] == false),
        "no unused member codes remain");

    // Superadmin bulk-revoke now clears the remaining unused (admin) code.
    let (status, n) = send(&app, req("DELETE", "/invites", Some(&root), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(n.as_u64().unwrap(), 1, "the unused admin code revoked");

    let (_, list) = send(&app, req("GET", "/invites", Some(&root), None)).await;
    assert!(!list.as_array().unwrap().iter().any(|c| c["used"] == false),
        "all unused codes gone; only the used one remains");
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
async fn events_date_is_optional() {
    if !emulator_available() { return; }
    let app = test_app("optdate").await;
    let root = bootstrap_superadmin(&app).await;

    // Create with no date — an event being voted on before a date is picked.
    let (status, created) = send(&app, req("POST", "/events", Some(&root), Some(json!({
        "event_type": "main", "title": "TBD Pick"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(created["date"].is_null());
    let id = created["id"].as_str().unwrap().to_string();

    // Empty-string date is normalized to null too.
    let (status, created2) = send(&app, req("POST", "/events", Some(&root), Some(json!({
        "event_type": "special", "title": "Blank", "date": ""
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(created2["date"].is_null());

    // Setting a date later.
    let (status, dated) = send(&app, req("PUT", &format!("/events/{id}"), Some(&root), Some(json!({
        "date": "2031-01-01"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dated["date"], "2031-01-01");

    // Updating other fields leaves the date intact (None = no change).
    let (status, kept) = send(&app, req("PUT", &format!("/events/{id}"), Some(&root), Some(json!({
        "title": "Now Titled"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(kept["date"], "2031-01-01");

    // Sending an empty-string date clears it back to null.
    let (status, cleared) = send(&app, req("PUT", &format!("/events/{id}"), Some(&root), Some(json!({
        "date": ""
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(cleared["date"].is_null());
}

#[tokio::test]
async fn event_rsvp_flow() {
    if !emulator_available() { return; }
    let app = test_app("rsvp").await;
    let root = bootstrap_superadmin(&app).await;
    let (member, _) = invite_and_register(&app, &root, "m@test.com", "member1", "member").await;
    let (member2, _) = invite_and_register(&app, &root, "m2@test.com", "member2", "member").await;

    // An event with a far-future RSVP deadline.
    let (status, created) = send(&app, req("POST", "/events", Some(&root), Some(json!({
        "event_type": "main", "title": "RSVP Movie", "date": "2099-01-01", "rsvp_deadline": "2099-01-01"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["rsvp_deadline"], "2099-01-01");
    assert_eq!(created["rsvp_count"], 0);
    let id = created["id"].as_str().unwrap().to_string();

    // Member RSVPs going; count reflects it and my_rsvp is true for that member.
    let (status, ev) = send(&app, req("POST", &format!("/events/{id}/rsvp"), Some(&member), Some(json!({ "going": true })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ev["rsvp_count"], 1);
    assert_eq!(ev["my_rsvp"], true);

    // Re-RSVPing is idempotent (still 1, not 2).
    let (_, ev) = send(&app, req("POST", &format!("/events/{id}/rsvp"), Some(&member), Some(json!({ "going": true })))).await;
    assert_eq!(ev["rsvp_count"], 1);

    // A second member pushes the count to 2.
    let (_, ev) = send(&app, req("POST", &format!("/events/{id}/rsvp"), Some(&member2), Some(json!({ "going": true })))).await;
    assert_eq!(ev["rsvp_count"], 2);

    // The list shows the caller's own status; member2 sees my_rsvp true.
    let (_, list) = send(&app, req("GET", "/events", Some(&member2), None)).await;
    let row = list.as_array().unwrap().iter().find(|e| e["id"] == id.as_str()).unwrap();
    assert_eq!(row["rsvp_count"], 2);
    assert_eq!(row["my_rsvp"], true);

    // Admin can see who's going; a member cannot.
    let (status, names) = send(&app, req("GET", &format!("/events/{id}/rsvps"), Some(&root), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names.as_array().unwrap().len(), 2);
    let (status, _) = send(&app, req("GET", &format!("/events/{id}/rsvps"), Some(&member), None)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Cancelling drops the count and clears my_rsvp.
    let (_, ev) = send(&app, req("POST", &format!("/events/{id}/rsvp"), Some(&member), Some(json!({ "going": false })))).await;
    assert_eq!(ev["rsvp_count"], 1);
    assert_eq!(ev["my_rsvp"], false);

    // A passed deadline rejects new RSVPs.
    let (status, past) = send(&app, req("POST", "/events", Some(&root), Some(json!({
        "event_type": "main", "title": "Closed", "date": "2000-01-01", "rsvp_deadline": "2000-01-01"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    let past_id = past["id"].as_str().unwrap().to_string();
    let (status, body) = send(&app, req("POST", &format!("/events/{past_id}/rsvp"), Some(&member), Some(json!({ "going": true })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("deadline"));
}

#[tokio::test]
async fn announcements_crud_and_permissions() {
    if !emulator_available() { return; }
    let app = test_app("announcements").await;
    let root = bootstrap_superadmin(&app).await;
    let (member, _) = invite_and_register(&app, &root, "m@test.com", "member1", "member").await;

    // Member cannot post announcements.
    let post = json!({ "title": "Welcome", "body": "Hello babes", "poll_embed_url": null });
    let (status, _) = send(&app, req("POST", "/announcements", Some(&member), Some(post.clone()))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Admin (superadmin) posts; member lists.
    let (status, created) = send(&app, req("POST", "/announcements", Some(&root), Some(post))).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["title"], "Welcome");

    let (status, list) = send(&app, req("GET", "/announcements", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Anonymous cannot list.
    let (status, _) = send(&app, req("GET", "/announcements", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Empty title rejected.
    let (status, body) = send(&app, req("POST", "/announcements", Some(&root), Some(json!({
        "title": "   ", "body": "x", "poll_embed_url": null
    })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("title"));

    // Update then delete.
    let (status, updated) = send(&app, req("PUT", &format!("/announcements/{id}"), Some(&root), Some(json!({
        "body": "Hello again", "poll_embed_url": "https://rcv123.org/poll/abc"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["body"], "Hello again");
    assert_eq!(updated["poll_embed_url"], "https://rcv123.org/poll/abc");

    let (status, _) = send(&app, req("DELETE", &format!("/announcements/{id}"), Some(&root), None)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, list) = send(&app, req("GET", "/announcements", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().is_empty());

    // Unknown id → 404.
    let (status, _) = send(&app, req("PUT", "/announcements/nope", Some(&root), Some(json!({ "title": "x" })))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn notifications_feed_prefs_tokens_and_broadcast() {
    if !emulator_available() { return; }
    let app = test_app("notifications").await;
    let root = bootstrap_superadmin(&app).await;
    let (member, _) = invite_and_register(&app, &root, "m@test.com", "member1", "member").await;

    // Prefs default to all-on.
    let (status, prefs) = send(&app, req("GET", "/notifications/prefs", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(prefs["announcements"], true);
    assert_eq!(prefs["general"], true);
    assert_eq!(prefs["movie_night"], true);
    assert_eq!(prefs["chat"], false); // chat is opt-in (off by default)

    // Update one channel; others persist.
    let (status, prefs) = send(&app, req("PUT", "/notifications/prefs", Some(&member), Some(json!({ "general": false })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(prefs["general"], false);
    assert_eq!(prefs["announcements"], true);
    // Survives a reload.
    let (_, prefs) = send(&app, req("GET", "/notifications/prefs", Some(&member), None)).await;
    assert_eq!(prefs["general"], false);

    // Device token register + unregister; empty token rejected.
    let (status, _) = send(&app, req("PUT", "/notifications/token", Some(&member), Some(json!({ "token": "device-tok-1" })))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send(&app, req("PUT", "/notifications/token", Some(&member), Some(json!({ "token": "  " })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("token"));
    let (status, _) = send(&app, req("DELETE", "/notifications/token", Some(&member), Some(json!({ "token": "device-tok-1" })))).await;
    assert_eq!(status, StatusCode::OK);

    // Anonymous cannot read the feed.
    let (status, _) = send(&app, req("GET", "/notifications", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Posting an announcement and creating an event both land in the feed.
    let (status, _) = send(&app, req("POST", "/announcements", Some(&root), Some(json!({
        "title": "Hi", "body": "welcome", "poll_embed_url": null
    })))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&app, req("POST", "/events", Some(&root), Some(json!({
        "event_type": "main", "title": "The Crow", "date": "2030-10-31"
    })))).await;
    assert_eq!(status, StatusCode::OK);

    let (status, feed) = send(&app, req("GET", "/notifications", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    let feed = feed.as_array().unwrap();
    assert_eq!(feed.len(), 2);
    let channels: Vec<&str> = feed.iter().map(|n| n["channel"].as_str().unwrap()).collect();
    assert!(channels.contains(&"announcements"));
    assert!(channels.contains(&"movie_night"));

    // Broadcast: members can't; admins can; it shows up on the General channel.
    let (status, _) = send(&app, req("POST", "/notifications/broadcast", Some(&member), Some(json!({
        "title": "x", "body": "y"
    })))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = send(&app, req("POST", "/notifications/broadcast", Some(&root), Some(json!({
        "title": "  ", "body": "y"
    })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("title"));
    let (status, _) = send(&app, req("POST", "/notifications/broadcast", Some(&root), Some(json!({
        "title": "Meetup Saturday", "body": "Park at 2pm"
    })))).await;
    assert_eq!(status, StatusCode::OK);

    let (_, feed) = send(&app, req("GET", "/notifications", Some(&member), None)).await;
    let feed = feed.as_array().unwrap();
    assert_eq!(feed.len(), 3);
    // The broadcast shows up on the General channel. (Position isn't asserted:
    // all three records can land in the same wall-clock second, and ordering
    // within a second is unspecified.)
    let general = feed.iter().find(|n| n["channel"] == "general").expect("general notification present");
    assert_eq!(general["title"], "Meetup Saturday");

    // Clear is per-user: it empties the member's view but not the root's, and
    // doesn't touch the shared records.
    std::thread::sleep(std::time::Duration::from_millis(1100)); // watermark is seconds-granular
    let (status, _) = send(&app, req("POST", "/notifications/clear", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    let (_, feed) = send(&app, req("GET", "/notifications", Some(&member), None)).await;
    assert!(feed.as_array().unwrap().is_empty(), "member feed cleared");
    let (_, feed) = send(&app, req("GET", "/notifications", Some(&root), None)).await;
    assert_eq!(feed.as_array().unwrap().len(), 3, "root feed unaffected by member's clear");

    // New notifications after a clear reappear for the member.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let (status, _) = send(&app, req("POST", "/notifications/broadcast", Some(&root), Some(json!({
        "title": "After clear", "body": "z"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, feed) = send(&app, req("GET", "/notifications", Some(&member), None)).await;
    let feed = feed.as_array().unwrap();
    assert_eq!(feed.len(), 1);
    assert_eq!(feed[0]["title"], "After clear");
}

#[tokio::test]
async fn group_chat_send_list_and_validation() {
    if !emulator_available() { return; }
    let app = test_app("chat").await;
    let root = bootstrap_superadmin(&app).await;
    let (member, _) = invite_and_register(&app, &root, "c@test.com", "chatter", "member").await;

    // Anonymous can't read or post.
    let (status, _) = send(&app, req("GET", "/chat", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Empty / blank messages are rejected.
    let (status, body) = send(&app, req("POST", "/chat", Some(&member), Some(json!({ "body": "   " })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("message"));

    // Send a message; the author label is the username (no display name set).
    let (status, msg) = send(&app, req("POST", "/chat", Some(&member), Some(json!({ "body": "  hello group  " })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(msg["body"], "hello group"); // trimmed
    assert_eq!(msg["author"], "chatter");

    // A second message from a different member. Sleep first so its created_at
    // lands in a later second — timestamps are seconds-granular, and the feed's
    // order_by(created_at) is otherwise ambiguous within a single second.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let (status, _) = send(&app, req("POST", "/chat", Some(&root), Some(json!({ "body": "hi chatter" })))).await;
    assert_eq!(status, StatusCode::OK);

    // The feed returns both, oldest-first.
    let (status, feed) = send(&app, req("GET", "/chat", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    let feed = feed.as_array().unwrap();
    assert_eq!(feed.len(), 2);
    assert_eq!(feed[0]["body"], "hello group");
    assert_eq!(feed[1]["body"], "hi chatter");

    // Chat is push-only: it must NOT create inbox records (else it would flood
    // the capped announcements feed).
    let (_, notifs) = send(&app, req("GET", "/notifications", Some(&member), None)).await;
    let chat_notif = notifs.as_array().unwrap().iter().find(|n| n["channel"] == "chat");
    assert!(chat_notif.is_none(), "chat messages stay out of the inbox feed");
}

#[tokio::test]
async fn calendar_token_and_ics_feed() {
    if !emulator_available() { return; }
    let app = test_app("calendar").await;
    let root = bootstrap_superadmin(&app).await;
    let (member, _) = invite_and_register(&app, &root, "cal@test.com", "calfan", "member").await;

    // Anonymous can't mint a token.
    let (status, _) = send(&app, req("GET", "/calendar/me", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Member mints a token; it's stable across calls.
    let (status, t1) = send(&app, req("GET", "/calendar/me", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    let token1 = t1["token"].as_str().unwrap().to_string();
    assert!(!token1.is_empty());
    let (_, t1b) = send(&app, req("GET", "/calendar/me", Some(&member), None)).await;
    assert_eq!(t1b["token"].as_str().unwrap(), token1, "token stable across reads");

    // Seed an event so the feed has content.
    let (status, _) = send(&app, req("POST", "/events", Some(&root), Some(json!({
        "event_type": "main", "title": "The Crow", "date": "2030-10-31", "description": "Bring snacks"
    })))).await;
    assert_eq!(status, StatusCode::OK);

    // Public feed with a valid token: 200, text/calendar, contains the event as
    // an all-day VEVENT.
    let feed_path = format!("/calendar/{token1}/baphomet-babes.ics");
    let resp = app.clone().oneshot(req("GET", &feed_path, None, None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "text/calendar; charset=utf-8",
    );
    let body = String::from_utf8(resp.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(body.contains("BEGIN:VCALENDAR"));
    assert!(body.contains("SUMMARY:The Crow"));
    assert!(body.contains("DTSTART;VALUE=DATE:20301031"));

    // Unknown token → 404.
    let (status, _) = send(&app, req("GET", "/calendar/deadbeef/baphomet-babes.ics", None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Regenerate rotates the token; the old feed URL stops working.
    let (status, t2) = send(&app, req("POST", "/calendar/me/regenerate", Some(&member), None)).await;
    assert_eq!(status, StatusCode::OK);
    let token2 = t2["token"].as_str().unwrap().to_string();
    assert_ne!(token2, token1, "regenerate yields a new token");
    let (status, _) = send(&app, req("GET", &feed_path, None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "old token revoked");
    let (status, _) = send(&app, req("GET", &format!("/calendar/{token2}/baphomet-babes.ics"), None, None)).await;
    assert_eq!(status, StatusCode::OK, "new token works");
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
        fcm: None,
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
        fcm: None,
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
        fcm: None,
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

    // The public .ics calendar feed is exempt (calendar apps send no token). An
    // unknown token 404s — crucially NOT 401 "missing app check token", which
    // proves the request passed the App Check gate.
    let (status, _) = send(&app, req("GET", "/calendar/whatever/baphomet-babes.ics", None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

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

#[tokio::test]
async fn user_admin_roles_disable_and_guards() {
    if !emulator_available() { return; }
    let app = test_app("useradmin").await;
    let root = bootstrap_superadmin(&app).await;
    let (admin, admin_id) = invite_and_register(&app, &root, "admin@test.com", "admin1", "admin").await;
    let (_member, member_id) = invite_and_register(&app, &root, "m@test.com", "member1", "member").await;

    // Only superadmin can list users.
    let (status, _) = send(&app, req("GET", "/users", Some(&admin), None)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, list) = send(&app, req("GET", "/users", Some(&root), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 3);

    // Only superadmin can update users.
    let (status, _) = send(&app, req("PUT", &format!("/users/{member_id}"), Some(&admin), Some(json!({ "role": "admin" })))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Capture the member's pre-promotion token (carries role "member") to prove
    // role changes are read from the DB per-request, not from the stale token.
    let (status, login) = send(&app, req("POST", "/auth/login", None, Some(json!({
        "email": "m@test.com", "password": "hunter2hunter2"
    })))).await;
    assert_eq!(status, StatusCode::OK);
    let member_token = login["token"].as_str().unwrap().to_string();

    // Member can't create events yet.
    let event = json!({ "event_type": "main", "title": "x", "date": "2026-07-01" });
    let (status, _) = send(&app, req("POST", "/events", Some(&member_token), Some(event.clone()))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Promote member to admin.
    let (status, body) = send(&app, req("PUT", &format!("/users/{member_id}"), Some(&root), Some(json!({ "role": "admin" })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["role"], "admin");

    // The SAME old token now grants admin — authorization uses the live DB role.
    let (status, _) = send(&app, req("POST", "/events", Some(&member_token), Some(event))).await;
    assert_eq!(status, StatusCode::OK);

    // Invalid role rejected.
    let (status, _) = send(&app, req("PUT", &format!("/users/{member_id}"), Some(&root), Some(json!({ "role": "wizard" })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Superadmin cannot change their own role/status (anti-lockout).
    let me = send(&app, req("GET", "/users", Some(&root), None)).await.1;
    let my_id = me.as_array().unwrap().iter()
        .find(|u| u["role"] == "superadmin").unwrap()["id"].as_str().unwrap().to_string();
    let (status, _) = send(&app, req("PUT", &format!("/users/{my_id}"), Some(&root), Some(json!({ "disabled": true })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Disable the admin account.
    let (status, body) = send(&app, req("PUT", &format!("/users/{admin_id}"), Some(&root), Some(json!({ "disabled": true })))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["disabled"], true);

    // Hard revocation: their ALREADY-ISSUED token is rejected on a protected
    // route, not just at login — the per-request DB check sees disabled=true.
    let (status, body) = send(&app, req("GET", "/events", Some(&admin), None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "account disabled");

    // And they can no longer log in.
    let (status, body) = send(&app, req("POST", "/auth/login", None, Some(json!({
        "email": "admin@test.com", "password": "hunter2hunter2"
    })))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "account disabled");

    // Re-enable, login works again.
    let (status, _) = send(&app, req("PUT", &format!("/users/{admin_id}"), Some(&root), Some(json!({ "disabled": false })))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&app, req("POST", "/auth/login", None, Some(json!({
        "email": "admin@test.com", "password": "hunter2hunter2"
    })))).await;
    assert_eq!(status, StatusCode::OK);

    // Unknown user id → 404.
    let (status, _) = send(&app, req("PUT", "/users/does-not-exist", Some(&root), Some(json!({ "role": "member" })))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn user_list_reports_enrolled_device_count() {
    if !emulator_available() { return; }
    let app = test_app("devicecount").await;
    let root = bootstrap_superadmin(&app).await;
    let (member, member_id) = invite_and_register(&app, &root, "m@test.com", "member1", "member").await;

    // Helper: the device_count the user list reports for `member_id`.
    let count_for = |list: &Value| -> i64 {
        list.as_array().unwrap().iter()
            .find(|u| u["id"] == member_id.as_str())
            .unwrap()["device_count"].as_i64().unwrap()
    };

    // No tokens registered yet.
    let (_, list) = send(&app, req("GET", "/users", Some(&root), None)).await;
    assert_eq!(count_for(&list), 0);

    // Member enrolls two devices.
    for tok in ["device-token-a", "device-token-b"] {
        let (status, _) = send(&app, req("PUT", "/notifications/token", Some(&member), Some(json!({ "token": tok })))).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (_, list) = send(&app, req("GET", "/users", Some(&root), None)).await;
    assert_eq!(count_for(&list), 2);

    // Re-registering the same token is idempotent (still 2, not 3).
    let (_, _) = send(&app, req("PUT", "/notifications/token", Some(&member), Some(json!({ "token": "device-token-a" })))).await;
    let (_, list) = send(&app, req("GET", "/users", Some(&root), None)).await;
    assert_eq!(count_for(&list), 2);

    // Unregistering one drops the count, and the update_user response carries it too.
    let (status, _) = send(&app, req("DELETE", "/notifications/token", Some(&member), Some(json!({ "token": "device-token-a" })))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, updated) = send(&app, req("PUT", &format!("/users/{member_id}"), Some(&root), Some(json!({ "role": "member" })))).await;
    assert_eq!(updated["device_count"], 1);
}
