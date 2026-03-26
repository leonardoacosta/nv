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
    #[allow(dead_code)]
    pub spec: Option<String>,
}

/// Detailed session info returned by query_session.
#[derive(Debug, Clone)]
pub struct SessionDetail {
    pub id: String,
    pub project: Option<String>,
    pub status: String,
    pub agent_name: String,
    #[allow(dead_code)]
    pub started_at: Option<DateTime<Utc>>,
    pub duration_display: String,
    #[allow(dead_code)]
    pub branch: Option<String>,
    #[allow(dead_code)]
    pub spec: Option<String>,
    pub cwd: String,
    pub command: Option<String>,
    pub session_type: String,
    #[allow(dead_code)]
    pub model: Option<String>,
    #[allow(dead_code)]
    pub cost_usd: Option<f32>,
}

/// Connection status for a machine / agent endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    #[allow(dead_code)]
    Disconnected,
    #[allow(dead_code)]
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
