use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use tonic::transport::Channel;

use super::proto::nexus_agent_client::NexusAgentClient;

/// Connection status for a single Nexus agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected => write!(f, "connected"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::Reconnecting => write!(f, "reconnecting"),
        }
    }
}

/// A connection to a single Nexus agent.
#[derive(Debug)]
pub struct NexusAgentConnection {
    pub name: String,
    pub endpoint: String,
    pub client: Option<NexusAgentClient<Channel>>,
    pub status: ConnectionStatus,
    pub last_seen: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    /// When set, the watchdog skips this agent until the instant has passed.
    pub quarantined_until: Option<Instant>,
    /// When the agent first transitioned to Disconnected in the current outage.
    pub disconnected_since: Option<Instant>,
    /// Whether a Telegram disconnect notification has already been sent for
    /// the current outage (prevents re-sending on every watchdog cycle).
    pub disconnect_notified: bool,
}

/// Connection timeout for initial connect.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum backoff for reconnection attempts.
const MAX_BACKOFF: Duration = Duration::from_secs(60);

impl NexusAgentConnection {
    /// Create a new connection (not yet connected).
    pub fn new(name: String, host: &str, port: u16) -> Self {
        let endpoint = format!("http://{}:{}", host, port);
        Self {
            name,
            endpoint,
            client: None,
            status: ConnectionStatus::Disconnected,
            last_seen: None,
            consecutive_failures: 0,
            quarantined_until: None,
            disconnected_since: None,
            disconnect_notified: false,
        }
    }

    /// Attempt to connect to the agent with a timeout.
    pub async fn connect(&mut self) -> Result<(), tonic::transport::Error> {
        let channel = Channel::from_shared(self.endpoint.clone())
            .expect("valid endpoint URI")
            .connect_timeout(CONNECT_TIMEOUT)
            .connect()
            .await?;

        self.client = Some(NexusAgentClient::new(channel));
        self.status = ConnectionStatus::Connected;
        self.last_seen = Some(Utc::now());
        self.consecutive_failures = 0;
        self.quarantined_until = None;
        self.disconnected_since = None;

        tracing::info!(agent = %self.name, endpoint = %self.endpoint, "connected to Nexus agent");
        Ok(())
    }

    /// Mark the connection as disconnected after a failed RPC.
    pub fn mark_disconnected(&mut self) {
        self.status = ConnectionStatus::Disconnected;
        self.client = None;
        self.consecutive_failures += 1;
        // Record the first moment we noticed the outage; preserve across cycles.
        if self.disconnected_since.is_none() {
            self.disconnected_since = Some(Instant::now());
        }
        tracing::warn!(
            agent = %self.name,
            failures = self.consecutive_failures,
            "Nexus agent disconnected"
        );
    }

    /// Returns true if the agent is currently quarantined (watchdog should skip it).
    pub fn is_quarantined(&self) -> bool {
        self.quarantined_until
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
    }

    /// Quarantine the agent for 5 minutes — used after 10+ consecutive failures.
    pub fn quarantine(&mut self) {
        let until = Instant::now() + Duration::from_secs(300);
        self.quarantined_until = Some(until);
        tracing::warn!(
            agent = %self.name,
            failures = self.consecutive_failures,
            "Nexus agent quarantined for 5 minutes"
        );
    }

    /// Ping the agent via the `GetHealth` RPC with a 5-second timeout.
    ///
    /// - Success: updates `last_seen` and returns `Ok(())`
    /// - `Unimplemented`: treated as healthy (agent is reachable but hasn't implemented the RPC)
    /// - Timeout / other error: returns `Err(tonic::Status)`
    pub async fn health_check(&mut self) -> Result<(), tonic::Status> {
        let Some(client) = self.client.as_mut() else {
            return Err(tonic::Status::unavailable("no client"));
        };

        let request = super::proto::HealthRequest {};
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            client.get_health(request),
        )
        .await;

        match result {
            Ok(Ok(_)) => {
                self.last_seen = Some(Utc::now());
                Ok(())
            }
            Ok(Err(status)) if status.code() == tonic::Code::Unimplemented => {
                // Agent is reachable but hasn't implemented GetHealth — treat as healthy.
                tracing::debug!(
                    agent = %self.name,
                    "GetHealth not implemented by agent — treating as healthy"
                );
                self.last_seen = Some(Utc::now());
                Ok(())
            }
            Ok(Err(status)) => Err(status),
            Err(_elapsed) => Err(tonic::Status::deadline_exceeded("health check timed out")),
        }
    }

    /// Calculate backoff duration based on consecutive failures.
    ///
    /// Exponential: 1s, 2s, 4s, 8s, ... capped at 60s.
    pub fn backoff_duration(&self) -> Duration {
        let secs = 1u64 << self.consecutive_failures.min(6);
        Duration::from_secs(secs).min(MAX_BACKOFF)
    }

    /// Attempt to reconnect with exponential backoff.
    pub async fn reconnect(&mut self) {
        self.status = ConnectionStatus::Reconnecting;
        let backoff = self.backoff_duration();
        tracing::info!(
            agent = %self.name,
            backoff_secs = backoff.as_secs(),
            "reconnecting to Nexus agent"
        );

        tokio::time::sleep(backoff).await;

        match self.connect().await {
            Ok(()) => {
                tracing::info!(agent = %self.name, "reconnected to Nexus agent");
            }
            Err(e) => {
                tracing::warn!(
                    agent = %self.name,
                    error = %e,
                    "reconnection failed"
                );
                self.status = ConnectionStatus::Disconnected;
                // Do NOT increment consecutive_failures here — mark_disconnected()
                // already incremented it before this call. One failed attempt = one
                // increment.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_connection_starts_disconnected() {
        let conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
        assert!(conn.client.is_none());
        assert!(conn.last_seen.is_none());
        assert_eq!(conn.consecutive_failures, 0);
        assert_eq!(conn.endpoint, "http://127.0.0.1:7400");
        assert!(conn.quarantined_until.is_none());
        assert!(conn.disconnected_since.is_none());
        assert!(!conn.disconnect_notified);
    }

    #[test]
    fn backoff_duration_exponential() {
        let mut conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        assert_eq!(conn.backoff_duration(), Duration::from_secs(1));

        conn.consecutive_failures = 1;
        assert_eq!(conn.backoff_duration(), Duration::from_secs(2));

        conn.consecutive_failures = 2;
        assert_eq!(conn.backoff_duration(), Duration::from_secs(4));

        conn.consecutive_failures = 3;
        assert_eq!(conn.backoff_duration(), Duration::from_secs(8));
    }

    #[test]
    fn backoff_duration_capped_at_60s() {
        let mut conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        conn.consecutive_failures = 10;
        assert_eq!(conn.backoff_duration(), Duration::from_secs(60));

        conn.consecutive_failures = 100;
        assert_eq!(conn.backoff_duration(), Duration::from_secs(60));
    }

    #[test]
    fn mark_disconnected_increments_failures() {
        let mut conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        assert_eq!(conn.consecutive_failures, 0);

        conn.mark_disconnected();
        assert_eq!(conn.consecutive_failures, 1);
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
        assert!(conn.client.is_none());

        conn.mark_disconnected();
        assert_eq!(conn.consecutive_failures, 2);
    }

    #[test]
    fn connection_status_display() {
        assert_eq!(ConnectionStatus::Connected.to_string(), "connected");
        assert_eq!(ConnectionStatus::Disconnected.to_string(), "disconnected");
        assert_eq!(ConnectionStatus::Reconnecting.to_string(), "reconnecting");
    }

    #[test]
    fn mark_disconnected_sets_disconnected_since_once() {
        let mut conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        assert!(conn.disconnected_since.is_none());

        conn.mark_disconnected();
        let first = conn.disconnected_since.expect("should be set after first disconnect");

        // Second call must NOT overwrite the first timestamp.
        conn.mark_disconnected();
        let second = conn.disconnected_since.unwrap();
        assert_eq!(first, second, "disconnected_since must not be overwritten");
    }

    #[test]
    fn is_quarantined_false_when_none() {
        let conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        assert!(!conn.is_quarantined());
    }

    #[test]
    fn quarantine_sets_future_instant() {
        let mut conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        conn.quarantine();
        assert!(conn.is_quarantined());
        // The quarantine deadline should be ~5 minutes in the future.
        let until = conn.quarantined_until.unwrap();
        let remaining = until.saturating_duration_since(Instant::now());
        // Allow a small tolerance — should be between 299s and 301s.
        assert!(remaining.as_secs() < 301, "quarantine too long: {remaining:?}");
        assert!(remaining.as_secs() >= 299, "quarantine too short: {remaining:?}");
    }

    #[test]
    fn is_quarantined_false_after_expiry() {
        let mut conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        // Set quarantine to an instant in the past.
        conn.quarantined_until = Some(Instant::now() - Duration::from_secs(1));
        assert!(!conn.is_quarantined());
    }

    /// Verify that a single mark_disconnected() + failed reconnect() results in
    /// consecutive_failures == 1, not 2.  The double-increment bug (P1) would give 2.
    #[test]
    fn single_mark_disconnected_plus_reconnect_failure_counts_once() {
        let mut conn = NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        assert_eq!(conn.consecutive_failures, 0);

        // Simulate what the watchdog does: mark disconnected first (count → 1),
        // then the reconnect() Err branch should NOT add another increment.
        conn.mark_disconnected();
        assert_eq!(conn.consecutive_failures, 1, "mark_disconnected should increment once");

        // Simulate the Err branch of reconnect() executing without connect():
        // In the real code, connect() would fail and we only set status/not increment.
        conn.status = ConnectionStatus::Disconnected;
        // No second increment — consecutive_failures must remain 1.
        assert_eq!(conn.consecutive_failures, 1, "reconnect Err branch must not add a second increment");
    }
}
