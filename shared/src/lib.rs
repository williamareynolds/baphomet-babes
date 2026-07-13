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
    /// Optional meeting-spot pin. Both set together or both None. Stored as raw
    /// coordinates so the client can build "open in maps" links without ever
    /// embedding a map (and leaking every viewer's IP to a tile server).
    #[serde(default)]
    pub meeting_lat: Option<f64>,
    #[serde(default)]
    pub meeting_lng: Option<f64>,
    /// Optional free-text contact info: a phone number, email, or a link to a
    /// group chat (e.g. a Signal group invite). Rendered smartly by the client.
    #[serde(default)]
    pub contact_info: Option<String>,
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
    #[serde(default)]
    pub meeting_lat: Option<f64>,
    #[serde(default)]
    pub meeting_lng: Option<f64>,
    #[serde(default)]
    pub contact_info: Option<String>,
}

/// How a ride's free-text contact string should be presented. The
/// classification lives here (not the WASM client) so it can be unit-tested on
/// the host, and so the one place that decides "is this a safe link" is pinned
/// by tests. The client only maps each variant to markup and NEVER emits an
/// href outside http(s)/mailto/tel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContactKind {
    /// A Signal group/contact invite link — render as a labelled button.
    Signal,
    /// Any other http(s) link — render as a plain link to the raw URL.
    Web,
    /// An email address — render as a `mailto:` link.
    Email,
    /// A phone number — render as a `tel:` link using this digits/`+` string.
    Phone(String),
    /// Anything unrecognised — render as escaped plain text, never a link.
    Plain,
}

fn looks_like_email(s: &str) -> bool {
    !s.contains(char::is_whitespace)
        && s.split_once('@')
            .is_some_and(|(user, domain)| !user.is_empty() && domain.len() > 2 && domain.contains('.'))
}

/// `Some(tel)` when `s` is phone-shaped (7–15 digits, only digits and the usual
/// separators), where `tel` is the `tel:`-safe digits-and-plus reduction.
fn phone_tel(s: &str) -> Option<String> {
    let digits = s.chars().filter(char::is_ascii_digit).count();
    let shaped = (7..=15).contains(&digits)
        && s.chars().all(|c| c.is_ascii_digit() || " +-().".contains(c));
    shaped.then(|| s.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect())
}

/// Classify a (already-trimmed or not) contact string for rendering. Only
/// `https://`/`http://` inputs are ever treated as links, so an attacker can't
/// smuggle a `javascript:` (or other) scheme through the free-text field.
pub fn classify_contact(raw: &str) -> ContactKind {
    let s = raw.trim();
    if s.is_empty() {
        return ContactKind::Plain;
    }
    let is_web = s.starts_with("https://") || s.starts_with("http://");
    if is_web && (s.contains("signal.group") || s.contains("signal.me")) {
        return ContactKind::Signal;
    }
    if is_web {
        return ContactKind::Web;
    }
    if looks_like_email(s) {
        return ContactKind::Email;
    }
    if let Some(tel) = phone_tel(s) {
        return ContactKind::Phone(tel);
    }
    ContactKind::Plain
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

    #[test]
    fn contact_classifies_signal_and_web_links() {
        assert_eq!(classify_contact("https://signal.group/#CjQKIabc"), ContactKind::Signal);
        assert_eq!(classify_contact("https://signal.me/#p/+15550100"), ContactKind::Signal);
        assert_eq!(classify_contact("  https://signal.group/#x  "), ContactKind::Signal); // trimmed
        assert_eq!(classify_contact("https://chat.example.com/room"), ContactKind::Web);
        assert_eq!(classify_contact("http://example.com"), ContactKind::Web);
    }

    #[test]
    fn contact_classifies_email_and_phone() {
        assert_eq!(classify_contact("rider@example.com"), ContactKind::Email);
        assert_eq!(classify_contact("479-555-0142"), ContactKind::Phone("4795550142".into()));
        assert_eq!(classify_contact("+1 (479) 555-0142"), ContactKind::Phone("+14795550142".into()));
    }

    #[test]
    fn contact_falls_back_to_plain_text() {
        // Empty, prose, a bare mention, and — critically — an unsafe scheme all
        // stay plain text: only http(s) is ever linkified.
        assert_eq!(classify_contact(""), ContactKind::Plain);
        assert_eq!(classify_contact("   "), ContactKind::Plain);
        assert_eq!(classify_contact("ask me at the trailhead"), ContactKind::Plain);
        assert_eq!(classify_contact("javascript:alert(1)"), ContactKind::Plain);
        assert_eq!(classify_contact("ftp://files.example.com"), ContactKind::Plain);
        // "@handle" is not an email (no domain), and "123" is too short to be a phone.
        assert_eq!(classify_contact("@rider"), ContactKind::Plain);
        assert_eq!(classify_contact("123"), ContactKind::Plain);
    }
}
