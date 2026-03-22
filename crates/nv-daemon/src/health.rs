use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_secs: u64,
    pub version: String,
    pub channels: HashMap<String, ChannelStatus>,
    pub last_digest_at: Option<DateTime<Utc>>,
    pub triggers_processed: u64,
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

    /// Build a serializable health response.
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
        }
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
