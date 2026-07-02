//! Golden tests: pin the JSON wire formats shared with the WASM clients and
//! Firestore. A failure here means a breaking change to data either in flight
//! or at rest — fix the code or consciously update the golden file.

use backend::models::{EventDoc, ProfileDoc};
use serde_json::Value;
use shared::{AuthResponse, ErrorResponse, Event, InviteCode, Profile, ProfileLink, UserInfo};

fn golden(name: &str) -> Value {
    let path = format!("{}/tests/golden/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {path}: {e}"))
}

#[test]
fn auth_response_wire_format() {
    let value = serde_json::to_value(AuthResponse {
        token: "eyJhbGciOiJIUzI1NiJ9.fixed.signature".into(),
        user: UserInfo {
            id: "5e1e3b9f-0000-4000-8000-000000000001".into(),
            email: "babe@baphometbabes.com".into(),
            username: "firstbabe".into(),
            role: "superadmin".into(),
        },
    })
    .unwrap();
    assert_eq!(value, golden("auth_response"));
}

#[test]
fn event_wire_format() {
    let value = serde_json::to_value(Event {
        id: "5e1e3b9f-0000-4000-8000-000000000002".into(),
        event_type: "main".into(),
        title: "The Witch".into(),
        date: Some("2026-06-13".to_string()),
        description: Some("A24 night".into()),
        poll_embed_url: Some("https://rcv123.org/embed/abc".into()),
        poster_url: Some("https://example.com/poster.jpg".into()),
        rsvp_deadline: Some("2026-06-10".into()),
        rsvp_count: 3,
        my_rsvp: true,
    })
    .unwrap();
    assert_eq!(value, golden("event"));
}

#[test]
fn profile_wire_format() {
    let value = serde_json::to_value(Profile {
        user_id: "5e1e3b9f-0000-4000-8000-000000000001".into(),
        username: "firstbabe".into(),
        display_name: Some("First Babe".into()),
        bio: Some("Crafts, cosmos, and cinema.".into()),
        pronouns: Some("they/them".into()),
        avatar_url: Some("https://example.com/avatar.png".into()),
        email: Some("babe@baphometbabes.com".into()),
        phone: Some("479-555-0142".into()),
        links: vec![ProfileLink { label: "Website".into(), url: "https://example.com".into() }],
        is_public: true,
        updated_at: 1781136000,
    })
    .unwrap();
    assert_eq!(value, golden("profile"));
}

#[test]
fn error_response_wire_format() {
    let value = serde_json::to_value(ErrorResponse { error: "invalid credentials".into() }).unwrap();
    assert_eq!(value, golden("error_response"));
}

#[test]
fn invite_code_wire_format() {
    let value = serde_json::to_value(InviteCode {
        id: "5e1e3b9f-0000-4000-8000-000000000003".into(),
        code: "JOIN-US-666".into(),
        role: "member".into(),
        first_name: "Ada".into(),
        last_name: Some("Lovelace".into()),
        phone: Some("555-0101".into()),
        created_by: "5e1e3b9f-0000-4000-8000-000000000001".into(),
        used: false,
        created_at: 1781136000,
    })
    .unwrap();
    assert_eq!(value, golden("invite_code"));
}

/// Firestore docs written before poster_url existed must still deserialize.
#[test]
fn legacy_event_doc_still_deserializes() {
    let doc: EventDoc = serde_json::from_value(golden("legacy_event_doc")).unwrap();
    assert_eq!(doc.title, "Bike Ride");
    assert_eq!(doc.poster_url, None);
}

/// Profile docs containing only the required fields must still deserialize —
/// every optional field rides on #[serde(default)].
#[test]
fn legacy_profile_doc_still_deserializes() {
    let doc: ProfileDoc = serde_json::from_value(golden("legacy_profile_doc")).unwrap();
    assert_eq!(doc.username, "oldmember");
    assert_eq!(doc.display_name, None);
    assert_eq!(doc.bio, None);
    assert!(doc.links.is_empty());
    assert!(!doc.is_public);
}

/// Golden files themselves roundtrip: file → struct → JSON equals file.
#[test]
fn golden_files_roundtrip() {
    let event: Event = serde_json::from_value(golden("event")).unwrap();
    assert_eq!(serde_json::to_value(&event).unwrap(), golden("event"));

    let profile: Profile = serde_json::from_value(golden("profile")).unwrap();
    assert_eq!(serde_json::to_value(&profile).unwrap(), golden("profile"));

    let auth: AuthResponse = serde_json::from_value(golden("auth_response")).unwrap();
    assert_eq!(serde_json::to_value(&auth).unwrap(), golden("auth_response"));
}
