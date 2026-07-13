use serde::{Deserialize, Serialize};
use shared::ProfileLink;

fn default_true() -> bool {
    true
}

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
    #[serde(default)]
    pub first_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
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
    /// Optional — set None until a date is chosen (e.g. while voting).
    #[serde(default)]
    pub date: Option<String>,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    #[serde(default)]
    pub poster_url: Option<String>,
    /// Optional RSVP cutoff date ("YYYY-MM-DD"). None = RSVPs never close.
    #[serde(default)]
    pub rsvp_deadline: Option<String>,
    pub created_at: i64,
}

/// One member's "going" RSVP to an event. Doc id is `{event_id}_{user_id}` so a
/// member has at most one per event (idempotent upsert); cancelling deletes it.
/// `author` is the denormalized display name so the admin list needs no joins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RsvpDoc {
    pub id: String,
    pub event_id: String,
    pub user_id: String,
    pub author: String,
    pub created_at: i64,
}

/// A member's posted mountain bike ride. Times are naive local datetimes
/// ("YYYY-MM-DDTHH:MM") — see `shared::Ride`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideDoc {
    pub id: String,
    pub location: String,
    pub start_at: String,
    pub end_at: String,
    pub created_by: String,
    /// Denormalized creator label, resolved at post time like chat authors.
    pub created_by_name: String,
    /// Optional meeting-spot pin (both set together or both None) and optional
    /// free-text contact info. `#[serde(default)]` so rides written before these
    /// fields existed still deserialize.
    #[serde(default)]
    pub meeting_lat: Option<f64>,
    #[serde(default)]
    pub meeting_lng: Option<f64>,
    #[serde(default)]
    pub contact_info: Option<String>,
    pub created_at: i64,
}

/// One member going on a ride. Doc id is `{ride_id}_{user_id}` so joining is an
/// idempotent upsert and leaving deletes it — same shape as `RsvpDoc`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideAttendeeDoc {
    pub id: String,
    pub ride_id: String,
    pub user_id: String,
    pub author: String,
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
    /// Opt-in channel: defaults off (serde default for bool), so members written
    /// before it existed — and new members — are not pushed every chat message.
    #[serde(default)]
    pub chat: bool,
    /// Opt-in like chat: only members who ride should get ride pushes.
    #[serde(default)]
    pub mountain_bike: bool,
    /// Admin-only test channel. Defaults on; the fanout restricts delivery to
    /// admins/superadmins no matter what this says, so it's safe for members'
    /// docs to carry `true`.
    #[serde(default = "default_true")]
    pub test: bool,
    /// Per-user inbox watermark: the feed hides notifications created at or
    /// before this unix-seconds time. "Clear" sets it to now. 0 = never cleared.
    #[serde(default)]
    pub cleared_at: i64,
}

/// Per-user calendar subscription token. Doc id is the user id, so regenerating
/// overwrites in place — instantly invalidating the previous token (the feed
/// looks up by the `token` field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarTokenDoc {
    pub user_id: String,
    pub token: String,
    pub created_at: i64,
}

/// One group-chat message. Doc id is `id`. `author` is denormalized at write
/// time so the feed needs no profile joins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageDoc {
    pub id: String,
    pub user_id: String,
    pub author: String,
    pub body: String,
    pub created_at: i64,
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
    pub phone: Option<String>,
    #[serde(default)]
    pub links: Vec<ProfileLink>,
    pub is_public: bool,
    pub updated_at: i64,
}
