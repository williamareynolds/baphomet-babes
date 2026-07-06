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
    /// Optional — an event can exist (and be voted on) before a date is set.
    #[serde(default)]
    pub date: Option<String>,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    #[serde(default)]
    pub poster_url: Option<String>,
    /// Optional RSVP cutoff date ("YYYY-MM-DD"). None = RSVPs never close.
    #[serde(default)]
    pub rsvp_deadline: Option<String>,
    /// How many members have RSVP'd "going". Computed per request, not stored.
    #[serde(default)]
    pub rsvp_count: i64,
    /// Whether the requesting member has RSVP'd "going". Computed per request.
    #[serde(default)]
    pub my_rsvp: bool,
}

/// Where an event sits in its lifecycle: posted (title only) → voting (poll
/// embed set, date still unknown) → scheduled (date set — the poll, if any, is
/// implicitly closed). Derived from the fields, never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStage {
    Posted,
    Voting,
    Scheduled,
}

impl Event {
    pub fn stage(&self) -> EventStage {
        if self.date.is_some() {
            EventStage::Scheduled
        } else if self.poll_embed_url.is_some() {
            EventStage::Voting
        } else {
            EventStage::Posted
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub event_type: String,
    pub title: String,
    #[serde(default)]
    pub date: Option<String>,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    pub poster_url: Option<String>,
    #[serde(default)]
    pub rsvp_deadline: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateEventRequest {
    pub event_type: Option<String>,
    pub title: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    pub poll_embed_url: Option<String>,
    pub poster_url: Option<String>,
    pub rsvp_deadline: Option<String>,
}

/// Member's RSVP action for an event: going (true) or cancel (false).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RsvpRequest {
    pub going: bool,
}

/// One "going" RSVP, as shown to admins (who can see who's attending).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rsvp {
    pub user_id: String,
    pub author: String,
    pub created_at: i64,
}

// Mountain bike rides
//
// Any member can post that they're heading out to ride; others tap "join".
// Times are naive local datetimes ("YYYY-MM-DDTHH:MM") — every trail is in
// Bentonville, so everyone shares a wall clock and lexicographic order is
// chronological order.
pub const RIDE_LOCATIONS: &[&str] = &[
    "Bike Park",
    "Slaughter Pen",
    "Coler",
    "Blowing Springs",
    "Railyard",
    "Little Sugar",
    "Back 40",
    "Handcut Hollow",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ride {
    pub id: String,
    pub location: String,
    pub start_at: String, // "YYYY-MM-DDTHH:MM"
    pub end_at: String,   // "YYYY-MM-DDTHH:MM"
    pub created_by: String,
    pub created_by_name: String,
    pub created_at: i64,
    /// Display names of everyone going (creator included), in join order.
    /// Unlike movie-night RSVPs these are visible to all members — knowing who
    /// you're riding with is the point.
    #[serde(default)]
    pub attendees: Vec<String>,
    /// Whether the requesting member is going. Computed per request.
    #[serde(default)]
    pub my_attending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRideRequest {
    pub location: String,
    pub start_at: String,
    pub end_at: String,
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
    pub phone: Option<String>,
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
    pub phone: Option<String>,
    pub links: Option<Vec<ProfileLink>>,
    pub is_public: Option<bool>,
}

// Invite codes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCode {
    pub id: String,
    pub code: String,
    pub role: String, // "admin" | "member"
    /// The person this code was minted for. Required when creating; older codes
    /// minted before this field existed deserialize to an empty string.
    #[serde(default)]
    pub first_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    pub created_by: String,
    pub used: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInviteRequest {
    pub role: String, // "admin" | "member"
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
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
    /// How many devices this user has enrolled for push notifications.
    #[serde(default)]
    pub device_count: i64,
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
pub const CHANNEL_CHAT: &str = "chat";
pub const CHANNEL_MOUNTAIN_BIKE: &str = "mountain_bike";
/// Admin-only channel for exercising the push pipeline without bothering
/// members: only admins/superadmins ever receive it, and it skips the inbox.
pub const CHANNEL_TEST: &str = "test";

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
    pub chat: bool,
    #[serde(default)]
    pub mountain_bike: bool,
    /// Admin-only test channel. Defaults on — the backend restricts delivery
    /// to admins/superadmins regardless of what's stored here.
    #[serde(default = "default_true")]
    pub test: bool,
}

fn default_true() -> bool {
    true
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        // Chat is opt-in (off by default) — it's the highest-volume channel, so
        // members shouldn't get pushed every message until they choose to.
        // Mountain bike is opt-in too: not everyone rides.
        NotificationPrefs {
            announcements: true,
            general: true,
            movie_night: true,
            chat: false,
            mountain_bike: false,
            test: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateNotificationPrefs {
    pub announcements: Option<bool>,
    pub general: Option<bool>,
    pub movie_night: Option<bool>,
    pub chat: Option<bool>,
    pub mountain_bike: Option<bool>,
    #[serde(default)]
    pub test: Option<bool>,
}

/// Register (or refresh) an FCM device token for the current user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPushTokenRequest {
    pub token: String,
}

/// Result of a self-serve test push: how many of the caller's devices are
/// enrolled and how many actually accepted the send. Lets members verify the
/// whole delivery path end-to-end from the profile page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestPushResponse {
    pub devices: usize,
    pub sent: usize,
    /// Present when push is disabled server-side or a send failed.
    #[serde(default)]
    pub detail: Option<String>,
}

// Group chat
//
// One whole-group room. Messages carry a denormalized author label (display name
// or username, resolved at post time) so the feed renders without per-message
// profile lookups.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    pub user_id: String,
    pub author: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendChatRequest {
    pub body: String,
}

/// Admin broadcast. `channel` may be the General channel (default, everyone)
/// or the Test channel (delivered only to admins/superadmins, skips the inbox).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastRequest {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub channel: Option<String>,
}

// Calendar subscription
//
// Each member gets a secret, revocable token. The ICS feed lives at a public
// capability URL carrying that token; calendar apps fetch it anonymously, so the
// token is the only credential. Regenerating rotates it (instantly killing the
// old link).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalendarToken {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(date: Option<&str>, poll: Option<&str>) -> Event {
        Event {
            id: "e1".into(),
            event_type: "main".into(),
            title: "The Wicker Man".into(),
            date: date.map(String::from),
            description: None,
            poll_embed_url: poll.map(String::from),
            poster_url: None,
            rsvp_deadline: None,
            rsvp_count: 0,
            my_rsvp: false,
        }
    }

    #[test]
    fn stage_follows_the_poll_lifecycle() {
        assert_eq!(event(None, None).stage(), EventStage::Posted);
        assert_eq!(event(None, Some("https://rcv123.org/p/1")).stage(), EventStage::Voting);
        // A date closes the poll — dated events are scheduled even if the
        // embed URL is still around for the archive.
        assert_eq!(event(Some("2030-10-31"), Some("https://rcv123.org/p/1")).stage(), EventStage::Scheduled);
        assert_eq!(event(Some("2030-10-31"), None).stage(), EventStage::Scheduled);
    }
}
