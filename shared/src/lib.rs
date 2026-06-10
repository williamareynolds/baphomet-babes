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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}
