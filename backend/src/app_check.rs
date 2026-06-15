//! Firebase App Check enforcement.
//!
//! App Check issues a short-lived RS256 JWT to each genuine instance of our web
//! apps (attested via reCAPTCHA Enterprise). Clients send it in the
//! `X-Firebase-AppCheck` header. Verifying it here means scripted bots that
//! never ran our frontend are rejected before they can touch Firestore or burn
//! Cloud Run cycles.
//!
//! Enforcement is opt-in: the middleware is a no-op unless an [`AppCheck`] is
//! present in [`crate::AppState`]. This lets us deploy the verifier dark, roll
//! out token-sending frontends, confirm tokens arrive, and only then flip
//! enforcement on — without locking anyone out mid-rollout.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::AppState;

const JWKS_URL: &str = "https://firebaseappcheck.googleapis.com/v1/jwks";
const HEADER: &str = "x-firebase-appcheck";
/// Google rotates these keys slowly; a few hours is a safe cache window, and we
/// force a refetch on an unknown `kid` regardless.
const JWKS_TTL: Duration = Duration::from_secs(6 * 3600);

#[derive(Clone)]
pub struct AppCheck(Arc<Inner>);

struct Inner {
    issuer: String,
    audience: String,
    http: reqwest::Client,
    cache: RwLock<Option<Cached>>,
}

struct Cached {
    jwks: JwkSet,
    fetched_at: Instant,
}

/// App Check tokens carry the standard registered claims; `aud`, `iss`, and
/// `exp` are validated by `jsonwebtoken`, so we only deserialize `sub` (the
/// attested app id) for logging.
#[derive(Debug, Deserialize)]
struct AppCheckClaims {
    #[allow(dead_code)]
    sub: String,
}

impl AppCheck {
    pub fn new(project_number: impl Into<String>) -> Self {
        let n = project_number.into();
        AppCheck(Arc::new(Inner {
            issuer: format!("https://firebaseappcheck.googleapis.com/{n}"),
            audience: format!("projects/{n}"),
            http: reqwest::Client::new(),
            cache: RwLock::new(None),
        }))
    }

    async fn jwks(&self) -> Result<JwkSet, String> {
        if let Some(c) = self.0.cache.read().await.as_ref() {
            if c.fetched_at.elapsed() < JWKS_TTL {
                return Ok(c.jwks.clone());
            }
        }
        self.refetch_jwks().await
    }

    async fn refetch_jwks(&self) -> Result<JwkSet, String> {
        let jwks: JwkSet = self
            .0
            .http
            .get(JWKS_URL)
            .send()
            .await
            .map_err(|e| format!("jwks fetch failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("jwks parse failed: {e}"))?;
        *self.0.cache.write().await = Some(Cached {
            jwks: jwks.clone(),
            fetched_at: Instant::now(),
        });
        Ok(jwks)
    }

    /// Returns `Ok` only for a well-formed, unexpired App Check token signed by
    /// Google with our project's audience and issuer.
    pub async fn verify(&self, token: &str) -> Result<(), String> {
        let header = decode_header(token).map_err(|e| format!("bad token header: {e}"))?;
        if header.alg != Algorithm::RS256 {
            return Err(format!("unexpected alg {:?}", header.alg));
        }
        let kid = header.kid.ok_or("token missing kid")?;

        let mut jwks = self.jwks().await?;
        let jwk = match jwks.find(&kid) {
            Some(k) => k.clone(),
            None => {
                // Key may have rotated since our last fetch — force one refetch.
                jwks = self.refetch_jwks().await?;
                jwks.find(&kid).ok_or("no signing key for kid")?.clone()
            }
        };

        let key = DecodingKey::from_jwk(&jwk).map_err(|e| format!("bad jwk: {e}"))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&self.0.audience]);
        validation.set_issuer(&[&self.0.issuer]);
        decode::<AppCheckClaims>(token, &key, &validation)
            .map_err(|e| format!("token rejected: {e}"))?;
        Ok(())
    }
}

/// Rejects requests lacking a valid App Check token when enforcement is on.
/// Health checks and CORS preflight are always allowed through.
pub async fn middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let Some(app_check) = &state.app_check else {
        return next.run(req).await; // enforcement disabled
    };

    if req.method() == Method::OPTIONS || req.uri().path() == "/health" {
        return next.run(req).await;
    }

    let token = req
        .headers()
        .get(HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    let Some(token) = token else {
        return reject("missing app check token");
    };
    if let Err(e) = app_check.verify(&token).await {
        tracing::warn!("app check rejected: {e}");
        return reject("app check verification failed");
    }

    next.run(req).await
}

fn reject(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(shared::ErrorResponse { error: msg.to_string() }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::traits::PublicKeyParts;
    use rsa::RsaPrivateKey;
    use serde_json::json;

    const PROJECT_NUMBER: &str = "780823612423";
    const KID: &str = "test-key-1";

    /// Seed the JWKS cache directly so `verify` never hits the network.
    async fn app_check_with_jwks(jwks: JwkSet) -> AppCheck {
        let ac = AppCheck::new(PROJECT_NUMBER);
        *ac.0.cache.write().await = Some(Cached {
            jwks,
            fetched_at: Instant::now(),
        });
        ac
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Build a keypair plus the matching single-key JWKS Google would publish.
    fn keypair() -> (EncodingKey, JwkSet) {
        let mut rng = rand::thread_rng();
        let private = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public = private.to_public_key();

        let pem = private.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF).unwrap();
        let enc = EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap();

        let jwk = json!({
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": KID,
            "n": b64(&public.n().to_bytes_be()),
            "e": b64(&public.e().to_bytes_be()),
        });
        let jwks: JwkSet = serde_json::from_value(json!({ "keys": [jwk] })).unwrap();
        (enc, jwks)
    }

    fn exp_in(secs: i64) -> usize {
        (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + secs) as usize
    }

    fn sign(enc: &EncodingKey, claims: serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KID.to_string());
        encode(&header, &claims, enc).unwrap()
    }

    fn valid_claims() -> serde_json::Value {
        json!({
            "sub": "1:web:abcdef",
            "aud": [format!("projects/{PROJECT_NUMBER}")],
            "iss": format!("https://firebaseappcheck.googleapis.com/{PROJECT_NUMBER}"),
            "exp": exp_in(3600),
            "iat": exp_in(-10),
        })
    }

    #[tokio::test]
    async fn accepts_a_genuine_token() {
        let (enc, jwks) = keypair();
        let ac = app_check_with_jwks(jwks).await;
        let token = sign(&enc, valid_claims());
        assert!(ac.verify(&token).await.is_ok());
    }

    #[tokio::test]
    async fn rejects_wrong_audience() {
        let (enc, jwks) = keypair();
        let ac = app_check_with_jwks(jwks).await;
        let mut claims = valid_claims();
        claims["aud"] = json!(["projects/999999"]);
        let token = sign(&enc, claims);
        assert!(ac.verify(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_wrong_issuer() {
        let (enc, jwks) = keypair();
        let ac = app_check_with_jwks(jwks).await;
        let mut claims = valid_claims();
        claims["iss"] = json!("https://evil.example.com/780823612423");
        let token = sign(&enc, claims);
        assert!(ac.verify(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        let (enc, jwks) = keypair();
        let ac = app_check_with_jwks(jwks).await;
        let mut claims = valid_claims();
        claims["exp"] = json!(exp_in(-120));
        let token = sign(&enc, claims);
        assert!(ac.verify(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_token_signed_by_a_foreign_key() {
        // Token signed by a key that isn't the one in our JWKS.
        let (_published_enc, jwks) = keypair();
        let (attacker_enc, _) = keypair();
        let ac = app_check_with_jwks(jwks).await;
        let token = sign(&attacker_enc, valid_claims());
        assert!(ac.verify(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_hs256_token() {
        // alg confusion: an HS256 token must never be accepted.
        let (_enc, jwks) = keypair();
        let ac = app_check_with_jwks(jwks).await;
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some(KID.to_string());
        let token = encode(&header, &valid_claims(), &EncodingKey::from_secret(b"x")).unwrap();
        assert!(ac.verify(&token).await.is_err());
    }
}
