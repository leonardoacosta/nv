//! Thin HTTP client for forwarding daemon messages to the Nova dashboard.
//!
//! When the dashboard is configured and reachable, the orchestrator routes
//! `Query` and `Command` triggers here instead of to the local worker pool.
//! If forwarding fails (network error, 5xx, timeout) the caller falls back to
//! the worker pool transparently.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

// ── Request / Response types ─────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ForwardRequest {
    chat_id: Option<i64>,
    text: String,
    context: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ForwardResponse {
    reply: String,
    #[allow(dead_code)]
    session_state: Option<String>,
    #[allow(dead_code)]
    processing_ms: Option<u64>,
}

// ── DashboardClient ──────────────────────────────────────────────────

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

        let payload = ForwardRequest {
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

        let body: ForwardResponse = resp
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
}
