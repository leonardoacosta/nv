//! Session state types for TeamAgentDispatcher.
//!
//! `AgentSession` tracks a running CC subprocess. Conversion methods produce
//! the same `SessionSummary` and `SessionDetail` types used by the Nexus
//! gRPC client so that callers can treat both backends uniformly.

use chrono::{DateTime, Utc};

use crate::nexus::client::{SessionDetail, SessionSummary};

// ── AgentStatus ─────────────────────────────────────────────────────

/// Lifecycle status of a managed CC subprocess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    /// Process is running and has not yet exited.
    Running,
    /// Process exited with code 0.
    Exited,
    /// Process exited with a non-zero code.
    Failed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Running => write!(f, "active"),
            AgentStatus::Exited => write!(f, "exited"),
            AgentStatus::Failed => write!(f, "errored"),
        }
    }
}

// ── AgentSession ────────────────────────────────────────────────────

/// A running or completed CC subprocess managed by `TeamAgentDispatcher`.
#[derive(Debug, Clone)]
pub struct AgentSession {
    /// Unique session ID (UUID string).
    pub id: String,
    /// Project code this session was launched for (e.g. "oo").
    pub project: String,
    /// Working directory the process was started in.
    pub cwd: String,
    /// Original args passed to the CC binary.
    pub args: Vec<String>,
    /// Logical machine name (matches `TeamAgentMachine::name`).
    pub machine_name: String,
    /// When the process was started.
    pub started_at: DateTime<Utc>,
    /// Current lifecycle status.
    pub status: AgentStatus,
    /// Process exit code when `status` is `Exited` or `Failed`.
    pub exit_code: Option<i32>,
}

impl AgentSession {
    /// Convert to a `SessionSummary` compatible with the Nexus gRPC type.
    pub fn to_session_summary(&self) -> SessionSummary {
        let duration_display = compute_duration_display(self.started_at);
        SessionSummary {
            id: self.id.clone(),
            project: Some(self.project.clone()),
            status: self.status.to_string(),
            agent_name: self.machine_name.clone(),
            started_at: Some(self.started_at),
            duration_display,
            branch: None,
            spec: None,
        }
    }

    /// Convert to a `SessionDetail` compatible with the Nexus gRPC type.
    pub fn to_session_detail(&self) -> SessionDetail {
        let duration_display = compute_duration_display(self.started_at);
        SessionDetail {
            id: self.id.clone(),
            project: Some(self.project.clone()),
            status: self.status.to_string(),
            agent_name: self.machine_name.clone(),
            started_at: Some(self.started_at),
            duration_display,
            branch: None,
            spec: None,
            cwd: self.cwd.clone(),
            command: Some(self.args.join(" ")),
            session_type: "managed".to_string(),
            model: None,
            cost_usd: None,
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn compute_duration_display(started_at: DateTime<Utc>) -> String {
    let elapsed = Utc::now() - started_at;
    let total_secs = elapsed.num_seconds();
    if total_secs < 60 {
        format!("{total_secs}s")
    } else if total_secs < 3600 {
        format!("{}m", total_secs / 60)
    } else {
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        format!("{hours}h{mins}m")
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(status: AgentStatus) -> AgentSession {
        AgentSession {
            id: "ta-test-001".into(),
            project: "oo".into(),
            cwd: "/home/user/dev/oo".into(),
            args: vec!["hello world".into()],
            machine_name: "local".into(),
            started_at: Utc::now() - chrono::Duration::minutes(5),
            status,
            exit_code: None,
        }
    }

    #[test]
    fn to_session_summary_running() {
        let sess = make_session(AgentStatus::Running);
        let summary = sess.to_session_summary();
        assert_eq!(summary.id, "ta-test-001");
        assert_eq!(summary.project.as_deref(), Some("oo"));
        assert_eq!(summary.status, "active");
        assert_eq!(summary.agent_name, "local");
        assert!(summary.started_at.is_some());
        // duration should reflect ~5 minutes
        assert!(summary.duration_display.contains('m') || summary.duration_display.contains('s'));
    }

    #[test]
    fn to_session_summary_exited() {
        let sess = make_session(AgentStatus::Exited);
        let summary = sess.to_session_summary();
        assert_eq!(summary.status, "exited");
    }

    #[test]
    fn to_session_summary_failed() {
        let sess = make_session(AgentStatus::Failed);
        let summary = sess.to_session_summary();
        assert_eq!(summary.status, "errored");
    }

    #[test]
    fn to_session_detail_fields() {
        let sess = make_session(AgentStatus::Running);
        let detail = sess.to_session_detail();
        assert_eq!(detail.id, "ta-test-001");
        assert_eq!(detail.project.as_deref(), Some("oo"));
        assert_eq!(detail.cwd, "/home/user/dev/oo");
        assert_eq!(detail.command.as_deref(), Some("hello world"));
        assert_eq!(detail.session_type, "managed");
        assert!(detail.model.is_none());
        assert!(detail.cost_usd.is_none());
    }

    #[test]
    fn agent_status_display() {
        assert_eq!(AgentStatus::Running.to_string(), "active");
        assert_eq!(AgentStatus::Exited.to_string(), "exited");
        assert_eq!(AgentStatus::Failed.to_string(), "errored");
    }

    #[test]
    fn duration_display_seconds() {
        let started = Utc::now() - chrono::Duration::seconds(30);
        let d = compute_duration_display(started);
        assert!(d.ends_with('s'));
    }

    #[test]
    fn duration_display_minutes() {
        let started = Utc::now() - chrono::Duration::minutes(10);
        let d = compute_duration_display(started);
        assert!(d.ends_with('m'));
    }

    #[test]
    fn duration_display_hours() {
        let started = Utc::now() - chrono::Duration::hours(2) - chrono::Duration::minutes(15);
        let d = compute_duration_display(started);
        assert!(d.contains('h') && d.contains('m'));
    }
}
