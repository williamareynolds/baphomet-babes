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
    /// Optional voting cutoff ("YYYY-MM-DD"). Nothing enforces it — the poll
    /// lives on rcv123 — but it gives the reminder job a date to nudge against,
    /// which is the whole point of collecting it.
    #[serde(default)]
    pub poll_deadline: Option<String>,
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

/// Split the schedule into the headline "next feature" and the rest of the list.
///
/// The feature is the soonest dated screening today-or-later; with nothing
/// dated, an undated pick — preferring one with an open poll — so an event
/// mid-vote still headlines as "Date TBD".
///
/// The remainder leads with undated screenings, then dated ones newest-first.
/// Undated means voting is still open, which is the only part of the list a
/// member can act on; burying it under years of past screenings hides the one
/// thing that needs them. The feature is removed from the remainder so it
/// isn't rendered twice in a row.
pub fn split_events(mut list: Vec<Event>, today: &str) -> (Option<Event>, Vec<Event>) {
    list.sort_by(|a, b| a.date.cmp(&b.date));
    let featured = list
        .iter()
        .find(|e| e.date.as_deref().is_some_and(|d| d >= today))
        .or_else(|| list.iter().find(|e| e.date.is_none() && e.poll_embed_url.is_some()))
        .or_else(|| list.iter().find(|e| e.date.is_none()))
        .cloned();

    if let Some(f) = &featured {
        list.retain(|e| e.id != f.id);
    }

    // Stable sort, so undated screenings keep the order the backend sent them.
    list.sort_by(|a, b| match (a.date.is_none(), b.date.is_none()) {
        (true, false) => core::cmp::Ordering::Less,
        (false, true) => core::cmp::Ordering::Greater,
        _ => b.date.cmp(&a.date),
    });
    (featured, list)
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
    #[serde(default)]
    pub poll_deadline: Option<String>,
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
    pub poll_deadline: Option<String>,
}

// Gatherings
//
// A club get-together that is not a screening: no poll, no voting — the date,
// time and place are decided up front and stated. Kept apart from `Event`
// precisely because those fields are required here and optional there; folding
// them together would leave the requirement living in validation instead of the
// type, and would drag gatherings through the movie-night voting flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gathering {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Naive local datetime, "YYYY-MM-DDTHH:MM" — the club is all in one
    /// timezone, so wall-clock time is what people mean. Same convention as
    /// rides, where lexicographic order is also chronological order.
    pub starts_at: String,
    #[serde(default)]
    pub ends_at: Option<String>,
    /// Human-readable address. Either this or a pin is required; both is best,
    /// since an address is what someone pastes into their phone's maps app.
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
    /// Uploaded cover image, stored in our media bucket.
    #[serde(default)]
    pub cover_url: Option<String>,
    pub created_by: String,
    pub created_at: i64,
    /// How many members are going. Computed per request, never stored.
    #[serde(default)]
    pub rsvp_count: i64,
    /// Whether the requesting member is going. Computed per request.
    #[serde(default)]
    pub my_rsvp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGatheringRequest {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub starts_at: String,
    #[serde(default)]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
    #[serde(default)]
    pub cover_url: Option<String>,
}

/// Edit an existing gathering. `Some(_)` replaces, `None` keeps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateGatheringRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub address: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    /// Send both coordinates to move the pin, or neither to keep it. Dropping
    /// the pin entirely needs `clear_pin` — `None` can't mean "clear".
    #[serde(default)]
    pub clear_pin: bool,
    pub cover_url: Option<String>,
}

/// Cover image upload. The bytes ride as base64 in JSON rather than multipart:
/// it keeps the wasm client to `fetch` with a JSON body, and at a 5 MB cap the
/// ~33% encoding overhead is irrelevant next to Cloud Run's 32 MB request limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadImageRequest {
    pub content_type: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadImageResponse {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeocodeRequest {
    pub query: String,
}

/// A geocode miss is `found: false` rather than an error — the admin form just
/// leaves the pin to the human.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeocodeResponse {
    pub found: bool,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Where a gathering will be. At least one form is required, so a member always
/// has something to navigate by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GatheringPlace<'a> {
    pub address: Option<&'a str>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

/// Validate the parts of a gathering that don't need a database.
///
/// Shared rather than backend-local so the form and the API agree on the rules
/// and the rules can be unit-tested without an emulator.
pub fn validate_gathering(
    title: &str,
    starts_at: &str,
    ends_at: Option<&str>,
    place: GatheringPlace<'_>,
) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("a gathering needs a title".into());
    }
    if !valid_local_datetime(starts_at) {
        return Err("start must be YYYY-MM-DDTHH:MM".into());
    }
    if let Some(end) = ends_at.filter(|e| !e.is_empty()) {
        if !valid_local_datetime(end) {
            return Err("end must be YYYY-MM-DDTHH:MM".into());
        }
        if end <= starts_at {
            return Err("the gathering must end after it starts".into());
        }
    }
    match (place.lat, place.lng) {
        (Some(lat), Some(lng)) => {
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
                return Err("that pin is off the map".into());
            }
        }
        (None, None) => {}
        _ => return Err("a pin needs both coordinates".into()),
    }
    let has_address = place.address.map(|a| !a.trim().is_empty()).unwrap_or(false);
    let has_pin = place.lat.is_some() && place.lng.is_some();
    if !has_address && !has_pin {
        return Err("a gathering needs an address or a pin on the map".into());
    }
    Ok(())
}

/// Validate a "YYYY-MM-DDTHH:MM" naive local datetime — what
/// `<input type="datetime-local">` produces.
pub fn valid_local_datetime(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 16
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && s.chars().enumerate().all(|(i, c)| matches!(i, 4 | 7 | 10 | 13) || c.is_ascii_digit())
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
    /// Optional free-text notes: weather/cancellation caveats, landmarks to find
    /// the group, pace — whatever the poster wants to add. Plain escaped text.
    #[serde(default)]
    pub notes: Option<String>,
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
    #[serde(default)]
    pub notes: Option<String>,
}

/// Edit an existing ride. Every field is optional: `Some(_)` replaces, `None`
/// keeps the stored value. The creator or any admin/superadmin may send this.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateRideRequest {
    pub location: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    /// New pin: send both coordinates to move it, or neither to keep it. To drop
    /// the pin entirely set `clear_meeting` (None here can't mean "clear").
    pub meeting_lat: Option<f64>,
    pub meeting_lng: Option<f64>,
    #[serde(default)]
    pub clear_meeting: bool,
    /// `Some("")`/whitespace clears, `Some(x)` sets, `None` keeps — for both.
    pub contact_info: Option<String>,
    pub notes: Option<String>,
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
pub const CHANNEL_GATHERINGS: &str = "gatherings";
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
    /// Club gatherings. On by default, like movie nights — it's a whole-club
    /// event with a date people need to plan around.
    #[serde(default = "default_true")]
    pub gatherings: bool,
    /// Email delivery, per channel. Independent of the push flags above: a
    /// member can take movie night by email only, push only, or both.
    #[serde(default)]
    pub email: EmailPrefs,
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
            gatherings: true,
            email: EmailPrefs::default(),
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
    #[serde(default)]
    pub gatherings: Option<bool>,
    #[serde(default)]
    pub email: Option<UpdateEmailPrefs>,
}

/// Per-channel *email* delivery, parallel to the push flags above.
///
/// Only movie night is on by default. Email is the loud channel — it reaches
/// people who never installed the PWA or turned push on, which is the whole
/// point, but a club that emails about everything gets filtered. Members opt
/// into the rest themselves.
///
/// Chat has no entry on purpose: it delivers via `push_only` and never reaches
/// the email fan-out, because per-message email would be unusable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmailPrefs {
    pub announcements: bool,
    pub general: bool,
    pub movie_night: bool,
    pub mountain_bike: bool,
    pub gatherings: bool,
}

impl Default for EmailPrefs {
    fn default() -> Self {
        EmailPrefs {
            announcements: false,
            general: false,
            movie_night: true,
            // On by default alongside movie nights: a gathering has a fixed
            // date, so a member who misses the push has missed the event.
            gatherings: true,
            mountain_bike: false,
        }
    }
}

impl EmailPrefs {
    /// Whether this member wants email for `channel`. Unknown channels — chat,
    /// the admin test channel, anything added later — are false: a new channel
    /// has to opt itself into email explicitly rather than inheriting it.
    pub fn allows(&self, channel: &str) -> bool {
        match channel {
            CHANNEL_ANNOUNCEMENTS => self.announcements,
            CHANNEL_GENERAL => self.general,
            CHANNEL_MOVIE_NIGHT => self.movie_night,
            CHANNEL_MOUNTAIN_BIKE => self.mountain_bike,
            CHANNEL_GATHERINGS => self.gatherings,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateEmailPrefs {
    pub announcements: Option<bool>,
    pub general: Option<bool>,
    pub movie_night: Option<bool>,
    pub mountain_bike: Option<bool>,
    pub gatherings: Option<bool>,
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

    fn place(address: Option<&str>, lat: Option<f64>, lng: Option<f64>) -> GatheringPlace<'_> {
        GatheringPlace { address, lat, lng }
    }

    const START: &str = "2026-09-01T18:30";

    #[test]
    fn a_gathering_needs_a_place() {
        // The whole point of a gathering is that people can turn up to it.
        let err = validate_gathering("Potluck", START, None, place(None, None, None)).unwrap_err();
        assert!(err.contains("address or a pin"), "got: {err}");
    }

    #[test]
    fn either_form_of_place_is_enough() {
        assert!(validate_gathering("Potluck", START, None, place(Some("905 NW 10th St"), None, None)).is_ok());
        assert!(validate_gathering("Potluck", START, None, place(None, Some(36.37), Some(-94.20))).is_ok());
        assert!(validate_gathering("Potluck", START, None, place(Some("905 NW 10th St"), Some(36.37), Some(-94.20))).is_ok());
    }

    #[test]
    fn a_blank_address_does_not_count_as_a_place() {
        let err = validate_gathering("Potluck", START, None, place(Some("   "), None, None)).unwrap_err();
        assert!(err.contains("address or a pin"), "got: {err}");
    }

    #[test]
    fn half_a_pin_is_rejected() {
        let err = validate_gathering("Potluck", START, None, place(Some("somewhere"), Some(36.37), None)).unwrap_err();
        assert!(err.contains("both coordinates"), "got: {err}");
    }

    #[test]
    fn pins_must_be_on_earth() {
        let err = validate_gathering("Potluck", START, None, place(None, Some(91.0), Some(0.0))).unwrap_err();
        assert!(err.contains("off the map"), "got: {err}");
        let err = validate_gathering("Potluck", START, None, place(None, Some(0.0), Some(181.0))).unwrap_err();
        assert!(err.contains("off the map"), "got: {err}");
    }

    #[test]
    fn start_time_is_required_and_shaped() {
        // Date alone won't do: a gathering states a time, unlike a movie night
        // whose date comes out of a poll.
        assert!(validate_gathering("Potluck", "2026-09-01", None, place(Some("x"), None, None)).is_err());
        assert!(validate_gathering("Potluck", "", None, place(Some("x"), None, None)).is_err());
        assert!(validate_gathering("Potluck", "2026-09-01T18:30", None, place(Some("x"), None, None)).is_ok());
    }

    #[test]
    fn an_end_time_must_follow_the_start() {
        let err = validate_gathering("Potluck", START, Some("2026-09-01T17:00"), place(Some("x"), None, None)).unwrap_err();
        assert!(err.contains("end after it starts"), "got: {err}");
        assert!(validate_gathering("Potluck", START, Some("2026-09-01T21:00"), place(Some("x"), None, None)).is_ok());
        // Absent or blank is fine — an end time is optional.
        assert!(validate_gathering("Potluck", START, None, place(Some("x"), None, None)).is_ok());
        assert!(validate_gathering("Potluck", START, Some(""), place(Some("x"), None, None)).is_ok());
    }

    #[test]
    fn a_title_is_required() {
        let err = validate_gathering("  ", START, None, place(Some("x"), None, None)).unwrap_err();
        assert!(err.contains("title"), "got: {err}");
    }

    #[test]
    fn local_datetime_shape() {
        assert!(valid_local_datetime("2026-09-01T18:30"));
        assert!(!valid_local_datetime("2026-09-01T18:30:00")); // seconds
        assert!(!valid_local_datetime("2026-9-01T18:30"));     // unpadded
        assert!(!valid_local_datetime("2026-09-01 18:30"));    // space
        assert!(!valid_local_datetime(""));
    }

    fn ev(id: &str, date: Option<&str>, poll: Option<&str>) -> Event {
        Event {
            id: id.into(),
            event_type: "main".into(),
            title: id.into(),
            date: date.map(String::from),
            description: None,
            poll_embed_url: poll.map(String::from),
            poster_url: None,
            rsvp_deadline: None,
            poll_deadline: None,
            rsvp_count: 0,
            my_rsvp: false,
        }
    }

    fn ids(list: &[Event]) -> Vec<&str> {
        list.iter().map(|e| e.id.as_str()).collect()
    }

    const TODAY: &str = "2026-08-07";

    #[test]
    fn features_the_soonest_upcoming_screening() {
        let (f, _) = split_events(
            vec![
                ev("far", Some("2026-12-01"), None),
                ev("soon", Some("2026-08-20"), None),
                ev("past", Some("2026-01-01"), None),
            ],
            TODAY,
        );
        assert_eq!(f.unwrap().id, "soon");
    }

    #[test]
    fn features_today() {
        let (f, _) = split_events(vec![ev("today", Some(TODAY), None)], TODAY);
        assert_eq!(f.unwrap().id, "today");
    }

    #[test]
    fn falls_back_to_the_event_being_voted_on() {
        // Nothing dated ahead, so the poll in progress headlines instead of a
        // past screening.
        let (f, _) = split_events(
            vec![
                ev("past", Some("2026-01-01"), None),
                ev("planned", None, None),
                ev("voting", None, Some("https://rcv123.org/p/1")),
            ],
            TODAY,
        );
        assert_eq!(f.unwrap().id, "voting");
    }

    #[test]
    fn the_feature_is_not_repeated_in_the_list() {
        let (f, rest) = split_events(
            vec![ev("soon", Some("2026-08-20"), None), ev("past", Some("2026-01-01"), None)],
            TODAY,
        );
        assert_eq!(f.unwrap().id, "soon");
        assert_eq!(ids(&rest), ["past"]);
    }

    #[test]
    fn undated_screenings_lead_the_list() {
        // "voting" headlines; the other undated entries come before every dated
        // one, which are then newest-first.
        let (f, rest) = split_events(
            vec![
                ev("old", Some("2026-01-01"), None),
                ev("undated_a", None, None),
                ev("voting", None, Some("https://rcv123.org/p/1")),
                ev("recent", Some("2026-06-01"), None),
                ev("undated_b", None, None),
            ],
            TODAY,
        );
        assert_eq!(f.unwrap().id, "voting");
        assert_eq!(ids(&rest), ["undated_a", "undated_b", "recent", "old"]);
    }

    #[test]
    fn an_empty_schedule_features_nothing() {
        let (f, rest) = split_events(vec![], TODAY);
        assert!(f.is_none());
        assert!(rest.is_empty());
    }

    #[test]
    fn email_defaults_to_movie_night_only() {
        // The turnout channel is on; everything else is opt-in so club mail
        // stays rare enough to get opened.
        let p = EmailPrefs::default();
        assert!(p.movie_night);
        assert!(!p.announcements);
        assert!(!p.general);
        assert!(!p.mountain_bike);
    }

    #[test]
    fn email_allows_maps_channels_to_flags() {
        let p = EmailPrefs { announcements: true, general: false, movie_night: true, mountain_bike: false, gatherings: true };
        assert!(p.allows(CHANNEL_ANNOUNCEMENTS));
        assert!(p.allows(CHANNEL_MOVIE_NIGHT));
        assert!(!p.allows(CHANNEL_GENERAL));
        assert!(!p.allows(CHANNEL_MOUNTAIN_BIKE));
    }

    #[test]
    fn email_never_allows_chat_or_unknown_channels() {
        // Chat delivers via push_only and must never reach the email fan-out;
        // a channel added later has to opt in explicitly rather than inherit.
        let p = EmailPrefs { announcements: true, general: true, movie_night: true, mountain_bike: true, gatherings: true };
        assert!(!p.allows(CHANNEL_CHAT));
        assert!(!p.allows(CHANNEL_TEST));
        assert!(!p.allows("something_new"));
    }

    fn event(date: Option<&str>, poll: Option<&str>) -> Event {
        Event {
            id: "e1".into(),
            event_type: "main".into(),
            title: "The Wicker Man".into(),
            date: date.map(String::from),
            description: None,
            poll_embed_url: poll.map(String::from),
            poster_url: None,
            poll_deadline: None,
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
