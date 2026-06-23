use backend::auth::{create_token, verify_token};
use backend::models::{EventDoc, ProfileDoc};
use proptest::prelude::*;
use shared::ProfileLink;

fn role_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("member".to_string()),
        Just("admin".to_string()),
        Just("superadmin".to_string()),
    ]
}

proptest! {
    /// Any (user_id, role, secret) survives a create→verify roundtrip intact.
    #[test]
    fn token_roundtrip(
        user_id in "[a-zA-Z0-9-]{1,64}",
        role in role_strategy(),
        secret in "[ -~]{8,64}",
    ) {
        let token = create_token(&user_id, &role, &secret).unwrap();
        let claims = verify_token(&token, &secret).unwrap();
        prop_assert_eq!(claims.sub, user_id);
        prop_assert_eq!(claims.role, role);
    }

    /// A token signed with one secret never verifies under a different secret.
    #[test]
    fn wrong_secret_always_fails(
        user_id in "[a-zA-Z0-9-]{1,64}",
        secret_a in "[ -~]{8,64}",
        secret_b in "[ -~]{8,64}",
    ) {
        prop_assume!(secret_a != secret_b);
        let token = create_token(&user_id, "member", &secret_a).unwrap();
        prop_assert!(verify_token(&token, &secret_b).is_err());
    }

    /// verify_token never panics on arbitrary garbage input.
    #[test]
    fn verify_never_panics(garbage in "\\PC{0,256}", secret in "[ -~]{8,32}") {
        let _ = verify_token(&garbage, &secret);
    }

    /// ProfileDoc (Firestore wire format) roundtrips through JSON losslessly,
    /// including unicode in user-controlled fields.
    #[test]
    fn profile_doc_serde_roundtrip(
        user_id in "[a-zA-Z0-9-]{1,36}",
        username in "\\PC{1,32}",
        bio in proptest::option::of("\\PC{0,256}"),
        pronouns in proptest::option::of("\\PC{0,32}"),
        is_public in any::<bool>(),
        link_label in "\\PC{1,32}",
        link_url in "[ -~]{1,128}",
        updated_at in 0i64..=4102444800,
    ) {
        let doc = ProfileDoc {
            user_id,
            username,
            display_name: None,
            bio,
            pronouns,
            avatar_url: None,
            email: None,
            links: vec![ProfileLink { label: link_label, url: link_url }],
            is_public,
            updated_at,
        };
        let json = serde_json::to_value(&doc).unwrap();
        let back: ProfileDoc = serde_json::from_value(json.clone()).unwrap();
        let json2 = serde_json::to_value(&back).unwrap();
        prop_assert_eq!(json, json2);
    }

    /// EventDoc roundtrips losslessly.
    #[test]
    fn event_doc_serde_roundtrip(
        id in "[a-zA-Z0-9-]{1,36}",
        event_type in prop_oneof![Just("main".to_string()), Just("special".to_string())],
        title in "\\PC{1,64}",
        date in "[0-9]{4}-[0-9]{2}-[0-9]{2}",
        description in proptest::option::of("\\PC{0,128}"),
        poster_url in proptest::option::of("[ -~]{1,128}"),
        created_at in 0i64..=4102444800,
    ) {
        let doc = EventDoc {
            id,
            event_type,
            title,
            date: Some(date),
            description,
            poll_embed_url: None,
            poster_url,
            rsvp_deadline: None,
            created_at,
        };
        let json = serde_json::to_value(&doc).unwrap();
        let back: EventDoc = serde_json::from_value(json.clone()).unwrap();
        let json2 = serde_json::to_value(&back).unwrap();
        prop_assert_eq!(json, json2);
    }
}
