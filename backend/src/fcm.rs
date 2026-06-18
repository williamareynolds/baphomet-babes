//! Firebase Cloud Messaging (HTTP v1) sender.
//!
//! Authentication uses the GCP metadata server, which hands the Cloud Run
//! runtime service account a short-lived OAuth token — no key files, no extra
//! crates. That endpoint only exists on Google infrastructure, so [`Fcm`] is
//! constructed solely in production; dev and tests run with `fcm: None` and all
//! sends become no-ops.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::RwLock;

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

/// Outcome of a single send, so the caller can prune dead device tokens.
pub enum SendOutcome {
    Sent,
    /// The token is no longer valid (app uninstalled, permission revoked); the
    /// caller should delete it.
    Stale,
}

#[derive(Clone)]
pub struct Fcm(Arc<Inner>);

struct Inner {
    project_id: String,
    http: reqwest::Client,
    token: RwLock<Option<CachedToken>>,
}

struct CachedToken {
    value: String,
    /// When this token should be considered expired (with safety margin).
    expires_at: Instant,
}

#[derive(Deserialize)]
struct MetadataToken {
    access_token: String,
    expires_in: u64,
}

impl Fcm {
    pub fn new(project_id: impl Into<String>) -> Self {
        Fcm(Arc::new(Inner {
            project_id: project_id.into(),
            http: reqwest::Client::new(),
            token: RwLock::new(None),
        }))
    }

    /// A valid OAuth access token for the messaging API, cached until shortly
    /// before it expires.
    async fn access_token(&self) -> anyhow::Result<String> {
        if let Some(c) = self.0.token.read().await.as_ref() {
            if c.expires_at > Instant::now() {
                return Ok(c.value.clone());
            }
        }

        let resp: MetadataToken = self
            .0
            .http
            .get(METADATA_TOKEN_URL)
            .header("Metadata-Flavor", "Google")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // Refresh a minute early to avoid using a token that expires mid-flight.
        let ttl = Duration::from_secs(resp.expires_in.saturating_sub(60).max(30));
        *self.0.token.write().await = Some(CachedToken {
            value: resp.access_token.clone(),
            expires_at: Instant::now() + ttl,
        });
        Ok(resp.access_token)
    }

    /// Push one notification to one device token.
    pub async fn send(
        &self,
        device_token: &str,
        title: &str,
        body: &str,
        url: Option<&str>,
    ) -> anyhow::Result<SendOutcome> {
        let token = self.access_token().await?;
        let endpoint = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            self.0.project_id
        );

        let mut data = serde_json::Map::new();
        if let Some(u) = url {
            data.insert("url".into(), serde_json::Value::String(u.to_string()));
        }

        let payload = serde_json::json!({
            "message": {
                "token": device_token,
                "notification": { "title": title, "body": body },
                "data": data,
            }
        });

        let resp = self
            .0
            .http
            .post(&endpoint)
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            return Ok(SendOutcome::Sent);
        }

        let text = resp.text().await.unwrap_or_default();
        // 404 NOT_FOUND or an UNREGISTERED error code means the device token is
        // dead — signal the caller to drop it. Everything else is a real error.
        if status.as_u16() == 404 || text.contains("UNREGISTERED") {
            tracing::info!("FCM token stale, will prune");
            return Ok(SendOutcome::Stale);
        }
        anyhow::bail!("FCM send failed ({status}): {text}");
    }
}
