//! Read-only peek at a JWT's payload.
//!
//! The client never *verifies* anything — the backend is the only authority on
//! whether a token is good. All we want is the `exp` claim, so the app can tell
//! that a stored session is already dead before it fires a request that is
//! guaranteed to 401.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

/// The `exp` claim (seconds since the epoch) from an unverified JWT, or `None`
/// when the token isn't a parseable three-segment JWT carrying a numeric `exp`.
pub fn exp_seconds(token: &str) -> Option<u64> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims.get("exp")?.as_u64()
}

/// Whether `token` is already past its `exp` at `now_seconds`. Tokens we can't
/// parse are treated as live: the backend rejects them, and guessing here would
/// log people out over a claim we never understood.
pub fn is_expired(token: &str, now_seconds: u64) -> bool {
    match exp_seconds(token) {
        Some(exp) => now_seconds >= exp,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    /// Build a token with the given payload. The signature is never inspected
    /// here, so a placeholder is enough.
    fn token_with(payload: &str) -> String {
        let e = URL_SAFE_NO_PAD;
        format!(
            "{}.{}.sig",
            e.encode(r#"{"alg":"HS256","typ":"JWT"}"#),
            e.encode(payload)
        )
    }

    #[test]
    fn reads_exp_claim() {
        let t = token_with(r#"{"sub":"u1","role":"member","exp":1893456000}"#);
        assert_eq!(exp_seconds(&t), Some(1893456000));
    }

    #[test]
    fn expired_when_now_is_past_exp() {
        let t = token_with(r#"{"exp":1000}"#);
        assert!(is_expired(&t, 1001));
        assert!(is_expired(&t, 1000)); // exp is the last valid instant
        assert!(!is_expired(&t, 999));
    }

    #[test]
    fn unparseable_tokens_are_left_to_the_backend() {
        assert_eq!(exp_seconds("not-a-jwt"), None);
        assert_eq!(exp_seconds(""), None);
        assert!(!is_expired("not-a-jwt", 9_999_999_999));
    }

    #[test]
    fn payload_without_exp_is_not_expired() {
        let t = token_with(r#"{"sub":"u1"}"#);
        assert_eq!(exp_seconds(&t), None);
        assert!(!is_expired(&t, 9_999_999_999));
    }

    #[test]
    fn non_numeric_exp_is_ignored() {
        let t = token_with(r#"{"exp":"soon"}"#);
        assert_eq!(exp_seconds(&t), None);
    }
}
