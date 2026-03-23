use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::tools::CheckResult;

// ── Channel Status ──────────────────────────────────────────────────

/// Connection status of a channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelStatus {
    Connected,
    Disconnected,
}

// ── Health Response ─────────────────────────────────────────────────

/// JSON response for GET /health.
///
/// The `tools` field is populated only when the request includes `?deep=true`.
/// Each entry maps a service name (e.g. `"stripe"`, `"jira/personal"`) to its
/// `CheckResult`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_secs: u64,
    pub version: String,
    pub channels: HashMap<String, ChannelStatus>,
    pub last_digest_at: Option<DateTime<Utc>>,
    pub triggers_processed: u64,
    /// Per-service connectivity check results. Only present when `?deep=true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<HashMap<String, CheckResult>>,
}

// ── Health State ────────────────────────────────────────────────────

/// Shared health state updated by daemon components.
///
/// Passed as `Arc<HealthState>` to the HTTP server, agent loop,
/// channel listeners, and digest sender.
pub struct HealthState {
    started_at: Instant,
    triggers_processed: AtomicU64,
    channel_status: RwLock<HashMap<String, ChannelStatus>>,
    last_digest_at: RwLock<Option<DateTime<Utc>>>,
}

#[allow(dead_code)]
impl HealthState {
    /// Create a new health state, recording the current instant as start time.
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            triggers_processed: AtomicU64::new(0),
            channel_status: RwLock::new(HashMap::new()),
            last_digest_at: RwLock::new(None),
        }
    }

    /// Increment the trigger counter (called by the agent loop on each batch).
    pub fn record_trigger(&self) {
        self.triggers_processed.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the connection status of a channel.
    pub async fn update_channel(&self, name: impl Into<String>, status: ChannelStatus) {
        let mut map = self.channel_status.write().await;
        map.insert(name.into(), status);
    }

    /// Record the timestamp of the most recent digest.
    pub async fn update_last_digest(&self, timestamp: DateTime<Utc>) {
        let mut guard = self.last_digest_at.write().await;
        *guard = Some(timestamp);
    }

    /// Build a serializable health response (shallow — no tool probes).
    pub async fn to_health_response(&self) -> HealthResponse {
        let channels = self.channel_status.read().await.clone();
        let last_digest_at = *self.last_digest_at.read().await;

        HealthResponse {
            status: "ok".into(),
            uptime_secs: self.started_at.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").into(),
            channels,
            last_digest_at,
            triggers_processed: self.triggers_processed.load(Ordering::Relaxed),
            tools: None,
        }
    }

    /// Build a deep health response that includes tool connectivity probes.
    ///
    /// Constructs each service client from environment variables, runs
    /// `check_read()` concurrently against all configured services, and
    /// attaches the results as `tools: HashMap<service_name, CheckResult>`.
    ///
    /// This is intentionally async and may take up to ~5 seconds to complete
    /// (read probes have their own internal timeouts).
    pub async fn to_deep_health_response(&self) -> HealthResponse {
        use crate::tools::{
            Checkable,
            check::{MissingService, check_all},
        };

        let mut owned: Vec<Box<dyn Checkable>> = Vec::new();

        macro_rules! push_env {
            ($ctor:expr, $missing_name:expr, $missing_var:expr) => {
                match $ctor {
                    Ok(c) => owned.push(Box::new(c)),
                    Err(_) => owned.push(Box::new(MissingService::new($missing_name, $missing_var))),
                }
            };
        }

        use crate::tools::{ado, cloudflare, doppler, ha, neon, posthog, resend, sentry, stripe, upstash, vercel};
        use crate::tools::{docker, github, plaid};

        push_env!(stripe::StripeClient::from_env(), "stripe", "STRIPE_SECRET_KEY");
        push_env!(vercel::VercelClient::from_env(), "vercel", "VERCEL_API_TOKEN");
        push_env!(sentry::SentryClient::from_env(), "sentry", "SENTRY_AUTH_TOKEN");
        push_env!(resend::ResendClient::from_env(), "resend", "RESEND_API_KEY");
        push_env!(ha::HAClient::from_env(), "ha", "HA_TOKEN");
        push_env!(upstash::UpstashClient::from_env(), "upstash", "UPSTASH_REDIS_REST_URL");
        push_env!(ado::AdoClient::from_env(), "ado", "ADO_PAT");
        push_env!(cloudflare::CloudflareClient::from_env(), "cloudflare", "CLOUDFLARE_API_TOKEN");
        push_env!(doppler::DopplerClient::from_env(), "doppler", "DOPPLER_TOKEN");
        // Neon: use "default" project as the probe target; check_read checks POSTGRES_URL_DEFAULT
        owned.push(Box::new(neon::NeonClient::new("default")));

        owned.push(Box::new(posthog::PosthogClient));
        owned.push(Box::new(github::GithubClient));
        owned.push(Box::new(docker::DockerClient));
        owned.push(Box::new(plaid::PlaidClient));

        let refs: Vec<&dyn Checkable> = owned.iter().map(|s| s.as_ref()).collect();
        // Read-only for the health endpoint — write probes are too expensive for a heartbeat
        let report = check_all(&refs, false).await;

        let tool_map: HashMap<String, CheckResult> = report
            .read_results
            .into_iter()
            .map(|e| (e.name, e.result))
            .collect();

        let mut resp = self.to_health_response().await;
        resp.tools = Some(tool_map);
        resp
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_health_state_has_zero_triggers() {
        let state = HealthState::new();
        let resp = state.to_health_response().await;
        assert_eq!(resp.triggers_processed, 0);
        assert_eq!(resp.status, "ok");
        assert!(resp.channels.is_empty());
        assert!(resp.last_digest_at.is_none());
    }

    #[tokio::test]
    async fn record_trigger_increments() {
        let state = HealthState::new();
        state.record_trigger();
        state.record_trigger();
        state.record_trigger();
        let resp = state.to_health_response().await;
        assert_eq!(resp.triggers_processed, 3);
    }

    #[tokio::test]
    async fn update_channel_status() {
        let state = HealthState::new();
        state
            .update_channel("telegram", ChannelStatus::Connected)
            .await;
        state
            .update_channel("nexus_homelab", ChannelStatus::Disconnected)
            .await;

        let resp = state.to_health_response().await;
        assert_eq!(resp.channels.len(), 2);
        assert_eq!(
            resp.channels.get("telegram"),
            Some(&ChannelStatus::Connected)
        );
        assert_eq!(
            resp.channels.get("nexus_homelab"),
            Some(&ChannelStatus::Disconnected)
        );
    }

    #[tokio::test]
    async fn update_last_digest() {
        let state = HealthState::new();
        assert!(state.to_health_response().await.last_digest_at.is_none());

        let now = Utc::now();
        state.update_last_digest(now).await;
        let resp = state.to_health_response().await;
        assert_eq!(resp.last_digest_at, Some(now));
    }

    #[tokio::test]
    async fn version_is_set() {
        let state = HealthState::new();
        let resp = state.to_health_response().await;
        assert!(!resp.version.is_empty());
    }

    #[tokio::test]
    async fn uptime_is_non_negative() {
        let state = HealthState::new();
        let resp = state.to_health_response().await;
        // Uptime should be 0 or a small number (test runs fast)
        assert!(resp.uptime_secs < 5);
    }
}
