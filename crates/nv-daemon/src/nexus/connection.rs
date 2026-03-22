use std::time::Duration;

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

        tracing::info!(agent = %self.name, endpoint = %self.endpoint, "connected to Nexus agent");
        Ok(())
    }

    /// Mark the connection as disconnected after a failed RPC.
    pub fn mark_disconnected(&mut self) {
        self.status = ConnectionStatus::Disconnected;
        self.client = None;
        self.consecutive_failures += 1;
        tracing::warn!(
            agent = %self.name,
            failures = self.consecutive_failures,
            "Nexus agent disconnected"
        );
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
                self.consecutive_failures += 1;
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
}
