use shared::{
    Announcement, AuthResponse, BroadcastRequest, CalendarToken, ChatMessage,
    CreateAnnouncementRequest, CreateEventRequest, CreateInviteRequest, CreateRideRequest, Event,
    InviteCode, LoginRequest, Notification, NotificationPrefs, Profile, RegisterPushTokenRequest,
    RegisterRequest, Ride, Rsvp, RsvpRequest, SendChatRequest, TestPushResponse,
    UpdateAnnouncementRequest, UpdateEventRequest, UpdateNotificationPrefs, UpdateProfileRequest,
    UpdateRideRequest, UpdateUserRequest, UserSummary,
};
use shared::{
    CreateGatheringRequest, Gathering, GeocodeRequest, GeocodeResponse, UploadImageRequest,
    UploadImageResponse,
};

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

/// The error every authed call returns once the backend has rejected the
/// session. Callers use [`is_session_expired`] to tell "you're logged out" apart
/// from "the network is down" — the two deserve very different handling.
pub const SESSION_EXPIRED: &str = "Your session expired — please log in again.";

/// Whether an API error came from a rejected session rather than a failed
/// request.
pub fn is_session_expired(err: &str) -> bool {
    err == SESSION_EXPIRED
}

/// Drop the stored session and bounce to the login screen.
///
/// Runs at most once per page load: a page typically has several requests in
/// flight and they will all 401 together, but only the first should navigate.
fn end_session() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ENDED: AtomicBool = AtomicBool::new(false);
    if ENDED.swap(true, Ordering::Relaxed) {
        return;
    }
    // Forget this device's push registration so the next login re-registers it
    // (see the self-healing effect in `app`). We can't unregister it backend-side
    // the way an explicit logout does — that call needs the token we just lost.
    crate::push::clear();
    auth_client::clear_auth();
    // Hard navigation rather than flipping the auth signal: this module has no
    // handle on it, and a fresh load also clears any half-populated page state.
    if let Some(w) = web_sys::window() {
        let _ = w.location().assign("/login");
    }
}

/// Turn a non-2xx response into the message callers surface.
///
/// A 401 on a request that carried a token means the session is gone — expired,
/// revoked, or the account was disabled — so end it here instead of letting the
/// backend's wording leak into the UI. 401s from login/register are ordinary bad
/// credentials and must not touch the session, hence `authed`.
async fn error_message(resp: gloo_net::http::Response, authed: bool) -> String {
    if authed && resp.status() == 401 {
        end_session();
        return SESSION_EXPIRED.to_string();
    }
    resp.json::<shared::ErrorResponse>()
        .await
        .map(|e| e.error)
        .unwrap_or_else(|_| "unknown error".to_string())
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
        return Err(error_message(resp, true).await);
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
        return Err(error_message(resp, true).await);
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
    // Anonymous posts (login, register) get their 401s handled as bad
    // credentials, not as a dead session.
    if !resp.ok() {
        return Err(error_message(resp, token.is_some()).await);
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
        return Err(error_message(resp, true).await);
    }
    Ok(())
}

/// DELETE that returns a JSON body (e.g. a count of affected records).
async fn delete_returning<T: serde::de::DeserializeOwned>(path: &str, token: &str) -> Result<T, String> {
    let req = gloo_net::http::Request::delete(&format!("{}{path}", api_base()))
        .header("Authorization", &format!("Bearer {token}"));
    let resp = attach_app_check(req)
        .await
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(error_message(resp, true).await);
    }
    resp.json::<T>().await.map_err(|e| e.to_string())
}

/// DELETE carrying a JSON body (used to unregister a specific push token).
async fn delete_json<B: serde::Serialize>(path: &str, body: &B, token: &str) -> Result<(), String> {
    let req = gloo_net::http::Request::delete(&format!("{}{path}", api_base()))
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
        return Err(error_message(resp, true).await);
    }
    Ok(())
}

/// PUT with no response body to deserialize (returns unit on success).
async fn put_unit<B: serde::Serialize>(path: &str, body: &B, token: &str) -> Result<(), String> {
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
        return Err(error_message(resp, true).await);
    }
    Ok(())
}

/// POST with no response body to deserialize.
async fn post_unit<B: serde::Serialize>(path: &str, body: &B, token: &str) -> Result<(), String> {
    let req = gloo_net::http::Request::post(&format!("{}{path}", api_base()))
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
        return Err(error_message(resp, true).await);
    }
    Ok(())
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

pub async fn fetch_invites(token: &str) -> Result<Vec<InviteCode>, String> {
    get("/invites", token).await
}

pub async fn create_invite(req: CreateInviteRequest, token: &str) -> Result<InviteCode, String> {
    post_json("/invites", &req, Some(token)).await
}

pub async fn delete_invite(id: &str, token: &str) -> Result<(), String> {
    delete(&format!("/invites/{id}"), token).await
}

/// Revoke every unused invite the caller may delete; returns the count revoked.
pub async fn revoke_unused_invites(token: &str) -> Result<usize, String> {
    delete_returning("/invites", token).await
}

pub async fn fetch_announcements(token: &str) -> Result<Vec<Announcement>, String> {
    get("/announcements", token).await
}

pub async fn create_announcement(req: CreateAnnouncementRequest, token: &str) -> Result<Announcement, String> {
    post_json("/announcements", &req, Some(token)).await
}

pub async fn update_announcement(id: &str, req: UpdateAnnouncementRequest, token: &str) -> Result<Announcement, String> {
    put_json(&format!("/announcements/{id}"), &req, token).await
}

pub async fn delete_announcement(id: &str, token: &str) -> Result<(), String> {
    delete(&format!("/announcements/{id}"), token).await
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

/// RSVP (going=true) or cancel (going=false) for an event; returns the event
/// with the refreshed count and the caller's new status.
pub async fn rsvp_event(id: &str, going: bool, token: &str) -> Result<Event, String> {
    post_json(&format!("/events/{id}/rsvp"), &RsvpRequest { going }, Some(token)).await
}

/// Admin-only: the list of members who've RSVP'd "going" to an event.
pub async fn fetch_rsvps(id: &str, token: &str) -> Result<Vec<Rsvp>, String> {
    get(&format!("/events/{id}/rsvps"), token).await
}

// ---- Mountain bike rides ----

pub async fn fetch_rides(token: &str) -> Result<Vec<Ride>, String> {
    get("/rides", token).await
}

pub async fn create_ride(req: CreateRideRequest, token: &str) -> Result<Ride, String> {
    post_json("/rides", &req, Some(token)).await
}

pub async fn update_ride(id: &str, req: UpdateRideRequest, token: &str) -> Result<Ride, String> {
    put_json(&format!("/rides/{id}"), &req, token).await
}

pub async fn delete_ride(id: &str, token: &str) -> Result<(), String> {
    delete(&format!("/rides/{id}"), token).await
}

/// Join (going=true) or bail on (going=false) a ride; returns the ride with the
/// refreshed attendee list and the caller's new status.
pub async fn attend_ride(id: &str, going: bool, token: &str) -> Result<Ride, String> {
    post_json(&format!("/rides/{id}/attend"), &RsvpRequest { going }, Some(token)).await
}

// ---- Notifications ----

pub async fn fetch_notifications(token: &str) -> Result<Vec<Notification>, String> {
    get("/notifications", token).await
}

pub async fn clear_notifications(token: &str) -> Result<(), String> {
    post_unit("/notifications/clear", &(), token).await
}

pub async fn fetch_notif_prefs(token: &str) -> Result<NotificationPrefs, String> {
    get("/notifications/prefs", token).await
}

pub async fn update_notif_prefs(req: UpdateNotificationPrefs, token: &str) -> Result<NotificationPrefs, String> {
    put_json("/notifications/prefs", &req, token).await
}

pub async fn register_push_token(device_token: &str, token: &str) -> Result<(), String> {
    put_unit("/notifications/token", &RegisterPushTokenRequest { token: device_token.to_string() }, token).await
}

pub async fn unregister_push_token(device_token: &str, token: &str) -> Result<(), String> {
    delete_json("/notifications/token", &RegisterPushTokenRequest { token: device_token.to_string() }, token).await
}

pub async fn broadcast(req: BroadcastRequest, token: &str) -> Result<(), String> {
    post_unit("/notifications/broadcast", &req, token).await
}

pub async fn send_test_push(token: &str) -> Result<TestPushResponse, String> {
    post_json("/notifications/test", &(), Some(token)).await
}

// ---- Calendar subscription ----

pub async fn get_calendar_token(token: &str) -> Result<CalendarToken, String> {
    get("/calendar/me", token).await
}

pub async fn regenerate_calendar_token(token: &str) -> Result<CalendarToken, String> {
    post_json("/calendar/me/regenerate", &(), Some(token)).await
}

/// Public https URL of the member's .ics feed.
pub fn calendar_feed_url(feed_token: &str) -> String {
    format!("{}/calendar/{feed_token}/baphomet-babes.ics", api_base())
}

// ---- Group chat ----

pub async fn fetch_chat(token: &str) -> Result<Vec<ChatMessage>, String> {
    get("/chat", token).await
}

pub async fn send_chat(body: &str, token: &str) -> Result<ChatMessage, String> {
    post_json("/chat", &SendChatRequest { body: body.to_string() }, Some(token)).await
}

pub async fn fetch_users(token: &str) -> Result<Vec<UserSummary>, String> {
    get("/users", token).await
}

pub async fn update_user(id: &str, req: UpdateUserRequest, token: &str) -> Result<UserSummary, String> {
    put_json(&format!("/users/{id}"), &req, token).await
}

// ---- Gatherings ----

pub async fn fetch_gatherings(token: &str) -> Result<Vec<Gathering>, String> {
    get("/gatherings", token).await
}

pub async fn create_gathering(req: CreateGatheringRequest, token: &str) -> Result<Gathering, String> {
    post_json("/gatherings", &req, Some(token)).await
}

pub async fn delete_gathering(id: &str, token: &str) -> Result<(), String> {
    delete(&format!("/gatherings/{id}"), token).await
}

pub async fn rsvp_gathering(id: &str, going: bool, token: &str) -> Result<Gathering, String> {
    post_json(&format!("/gatherings/{id}/rsvp"), &RsvpRequest { going }, Some(token)).await
}

/// Admin-only: who's going. Members get a 403 here and see only the count.
pub async fn fetch_gathering_rsvps(id: &str, token: &str) -> Result<Vec<Rsvp>, String> {
    get(&format!("/gatherings/{id}/rsvps"), token).await
}

pub async fn upload_gathering_cover(
    content_type: &str,
    data_base64: &str,
    token: &str,
) -> Result<UploadImageResponse, String> {
    let req = UploadImageRequest {
        content_type: content_type.to_string(),
        data_base64: data_base64.to_string(),
    };
    post_json("/gatherings/cover", &req, Some(token)).await
}

pub async fn geocode_address(query: &str, token: &str) -> Result<GeocodeResponse, String> {
    let req = GeocodeRequest { query: query.to_string() };
    post_json("/gatherings/geocode", &req, Some(token)).await
}
