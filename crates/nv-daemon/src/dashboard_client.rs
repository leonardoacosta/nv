//! Thin HTTP client for forwarding daemon messages to the Nova dashboard.
//!
//! When the dashboard is configured and reachable, the worker routes
//! `Query` and `Command` triggers here instead of to the local cold-start path.
//! If forwarding fails (network error, 5xx, timeout) the caller falls back to
//! the cold-start worker transparently.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Legacy request / response types (kept for /api/session/message endpoint) ─

#[derive(Debug, Serialize)]
struct LegacyForwardRequest {
    chat_id: Option<i64>,
    text: String,
    context: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyForwardResponse {
    reply: String,
    #[allow(dead_code)]
    session_state: Option<String>,
    #[allow(dead_code)]
    processing_ms: Option<u64>,
}

// ── Nova worker endpoint types (/api/nova/message) ────────────────────────────

/// Request body for the `/api/nova/message` endpoint.
///
/// Carries the user message, optional channel context, and the pre-built
/// system context string so the dashboard can reproduce the same environment
/// the local cold-start would use.
#[derive(Debug, Serialize)]
pub struct ForwardRequest {
    /// Raw user message text (trigger content).
    pub message: String,
    /// Telegram chat ID (None for non-Telegram triggers).
    pub chat_id: Option<i64>,
    /// Telegram message ID of the original user message.
    pub message_id: Option<i64>,
    /// Originating channel name (e.g. "telegram", "discord").
    pub channel: String,
    /// Pre-built system context string from `build_system_context()`.
    pub system_context: String,
}

/// Successful response from `/api/nova/message`.
#[derive(Debug, Deserialize)]
pub struct ForwardResponse {
    /// Claude's reply text to send back to the user.
    pub reply: String,
    /// Opaque session identifier from the dashboard CC session.
    #[allow(dead_code)]
    pub session_id: String,
}

// ── Error classification ──────────────────────────────────────────────────────

/// Errors returned by `DashboardClient::forward()`.
///
/// Error variants are classified so the worker can decide whether to fall
/// back to the cold-start path or surface an error to the user:
///
/// - `Unavailable` — transient: fall back silently to cold-start.
/// - `AuthError` / `BadRequest` — logic errors: log + alert, no fallback.
#[derive(Debug, Error)]
pub enum DashboardError {
    /// Transient availability error: 5xx response, connection refused, or
    /// request timeout.  The caller SHOULD fall back to the cold-start path.
    #[error("dashboard unavailable: {0}")]
    Unavailable(String),

    /// Authentication or authorization failure (HTTP 401 or 403).
    /// The caller MUST NOT fall back — this is a misconfiguration bug.
    #[error("dashboard auth error: {0}")]
    AuthError(String),

    /// Malformed request or other 4xx error (excluding 401/403).
    /// The caller MUST NOT fall back — this indicates a client-side bug.
    #[error("dashboard bad request: {0}")]
    BadRequest(String),
}

// ── DashboardClient ──────────────────────────────────────────────────────────

/// HTTP client that forwards messages to the Nova dashboard CC session.
///
/// Holds a `reqwest::Client` configured with a 120s timeout (matching the
/// dashboard's own abort controller). Thread-safe: clone freely.
#[derive(Clone)]
pub struct DashboardClient {
    client: reqwest::Client,
    /// Base URL for the dashboard (e.g. "https://nova.example.com").
    base_url: String,
    /// Bearer token sent with every request.
    secret: String,
    /// Set to `false` after a failed reachability check; restored on success.
    pub healthy: Arc<AtomicBool>,
}

impl DashboardClient {
    /// Create a new client.
    ///
    /// `base_url` — dashboard origin (no trailing slash).
    /// `secret`   — value sent as `Authorization: Bearer <secret>`.
    pub fn new(base_url: impl Into<String>, secret: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("failed to build reqwest client for DashboardClient")?;

        Ok(Self {
            client,
            base_url: base_url.into(),
            secret: secret.into(),
            healthy: Arc::new(AtomicBool::new(false)), // starts unhealthy until ping
        })
    }

    /// Forward a message to the Nova dashboard worker endpoint.
    ///
    /// Posts to `{base_url}/api/nova/message` with `Authorization: Bearer` header.
    ///
    /// Returns:
    /// - `Ok(ForwardResponse)` on HTTP 2xx with a parseable body.
    /// - `Err(DashboardError::Unavailable)` on 5xx, timeouts, or connection errors
    ///   — the caller should fall back to the cold-start path.
    /// - `Err(DashboardError::AuthError)` on 401/403 — caller should alert + abort.
    /// - `Err(DashboardError::BadRequest)` on other 4xx — caller should alert + abort.
    pub async fn forward(&self, req: ForwardRequest) -> Result<ForwardResponse, DashboardError> {
        let url = format!("{}/api/nova/message", self.base_url);

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.secret)
            .json(&req)
            .send()
            .await
            .map_err(|e| {
                self.healthy.store(false, Ordering::Relaxed);
                DashboardError::Unavailable(format!("connection error: {e}"))
            })?;

        let status = resp.status();

        if status.is_success() {
            let body: ForwardResponse = resp.json().await.map_err(|e| {
                // Parse error after 2xx: treat as unavailable so we can fall back.
                self.healthy.store(false, Ordering::Relaxed);
                DashboardError::Unavailable(format!("failed to parse response JSON: {e}"))
            })?;
            self.healthy.store(true, Ordering::Relaxed);
            return Ok(body);
        }

        self.healthy.store(false, Ordering::Relaxed);

        match status.as_u16() {
            401 | 403 => Err(DashboardError::AuthError(format!("HTTP {status}"))),
            500..=599 => Err(DashboardError::Unavailable(format!("HTTP {status}"))),
            _ => Err(DashboardError::BadRequest(format!("HTTP {status}"))),
        }
    }

    /// Forward a message to the dashboard and return the CC reply text.
    ///
    /// Posts to `{base_url}/api/session/message`.  On any non-2xx response
    /// or transport error, returns `Err` so the caller can fall back to the
    /// local worker pool.
    pub async fn forward_message(
        &self,
        text: impl Into<String>,
        chat_id: Option<i64>,
        context: Option<String>,
    ) -> Result<String> {
        let url = format!("{}/api/session/message", self.base_url);

        let payload = LegacyForwardRequest {
            chat_id,
            text: text.into(),
            context,
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.secret)
            .json(&payload)
            .send()
            .await
            .context("dashboard POST failed")?;

        let status = resp.status();

        if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            // 503 means session not ready — mark unhealthy and propagate so
            // the caller falls back to the local worker pool.
            self.healthy.store(false, Ordering::Relaxed);
            return Err(anyhow!("dashboard returned 503 — session not ready"));
        }

        if !status.is_success() {
            self.healthy.store(false, Ordering::Relaxed);
            return Err(anyhow!("dashboard returned HTTP {status}"));
        }

        let body: LegacyForwardResponse = resp
            .json()
            .await
            .context("failed to parse dashboard response JSON")?;

        // Restore healthy flag on success.
        self.healthy.store(true, Ordering::Relaxed);

        Ok(body.reply)
    }

    /// Ping `/api/session/status` to check reachability.
    ///
    /// Updates `self.healthy` and returns whether the dashboard is up.
    /// Never fails — errors are logged and treated as unhealthy.
    pub async fn ping(&self) -> bool {
        let url = format!("{}/api/session/status", self.base_url);

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.healthy.store(true, Ordering::Relaxed);
                true
            }
            Ok(resp) => {
                tracing::warn!(
                    status = %resp.status(),
                    url = %url,
                    "dashboard ping returned non-2xx"
                );
                self.healthy.store(false, Ordering::Relaxed);
                false
            }
            Err(e) => {
                tracing::warn!(error = %e, url = %url, "dashboard ping failed");
                self.healthy.store(false, Ordering::Relaxed);
                false
            }
        }
    }

    /// Returns `true` if the last ping or forward call succeeded.
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    /// Returns the base URL (for logging).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_request() -> ForwardRequest {
        ForwardRequest {
            message: "what's the weather?".to_string(),
            chat_id: Some(12345),
            message_id: Some(99),
            channel: "telegram".to_string(),
            system_context: "You are Nova.".to_string(),
        }
    }

    #[tokio::test]
    async fn forward_200_ok() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/nova/message"))
            .and(header("Authorization", "Bearer test-secret"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({
                        "reply": "Sunny and 72°F.",
                        "session_id": "sess-abc"
                    })),
            )
            .mount(&server)
            .await;

        let client = DashboardClient::new(&server.uri(), "test-secret").unwrap();
        let result = client.forward(make_request()).await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let resp = result.unwrap();
        assert_eq!(resp.reply, "Sunny and 72°F.");
        assert_eq!(resp.session_id, "sess-abc");
        assert!(client.is_healthy());
    }

    #[tokio::test]
    async fn forward_503_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/nova/message"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let client = DashboardClient::new(&server.uri(), "test-secret").unwrap();
        let result = client.forward(make_request()).await;
        assert!(matches!(result, Err(DashboardError::Unavailable(_))));
        assert!(!client.is_healthy());
    }

    #[tokio::test]
    async fn forward_401_auth_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/nova/message"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = DashboardClient::new(&server.uri(), "wrong-secret").unwrap();
        let result = client.forward(make_request()).await;
        assert!(matches!(result, Err(DashboardError::AuthError(_))));
    }

    #[tokio::test]
    async fn forward_request_serialization() {
        let req = make_request();
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("what's the weather?"));
        assert!(json.contains("telegram"));
        assert!(json.contains("You are Nova."));
    }
}
