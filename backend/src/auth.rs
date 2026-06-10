use crate::error::{AppError, AppResult};
use axum::http::HeaderMap;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user id
    pub role: String,
    pub exp: usize,
}

pub fn create_token(user_id: &str, role: &str, secret: &str) -> AppResult<String> {
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
        + 30 * 24 * 3600; // 30 days

    let claims = Claims {
        sub: user_id.to_string(),
        role: role.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Auth(e.to_string()))
}

pub fn verify_token(token: &str, secret: &str) -> AppResult<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|d| d.claims)
    .map_err(|e| AppError::Auth(e.to_string()))
}

pub fn require_auth(headers: &HeaderMap, secret: &str) -> AppResult<Claims> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Auth("missing bearer token".into()))?;
    verify_token(token, secret)
}

pub fn require_admin(headers: &HeaderMap, secret: &str) -> AppResult<Claims> {
    let claims = require_auth(headers, secret)?;
    if claims.role != "admin" && claims.role != "superadmin" {
        return Err(AppError::Forbidden);
    }
    Ok(claims)
}

#[allow(dead_code)]
pub fn require_superadmin(headers: &HeaderMap, secret: &str) -> AppResult<Claims> {
    let claims = require_auth(headers, secret)?;
    if claims.role != "superadmin" {
        return Err(AppError::Forbidden);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const SECRET: &str = "test-secret";

    fn headers_with_bearer(token: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("Authorization", HeaderValue::from_str(&format!("Bearer {token}")).unwrap());
        h
    }

    #[test]
    fn token_roundtrip_preserves_claims() {
        let token = create_token("user-123", "member", SECRET).unwrap();
        let claims = verify_token(&token, SECRET).unwrap();
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.role, "member");
    }

    #[test]
    fn token_with_wrong_secret_fails() {
        let token = create_token("user-123", "member", SECRET).unwrap();
        assert!(verify_token(&token, "other-secret").is_err());
    }

    #[test]
    fn tampered_token_fails() {
        let token = create_token("user-123", "member", SECRET).unwrap();
        let mut tampered = token.clone();
        // flip a char in the payload segment
        let mid = token.len() / 2;
        let replacement = if &token[mid..mid + 1] == "A" { "B" } else { "A" };
        tampered.replace_range(mid..mid + 1, replacement);
        assert!(verify_token(&tampered, SECRET).is_err());
    }

    #[test]
    fn expired_token_fails() {
        let exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize
            - 120; // past the default 60s leeway
        let claims = Claims { sub: "user-123".into(), role: "member".into(), exp };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap();
        assert!(verify_token(&token, SECRET).is_err());
    }

    #[test]
    fn require_auth_missing_header() {
        let err = require_auth(&HeaderMap::new(), SECRET).unwrap_err();
        assert!(matches!(err, AppError::Auth(_)));
    }

    #[test]
    fn require_auth_rejects_non_bearer() {
        let mut h = HeaderMap::new();
        h.insert("Authorization", HeaderValue::from_static("Basic dXNlcjpwYXNz"));
        assert!(require_auth(&h, SECRET).is_err());
    }

    #[test]
    fn require_auth_accepts_valid_bearer() {
        let token = create_token("user-1", "member", SECRET).unwrap();
        let claims = require_auth(&headers_with_bearer(&token), SECRET).unwrap();
        assert_eq!(claims.sub, "user-1");
    }

    #[test]
    fn require_admin_gates_by_role() {
        let member = create_token("u", "member", SECRET).unwrap();
        let admin = create_token("u", "admin", SECRET).unwrap();
        let superadmin = create_token("u", "superadmin", SECRET).unwrap();

        assert!(matches!(
            require_admin(&headers_with_bearer(&member), SECRET).unwrap_err(),
            AppError::Forbidden
        ));
        assert!(require_admin(&headers_with_bearer(&admin), SECRET).is_ok());
        assert!(require_admin(&headers_with_bearer(&superadmin), SECRET).is_ok());
    }

    #[test]
    fn require_superadmin_rejects_admin() {
        let admin = create_token("u", "admin", SECRET).unwrap();
        let superadmin = create_token("u", "superadmin", SECRET).unwrap();

        assert!(matches!(
            require_superadmin(&headers_with_bearer(&admin), SECRET).unwrap_err(),
            AppError::Forbidden
        ));
        assert!(require_superadmin(&headers_with_bearer(&superadmin), SECRET).is_ok());
    }

    #[test]
    fn unsigned_alg_none_token_fails() {
        // Token claiming alg=none with valid-looking claims must be rejected.
        let header = r#"{"alg":"none","typ":"JWT"}"#;
        let exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize
            + 3600;
        let payload = format!(r#"{{"sub":"u","role":"superadmin","exp":{exp}}}"#);
        use base64::Engine;
        let e = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let token = format!("{}.{}.", e.encode(header), e.encode(payload));
        assert!(verify_token(&token, SECRET).is_err());
    }
}
