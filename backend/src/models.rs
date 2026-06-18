use serde::{Deserialize, Serialize};
use shared::ProfileLink;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDoc {
    pub id: String,
    pub email: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    #[serde(default)]
    pub disabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCodeDoc {
    pub id: String,
    pub code: String,
    pub role: String,
    pub created_by: String,
    pub used: bool,
    pub used_by: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDoc {
    pub id: String,
    pub event_type: String,
    pub title: String,
    pub date: String,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    #[serde(default)]
    pub poster_url: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncementDoc {
    pub id: String,
    pub title: String,
    pub body: String,
    pub poll_embed_url: Option<String>,
    pub created_by: String,
    pub created_at: i64,
}

/// One registered FCM device token. Doc id is the token itself, so re-registering
/// the same device is an idempotent upsert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushTokenDoc {
    pub token: String,
    pub user_id: String,
    pub created_at: i64,
}

/// Per-user channel subscriptions. Doc id is the user id. Absence means defaults
/// (all channels on).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifPrefsDoc {
    pub user_id: String,
    pub announcements: bool,
    pub general: bool,
    pub movie_night: bool,
    /// Per-user inbox watermark: the feed hides notifications created at or
    /// before this unix-seconds time. "Clear" sets it to now. 0 = never cleared.
    #[serde(default)]
    pub cleared_at: i64,
}

/// A persisted notification record powering the inbox feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationDoc {
    pub id: String,
    pub channel: String,
    pub title: String,
    pub body: String,
    pub url: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDoc {
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
