//! Cloud Storage uploads for gathering cover images.
//!
//! Authenticates off the GCP metadata server, exactly like [`crate::fcm`] — the
//! Cloud Run runtime service account gets a short-lived OAuth token, so there
//! are no key files to manage. That endpoint only exists on Google
//! infrastructure, so [`Media`] is constructed solely in production; dev and
//! tests run with `media: None` and uploads return a clear "not configured"
//! error rather than failing obscurely.
//!
//! Objects are written with UUID names and read publicly. Public-read is a
//! deliberate tradeoff: cover images are decoration, serving them through the
//! API would put image bytes on every page load through Cloud Run, and the
//! object names are unguessable. Nothing sensitive belongs in this bucket.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::RwLock;

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

/// Image types we accept. Anything else is rejected before a byte is stored —
/// an open bucket that takes arbitrary content types is a hosting service for
/// whoever finds it.
const ALLOWED: &[(&str, &str)] = &[
    ("image/jpeg", "jpg"),
    ("image/png", "png"),
    ("image/webp", "webp"),
    ("image/gif", "gif"),
];

/// Cap on a single upload. Cover images are decoration; anything larger is a
/// mistake or an attack on the storage bill.
pub const MAX_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone)]
pub struct Media(Arc<Inner>);

struct Inner {
    bucket: String,
    /// Base URL of the upload API. The real one in production; tests point this
    /// at a local mock.
    endpoint_base: String,
    /// Fixed bearer token for tests, where no metadata server exists.
    static_token: Option<String>,
    http: reqwest::Client,
    token: RwLock<Option<CachedToken>>,
}

struct CachedToken {
    value: String,
    expires_at: Instant,
}

#[derive(Deserialize)]
struct MetadataToken {
    access_token: String,
    expires_in: u64,
}

/// Whether `content_type` is an image we store, and the extension to give it.
pub fn extension_for(content_type: &str) -> Option<&'static str> {
    ALLOWED
        .iter()
        .find(|(ct, _)| *ct == content_type)
        .map(|(_, ext)| *ext)
}

impl Media {
    pub fn new(bucket: impl Into<String>) -> Self {
        Media(Arc::new(Inner {
            bucket: bucket.into(),
            endpoint_base: "https://storage.googleapis.com".into(),
            static_token: None,
            http: reqwest::Client::new(),
            token: RwLock::new(None),
        }))
    }

    /// Test constructor: upload to a mock server with a fixed bearer token.
    pub fn with_endpoint(
        bucket: impl Into<String>,
        endpoint_base: impl Into<String>,
        static_token: impl Into<String>,
    ) -> Self {
        Media(Arc::new(Inner {
            bucket: bucket.into(),
            endpoint_base: endpoint_base.into(),
            static_token: Some(static_token.into()),
            http: reqwest::Client::new(),
            token: RwLock::new(None),
        }))
    }

    async fn access_token(&self) -> anyhow::Result<String> {
        if let Some(t) = &self.0.static_token {
            return Ok(t.clone());
        }
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

        let ttl = Duration::from_secs(resp.expires_in.saturating_sub(60).max(30));
        *self.0.token.write().await = Some(CachedToken {
            value: resp.access_token.clone(),
            expires_at: Instant::now() + ttl,
        });
        Ok(resp.access_token)
    }

    /// Store `bytes` and return the public URL.
    ///
    /// `name` is caller-chosen and must already be unguessable — this does not
    /// sanitize it beyond what the caller passes.
    pub async fn upload(
        &self,
        name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> anyhow::Result<String> {
        let token = self.access_token().await?;
        let endpoint = format!(
            "{}/upload/storage/v1/b/{}/o?uploadType=media&name={}",
            self.0.endpoint_base, self.0.bucket, name
        );

        let resp = self
            .0
            .http
            .post(&endpoint)
            .bearer_auth(token)
            .header("Content-Type", content_type)
            .body(bytes)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("upload failed ({status}): {body}");
        }

        Ok(format!(
            "https://storage.googleapis.com/{}/{}",
            self.0.bucket, name
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_image_types_we_serve() {
        assert_eq!(extension_for("image/jpeg"), Some("jpg"));
        assert_eq!(extension_for("image/png"), Some("png"));
        assert_eq!(extension_for("image/webp"), Some("webp"));
        assert_eq!(extension_for("image/gif"), Some("gif"));
    }

    #[test]
    fn rejects_everything_else() {
        // A bucket that accepts these is a free file host for whoever finds it.
        assert_eq!(extension_for("text/html"), None);
        assert_eq!(extension_for("application/pdf"), None);
        assert_eq!(extension_for("image/svg+xml"), None); // SVG can carry script
        assert_eq!(extension_for(""), None);
        // Parameters aren't stripped for us; an exact match is required.
        assert_eq!(extension_for("image/png; charset=binary"), None);
    }
}
