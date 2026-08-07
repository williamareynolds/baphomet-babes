//! Resend transactional-email sender.
//!
//! Mirrors [`crate::fcm`]: constructed only when an API key is configured, so
//! dev and tests run with `email: None` and every send is a no-op. Unlike FCM
//! there is no metadata-server dance — Resend authenticates with a static API
//! key from Secret Manager.
//!
//! Sends are one request per recipient rather than a batch. Each member's mail
//! carries their own unsubscribe link, so the bodies differ anyway, and the
//! club is ~30 people.

use std::sync::Arc;

const RESEND_API_BASE: &str = "https://api.resend.com";

/// Outcome of a single send. Anything the caller should stop retrying is
/// [`SendOutcome::Rejected`] rather than an error — one bad address must not
/// look like an outage.
pub enum SendOutcome {
    Sent,
    /// Resend refused the message and a retry would fail the same way (invalid
    /// address, blocked domain).
    Rejected(String),
}

#[derive(Clone)]
pub struct Email(Arc<Inner>);

struct Inner {
    api_key: String,
    /// Who the mail is from, e.g. `Baphomet Babes <noreply@baphometbabes.com>`.
    from: String,
    /// Base URL of the Resend API. The real one in production; tests point this
    /// at a local mock server.
    endpoint_base: String,
    http: reqwest::Client,
}

impl Email {
    pub fn new(api_key: impl Into<String>, from: impl Into<String>) -> Self {
        Email(Arc::new(Inner {
            api_key: api_key.into(),
            from: from.into(),
            endpoint_base: RESEND_API_BASE.into(),
            http: reqwest::Client::new(),
        }))
    }

    /// Test constructor: talk to a mock server instead of the real API.
    pub fn with_endpoint(
        api_key: impl Into<String>,
        from: impl Into<String>,
        endpoint_base: impl Into<String>,
    ) -> Self {
        Email(Arc::new(Inner {
            api_key: api_key.into(),
            from: from.into(),
            endpoint_base: endpoint_base.into(),
            http: reqwest::Client::new(),
        }))
    }

    /// Send one message to one address.
    ///
    /// `unsubscribe_url` becomes a `List-Unsubscribe` header as well as the
    /// footer link the body should already carry — Gmail and friends surface
    /// the header as a native unsubscribe button, which keeps a club mailout
    /// from being reported as spam when someone wants out.
    pub async fn send(
        &self,
        to: &str,
        subject: &str,
        html: &str,
        text: &str,
        unsubscribe_url: Option<&str>,
    ) -> anyhow::Result<SendOutcome> {
        let endpoint = format!("{}/emails", self.0.endpoint_base);

        let mut payload = serde_json::json!({
            "from": self.0.from,
            "to": [to],
            "subject": subject,
            "html": html,
            "text": text,
        });
        if let Some(url) = unsubscribe_url {
            payload["headers"] = serde_json::json!({
                "List-Unsubscribe": format!("<{url}>"),
                "List-Unsubscribe-Post": "List-Unsubscribe=One-Click",
            });
        }

        let resp = self
            .0
            .http
            .post(&endpoint)
            .bearer_auth(&self.0.api_key)
            .json(&payload)
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            return Ok(SendOutcome::Sent);
        }

        let body = resp.text().await.unwrap_or_default();
        // 4xx other than rate-limiting means this message is malformed or the
        // address is unusable: report it and move on to the next recipient.
        // 429 and 5xx are transient, so those stay errors worth logging loudly.
        if status.is_client_error() && status.as_u16() != 429 {
            return Ok(SendOutcome::Rejected(format!("{status}: {body}")));
        }
        anyhow::bail!("Resend send failed ({status}): {body}");
    }
}
