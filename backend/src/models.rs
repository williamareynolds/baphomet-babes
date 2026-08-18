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
    /// Optional voting cutoff ("YYYY-MM-DD") the reminder job nudges against.
    #[serde(default)]
    pub poll_deadline: Option<String>,
    /// Unix seconds when the closing-soon reminder went out, so a job that runs
    /// daily sends it once rather than every day until the poll closes. 0 =
    /// never sent.
    #[serde(default)]
    pub poll_reminder_sent_at: i64,
    pub created_at: i64,
}

/// A club gathering: stated date, time and place, no poll. Doc id is `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatheringDoc {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Naive local datetime "YYYY-MM-DDTHH:MM"; lexicographic order is
    /// chronological order, same as rides.
    pub starts_at: String,
    #[serde(default)]
    pub ends_at: Option<String>,
    /// At least one of `address` or the pin is always set — enforced on write
    /// by `shared::validate_gathering`, not by the type.
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
    #[serde(default)]
    pub cover_url: Option<String>,
    pub created_by: String,
    pub created_at: i64,
}

/// One member going to a gathering. Doc id is `{gathering_id}_{user_id}` so
/// RSVPing is an idempotent upsert and cancelling deletes it — the same shape
/// as `RsvpDoc`. `author` is denormalized so the admin list needs no joins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatheringRsvpDoc {
    pub id: String,
    pub gathering_id: String,
    pub user_id: String,
    pub author: String,
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
    /// Optional free-text notes (weather/cancellation caveats, landmarks, pace).
    #[serde(default)]
    pub notes: Option<String>,
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
    /// Club gatherings, on by default like movie nights.
    #[serde(default = "default_true")]
    pub gatherings: bool,
    /// Per-user inbox watermark: the feed hides notifications created at or
    /// before this unix-seconds time. "Clear" sets it to now. 0 = never cleared.
    #[serde(default)]
    pub cleared_at: i64,
    // ---- email delivery, per channel ----
    //
    // Stored flat rather than as a nested map so the doc stays queryable and
    // matches the shape of the push flags above; the API type nests them under
    // `email` for the UI's benefit. Every flag defaults off except movie night,
    // so members written before email existed are opted into the vote nudge and
    // nothing else.
    #[serde(default)]
    pub email_announcements: bool,
    #[serde(default)]
    pub email_general: bool,
    #[serde(default = "default_true")]
    pub email_movie_night: bool,
    #[serde(default)]
    pub email_mountain_bike: bool,
    #[serde(default = "default_true")]
    pub email_gatherings: bool,
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

/// A calendar link issued to a non-member. Same capability-URL shape as the
/// per-member token, but keyed by its own id rather than a user id, so one
/// person can hold several and each is revocable on its own. Revoking deletes
/// the doc outright — the link 404s immediately and we stop holding a
/// non-member's name and phone number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCalendarDoc {
    pub id: String,
    pub name: String,
    pub phone: String,
    pub token: String,
    pub created_at: i64,
    pub created_by: String,
}

/// Per-user unsubscribe token, same capability-URL shape as the calendar token
/// above: doc id is the user id, lookup is by the `token` field. Deliberately
/// separate from the calendar token so a link forwarded out of someone's inbox
/// can only turn email off — it can't also expose their event feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailTokenDoc {
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
