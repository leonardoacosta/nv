//! NexusBackend — dispatch over TeamAgentDispatcher.
//!
//! Tool calls and callback handlers go through `NexusBackend` methods.
//! Previously this enum also had a Nexus gRPC variant; that has been removed.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::team_agent::types::ConnectionStatus;
use crate::team_agent::types::SessionSummary;
use crate::team_agent::TeamAgentDispatcher;

// ── NexusBackend ─────────────────────────────────────────────────────

/// Wrapper around TeamAgentDispatcher for uniform session management.
///
/// Callers receive `SessionSummary` / `SessionDetail` values; transport
/// details are hidden behind this type.
#[derive(Clone)]
pub struct NexusBackend {
    dispatcher: TeamAgentDispatcher,
}

impl NexusBackend {
    pub fn new(dispatcher: TeamAgentDispatcher) -> Self {
        Self { dispatcher }
    }

    /// Returns `true` if any active/running session targets `project`.
    pub async fn has_active_session_for_project(&self, project: &str) -> bool {
        self.dispatcher.has_active_agent_for_project(project).await
    }

    // ── Session Lifecycle ────────────────────────────────────────────

    /// Start a new session. Returns `(session_id, machine_name)`.
    pub async fn start_session(
        &self,
        project: &str,
        cwd: &str,
        args: &[String],
        agent: Option<&str>,
    ) -> Result<(String, String)> {
        self.dispatcher.start_agent(project, cwd, args, agent).await
    }

    /// Stop a running session by ID.
    pub async fn stop_session(&self, session_id: &str) -> Result<String> {
        self.dispatcher.stop_agent(session_id).await
    }

    // ── Tool Format Helpers ──────────────────────────────────────────

    /// Format `query_nexus` tool response.
    pub async fn format_query_sessions(&self) -> Result<String> {
        let sessions: Vec<SessionSummary> = self.dispatcher.list_agents().await;
        if sessions.is_empty() {
            return Ok("No active team-agent sessions.".to_string());
        }
        let mut output = format!("{} session(s):\n", sessions.len());
        for s in &sessions {
            let project = s.project.as_deref().unwrap_or("(no project)");
            output.push_str(&format!(
                "[{}] {}: {} -- {} ({})\n",
                s.agent_name, s.id, project, s.status, s.duration_display
            ));
        }
        Ok(output)
    }

    /// Format `query_session` tool response.
    pub async fn format_query_session(&self, session_id: &str) -> Result<String> {
        let Some(detail) = self.dispatcher.get_agent(session_id).await else {
            return Ok(format!("Session '{session_id}' not found."));
        };
        let mut output = String::new();
        output.push_str(&format!("Session: {}\n", detail.id));
        output.push_str(&format!("Machine: {}\n", detail.agent_name));
        output.push_str(&format!("Status: {}\n", detail.status));
        output.push_str(&format!("Type: {}\n", detail.session_type));
        if let Some(project) = &detail.project {
            output.push_str(&format!("Project: {project}\n"));
        }
        output.push_str(&format!("CWD: {}\n", detail.cwd));
        output.push_str(&format!("Duration: {}\n", detail.duration_display));
        if let Some(cmd) = &detail.command {
            output.push_str(&format!("Command: {cmd}\n"));
        }
        Ok(output)
    }

    /// Format `query_nexus_health` tool response.
    pub async fn format_query_health(&self) -> Result<String> {
        let details: Vec<(String, String, ConnectionStatus, Option<DateTime<Utc>>)> =
            self.dispatcher.agent_details().await;
        if details.is_empty() {
            return Ok("No team-agent machines configured.".to_string());
        }
        let mut output = String::new();
        for (name, endpoint, status, last_seen) in &details {
            output.push_str(&format!("── {name} ──\n"));
            output.push_str(&format!("  Endpoint: {endpoint}\n"));
            output.push_str(&format!("  Status: {status}\n"));
            if let Some(seen) = last_seen {
                output.push_str(&format!(
                    "  Last seen: {}\n",
                    seen.format("%H:%M:%S UTC")
                ));
            }
        }
        Ok(output.trim_end().to_string())
    }

    /// Format `query_nexus_agents` tool response.
    pub async fn format_query_agents(&self) -> Result<String> {
        let details: Vec<(String, String, ConnectionStatus, Option<DateTime<Utc>>)> =
            self.dispatcher.agent_details().await;
        if details.is_empty() {
            return Ok("No team-agent machines configured.".to_string());
        }
        let mut output = format!("{} machine(s):\n", details.len());
        for (name, endpoint, status, last_seen) in &details {
            let seen: String = last_seen
                .map(|t| t.format("%H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "never".to_string());
            output.push_str(&format!(
                "  {name}: {status} ({endpoint}) — last seen: {seen}\n"
            ));
        }
        Ok(output.trim_end().to_string())
    }

    /// Format `query_nexus_projects` tool response.
    pub async fn format_query_projects(&self) -> Result<String> {
        // Derive project list from active sessions
        let sessions: Vec<SessionSummary> = self.dispatcher.list_agents().await;
        let mut projects: std::collections::BTreeSet<String> = sessions
            .into_iter()
            .filter_map(|s| s.project)
            .collect();

        // Also include configured machine names as available "agents"
        let details: Vec<(String, String, ConnectionStatus, Option<DateTime<Utc>>)> =
            self.dispatcher.agent_details().await;
        for (name, _, _, _) in details {
            projects.insert(name);
        }

        if projects.is_empty() {
            return Ok("No projects found across team-agent machines.".to_string());
        }
        let mut output = format!("{} project(s):\n", projects.len());
        for p in &projects {
            output.push_str(&format!("  {p}\n"));
        }
        Ok(output.trim_end().to_string())
    }

    // ── Callbacks ────────────────────────────────────────────────────

    /// Execute a confirmed NexusStartSession callback action.
    ///
    /// Includes the pre-launch dedup guard.
    pub async fn execute_start_session(
        &self,
        payload: &serde_json::Value,
        project_registry: &HashMap<String, PathBuf>,
    ) -> Result<String> {
        let project = payload["project"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'project' in payload"))?;
        let command = payload["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'command' in payload"))?;

        // Pre-launch dedup guard
        if self.has_active_session_for_project(project).await {
            tracing::info!(
                project,
                dedup = true,
                "session launch skipped — already active"
            );
            return Ok(format!(
                "Session already active for {project} \u{2014} launch skipped"
            ));
        }

        let cwd = project_registry
            .get(project)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_default();
                format!("{home}/dev/{project}")
            });

        let agent = payload["agent"].as_str();
        let args: Vec<String> = command
            .split_whitespace()
            .map(String::from)
            .collect();

        let (session_id, machine) = self.start_session(project, &cwd, &args, agent).await?;

        Ok(format!(
            "Session started: {session_id} (machine: {machine})"
        ))
    }

    /// Execute a confirmed NexusStopSession callback action.
    pub async fn execute_stop_session(&self, payload: &serde_json::Value) -> Result<String> {
        let session_id = payload["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'session_id' in payload"))?;
        self.stop_session(session_id).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::config::{TeamAgentMachine, TeamAgentsConfig};

    fn make_dispatcher(machines: Vec<TeamAgentMachine>) -> TeamAgentDispatcher {
        let config = TeamAgentsConfig {
            machines,
            cc_binary: "/bin/true".to_string(),
        };
        TeamAgentDispatcher::new(&config)
    }

    fn local_machine(name: &str) -> TeamAgentMachine {
        TeamAgentMachine {
            name: name.to_string(),
            ssh_host: None,
            working_dir: Some("/tmp".to_string()),
        }
    }

    #[tokio::test]
    async fn format_query_sessions_empty() {
        let backend = NexusBackend::new(make_dispatcher(vec![local_machine("local")]));
        let output = backend.format_query_sessions().await.unwrap();
        assert!(output.contains("No active"));
    }

    #[tokio::test]
    async fn format_query_health_with_machine() {
        let backend = NexusBackend::new(make_dispatcher(vec![local_machine("local")]));
        let output = backend.format_query_health().await.unwrap();
        assert!(output.contains("local"));
    }

    #[tokio::test]
    async fn format_query_agents_with_machine() {
        let backend = NexusBackend::new(make_dispatcher(vec![local_machine("local")]));
        let output = backend.format_query_agents().await.unwrap();
        assert!(output.contains("1 machine"));
        assert!(output.contains("local"));
    }

    #[tokio::test]
    async fn format_query_session_not_found() {
        let backend = NexusBackend::new(make_dispatcher(vec![local_machine("local")]));
        let output = backend.format_query_session("nonexistent").await.unwrap();
        assert!(output.contains("not found"));
    }

    #[tokio::test]
    async fn has_active_session_false_initially() {
        let backend = NexusBackend::new(make_dispatcher(vec![local_machine("local")]));
        assert!(!backend.has_active_session_for_project("oo").await);
    }

    #[tokio::test]
    async fn execute_stop_session_missing_payload() {
        let backend = NexusBackend::new(make_dispatcher(vec![local_machine("local")]));
        let payload = serde_json::json!({});
        let result = backend.execute_stop_session(&payload).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn execute_start_session_missing_project() {
        let backend = NexusBackend::new(make_dispatcher(vec![local_machine("local")]));
        let payload = serde_json::json!({ "command": "claude" });
        let result = backend
            .execute_start_session(&payload, &HashMap::new())
            .await;
        assert!(result.is_err());
    }
}
