use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub username: String,
    pub role: String,
    pub token: String,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin" || self.role == "superadmin"
    }
    pub fn is_superadmin(&self) -> bool {
        self.role == "superadmin"
    }
}

/// Stored in cross-domain cookie — no JWT token.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CookieIdentity {
    pub id: String,
    pub email: String,
    pub username: String,
    pub role: String,
}

impl From<&AuthUser> for CookieIdentity {
    fn from(u: &AuthUser) -> Self {
        CookieIdentity { id: u.id.clone(), email: u.email.clone(), username: u.username.clone(), role: u.role.clone() }
    }
}

// Auth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub username: String,
    pub password: String,
    pub invite_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub username: String,
    pub role: String,
}

// Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: String, // "main" | "special"
    pub title: String,
    pub date: String,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    #[serde(default)]
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub event_type: String,
    pub title: String,
    pub date: String,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEventRequest {
    pub event_type: Option<String>,
    pub title: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    pub poster_url: Option<String>,
}

// Announcements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub body: String,
    pub poll_embed_url: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAnnouncementRequest {
    pub title: String,
    pub body: String,
    pub poll_embed_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateAnnouncementRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub poll_embed_url: Option<String>,
}

// Profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLink {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub user_id: String,
    pub username: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub pronouns: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub links: Vec<ProfileLink>,
    pub is_public: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub pronouns: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub links: Option<Vec<ProfileLink>>,
    pub is_public: Option<bool>,
}

// Invite codes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCode {
    pub id: String,
    pub code: String,
    pub role: String, // "admin" | "member"
    pub created_by: String,
    pub used: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInviteRequest {
    pub role: String, // "admin" | "member"
}

// User administration (superadmin control panel)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserSummary {
    pub id: String,
    pub email: String,
    pub username: String,
    pub role: String, // "superadmin" | "admin" | "member"
    pub disabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUserRequest {
    pub role: Option<String>,
    pub disabled: Option<bool>,
}

// Notifications
//
// Channels a notification can belong to. Members opt in/out per channel; pushes
// and the inbox both respect these.
pub const CHANNEL_ANNOUNCEMENTS: &str = "announcements";
pub const CHANNEL_GENERAL: &str = "general";
pub const CHANNEL_MOVIE_NIGHT: &str = "movie_night";

/// A delivered notification, as shown in the inbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notification {
    pub id: String,
    pub channel: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub url: Option<String>,
    pub created_at: i64,
}

/// Per-user channel subscriptions. Default is all-on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationPrefs {
    pub announcements: bool,
    pub general: bool,
    pub movie_night: bool,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        NotificationPrefs { announcements: true, general: true, movie_night: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateNotificationPrefs {
    pub announcements: Option<bool>,
    pub general: Option<bool>,
    pub movie_night: Option<bool>,
}

/// Register (or refresh) an FCM device token for the current user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPushTokenRequest {
    pub token: String,
}

/// Admin broadcast to the General channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastRequest {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}
