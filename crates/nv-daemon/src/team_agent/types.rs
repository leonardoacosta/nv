//! Shared session types used by TeamAgentDispatcher and the nexus backend layer.

use chrono::{DateTime, Utc};

/// Summary of a session for display and digest integration.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub project: Option<String>,
    pub status: String,
    pub agent_name: String,
    pub started_at: Option<DateTime<Utc>>,
    pub duration_display: String,
    #[allow(dead_code)] // reserved for Next.js dashboard API exposure
    pub branch: Option<String>,
    pub spec: Option<String>,
}

/// Detailed session info returned by query_session.
#[derive(Debug, Clone)]
pub struct SessionDetail {
    pub id: String,
    pub project: Option<String>,
    pub status: String,
    pub agent_name: String,
    pub started_at: Option<DateTime<Utc>>,
    pub duration_display: String,
    pub branch: Option<String>,
    pub spec: Option<String>,
    pub cwd: String,
    pub command: Option<String>,
    pub session_type: String,
    pub model: Option<String>,
    pub cost_usd: Option<f32>,
}

/// Connection status for a machine / agent endpoint.
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
