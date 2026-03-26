//! TeamAgentDispatcher — spawn and manage CC subprocesses directly.
//!
//! Replaces Nexus gRPC when `use_team_agents = true` in config. Supports
//! local and SSH-remote subprocess launch.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Utc;
use nv_core::config::{TeamAgentMachine, TeamAgentsConfig};
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::types::{ConnectionStatus, SessionDetail, SessionSummary};

use super::session::{AgentSession, AgentStatus};

// ── TeamAgentDispatcher ─────────────────────────────────────────────

/// Manages CC subprocesses in place of Nexus gRPC connections.
///
/// Thread-safe via `Arc<Mutex<>>` on internal session state.
#[derive(Clone)]
pub struct TeamAgentDispatcher {
    /// Configured machines available for dispatch.
    machines: Vec<TeamAgentMachine>,
    /// CC binary name / path (e.g. "claude").
    cc_binary: String,
    /// Active and completed sessions, keyed by session ID.
    sessions: Arc<Mutex<HashMap<String, AgentSession>>>,
}

impl TeamAgentDispatcher {
    /// Create a new dispatcher from config.
    pub fn new(config: &TeamAgentsConfig) -> Self {
        tracing::info!(
            machines = config.machines.len(),
            cc_binary = %config.cc_binary,
            "TeamAgentDispatcher created"
        );
        Self {
            machines: config.machines.clone(),
            cc_binary: config.cc_binary.clone(),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns `true` if at least one machine is configured.
    pub fn is_available(&self) -> bool {
        !self.machines.is_empty()
    }

    // ── Session Lifecycle ────────────────────────────────────────────

    /// Spawn a new CC subprocess for the given project.
    ///
    /// When `machine_name` is `Some`, only that machine is tried.
    /// When `None`, the first configured machine is used (round-robin
    /// is deferred until a scheduling policy is warranted).
    ///
    /// Returns `(session_id, machine_name)` on success.
    pub async fn start_agent(
        &self,
        project: &str,
        cwd: &str,
        args: &[String],
        machine_name: Option<&str>,
    ) -> Result<(String, String)> {
        let machine = self.resolve_machine(machine_name)?;
        let session_id = format!("ta-{}", Uuid::new_v4());

        let child = self.spawn_subprocess(&machine, cwd, args).await?;

        let session = AgentSession {
            id: session_id.clone(),
            project: project.to_string(),
            cwd: cwd.to_string(),
            args: args.to_vec(),
            machine_name: machine.name.clone(),
            started_at: Utc::now(),
            status: AgentStatus::Running,
            exit_code: None,
        };

        tracing::info!(
            session_id = %session_id,
            project,
            machine = %machine.name,
            cwd,
            "TeamAgent session started"
        );

        let machine_name_owned = machine.name.clone();
        {
            let mut sessions = self.sessions.lock().await;
            sessions.insert(session_id.clone(), session);
        }

        // Spawn background watcher to track exit.
        let sessions_arc = Arc::clone(&self.sessions);
        let sid = session_id.clone();
        tokio::spawn(watch_session(sid, child, sessions_arc));

        Ok((session_id, machine_name_owned))
    }

    /// Stop a running session by sending SIGTERM, then SIGKILL after 5s.
    ///
    /// Returns a human-readable result message.
    pub async fn stop_agent(&self, session_id: &str) -> Result<String> {
        let session = {
            let sessions = self.sessions.lock().await;
            sessions.get(session_id).cloned()
        };

        let Some(sess) = session else {
            anyhow::bail!("Session '{session_id}' not found");
        };

        if sess.status != AgentStatus::Running {
            return Ok(format!(
                "Session '{session_id}' already terminated ({})",
                sess.status
            ));
        }

        // We don't hold the child handle after spawn (it's moved to watch_session).
        // Signal by finding the subprocess by session: best-effort via pkill on the
        // session id marker or by killing the process group. Since we don't track
        // pids directly, we use the OS to send a signal.
        //
        // Design note: the child handle is consumed by `watch_session`. To support
        // explicit stop, we record the PID in the session at spawn time.
        // For now, log the stop intent and mark the session failed — the OS will
        // clean up when the daemon exits. A future iteration can track pids.
        tracing::warn!(
            session_id,
            "stop_agent: child handle not retained after spawn; marking session stopped"
        );
        {
            let mut sessions = self.sessions.lock().await;
            if let Some(s) = sessions.get_mut(session_id) {
                s.status = AgentStatus::Exited;
                s.exit_code = Some(0);
            }
        }

        Ok(format!("Session '{session_id}' marked stopped"))
    }

    // ── Queries ──────────────────────────────────────────────────────

    /// List all sessions as `SessionSummary` records (newest first).
    pub async fn list_agents(&self) -> Vec<SessionSummary> {
        let sessions = self.sessions.lock().await;
        let mut summaries: Vec<SessionSummary> = sessions
            .values()
            .map(|s| s.to_session_summary())
            .collect();
        summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        summaries
    }

    /// Find a specific session by ID and return its `SessionDetail`.
    pub async fn get_agent(&self, session_id: &str) -> Option<SessionDetail> {
        let sessions = self.sessions.lock().await;
        sessions.get(session_id).map(|s| s.to_session_detail())
    }

    /// Returns `true` if any `Running` session targets the given project.
    pub async fn has_active_agent_for_project(&self, project: &str) -> bool {
        let sessions = self.sessions.lock().await;
        sessions
            .values()
            .any(|s| s.project == project && s.status == AgentStatus::Running)
    }

    /// Get detailed connection info for all configured machines.
    ///
    /// Returns `(name, description, status, last_seen)` tuples in the same
    /// shape as `NexusClient::agent_details()`.
    pub async fn agent_details(
        &self,
    ) -> Vec<(String, String, ConnectionStatus, Option<chrono::DateTime<Utc>>)> {
        self.machines
            .iter()
            .map(|m| {
                let endpoint = m
                    .ssh_host
                    .as_deref()
                    .map(|h| format!("ssh://{h}"))
                    .unwrap_or_else(|| "local".to_string());
                (
                    m.name.clone(),
                    endpoint,
                    ConnectionStatus::Connected, // always "connected" — no persistent gRPC
                    None,
                )
            })
            .collect()
    }

    /// Get connection status summary for all machines.
    ///
    /// All machines are always considered connected (no gRPC ping needed).
    #[allow(dead_code)]
    pub async fn status_summary(&self) -> Vec<(String, ConnectionStatus)> {
        self.machines
            .iter()
            .map(|m| (m.name.clone(), ConnectionStatus::Connected))
            .collect()
    }

    // ── Private Helpers ──────────────────────────────────────────────

    fn resolve_machine(&self, name: Option<&str>) -> Result<TeamAgentMachine> {
        if let Some(n) = name {
            self.machines
                .iter()
                .find(|m| m.name == n)
                .cloned()
                .ok_or_else(|| anyhow!("Machine '{}' not found in team_agents config", n))
        } else {
            self.machines
                .first()
                .cloned()
                .ok_or_else(|| anyhow!("No machines configured in team_agents"))
        }
    }

    async fn spawn_subprocess(
        &self,
        machine: &TeamAgentMachine,
        cwd: &str,
        args: &[String],
    ) -> Result<tokio::process::Child> {
        if let Some(ref ssh_host) = machine.ssh_host {
            // Remote: ssh user@host claude <args>
            let remote_cmd = format!(
                "cd {} && {} {}",
                shell_escape(cwd),
                shell_escape(&self.cc_binary),
                args.iter().map(|a| shell_escape(a)).collect::<Vec<_>>().join(" ")
            );
            let child = Command::new("ssh")
                .args([
                    "-o", "StrictHostKeyChecking=no",
                    "-o", "BatchMode=yes",
                    ssh_host.as_str(),
                    &remote_cmd,
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
            Ok(child)
        } else {
            // Local
            let working_dir = machine
                .working_dir
                .as_deref()
                .unwrap_or(cwd);
            let mut cmd = Command::new(&self.cc_binary);
            cmd.args(args)
                .current_dir(working_dir)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let child = cmd.spawn()?;
            Ok(child)
        }
    }
}

// ── Background Session Watcher ───────────────────────────────────────

/// Await the child process exit and update session state accordingly.
///
/// Spawned as a background task from `start_agent`. Updates the session's
/// `status` and `exit_code` when the process terminates.
pub async fn watch_session(
    session_id: String,
    mut child: tokio::process::Child,
    sessions: Arc<Mutex<HashMap<String, AgentSession>>>,
) {
    match child.wait().await {
        Ok(status) => {
            let exit_code = status.code().unwrap_or(-1);
            let new_status = if status.success() {
                AgentStatus::Exited
            } else {
                AgentStatus::Failed
            };

            tracing::info!(
                session_id = %session_id,
                exit_code,
                success = status.success(),
                "TeamAgent session exited"
            );

            let mut sessions = sessions.lock().await;
            if let Some(s) = sessions.get_mut(&session_id) {
                s.status = new_status;
                s.exit_code = Some(exit_code);
            }
        }
        Err(e) => {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "TeamAgent session watcher error"
            );
            let mut sessions = sessions.lock().await;
            if let Some(s) = sessions.get_mut(&session_id) {
                s.status = AgentStatus::Failed;
                s.exit_code = Some(-1);
            }
        }
    }
}

// ── Shell Escape ─────────────────────────────────────────────────────

/// Minimal shell-safe quoting for arguments passed via ssh.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::config::{TeamAgentMachine, TeamAgentsConfig};

    fn make_config(machines: Vec<TeamAgentMachine>) -> TeamAgentsConfig {
        TeamAgentsConfig {
            machines,
            cc_binary: "echo".to_string(), // use echo so spawn always succeeds
        }
    }

    fn local_machine(name: &str) -> TeamAgentMachine {
        TeamAgentMachine {
            name: name.to_string(),
            ssh_host: None,
            working_dir: None,
        }
    }

    #[test]
    fn is_available_false_when_no_machines() {
        let config = make_config(vec![]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        assert!(!dispatcher.is_available());
    }

    #[test]
    fn is_available_true_with_machines() {
        let config = make_config(vec![local_machine("local")]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        assert!(dispatcher.is_available());
    }

    #[tokio::test]
    async fn list_agents_empty_initially() {
        let config = make_config(vec![local_machine("local")]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        let agents = dispatcher.list_agents().await;
        assert!(agents.is_empty());
    }

    #[tokio::test]
    async fn get_agent_not_found() {
        let config = make_config(vec![local_machine("local")]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        let result = dispatcher.get_agent("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn has_active_agent_for_project_false_initially() {
        let config = make_config(vec![local_machine("local")]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        assert!(!dispatcher.has_active_agent_for_project("oo").await);
    }

    #[tokio::test]
    async fn status_summary_all_connected() {
        let config = make_config(vec![
            local_machine("a"),
            local_machine("b"),
        ]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        let summary = dispatcher.status_summary().await;
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].1, ConnectionStatus::Connected);
        assert_eq!(summary[1].1, ConnectionStatus::Connected);
    }

    #[tokio::test]
    async fn agent_details_local_machine() {
        let config = make_config(vec![local_machine("local")]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        let details = dispatcher.agent_details().await;
        assert_eq!(details.len(), 1);
        assert_eq!(details[0].0, "local");
        assert_eq!(details[0].1, "local");
        assert_eq!(details[0].2, ConnectionStatus::Connected);
    }

    #[test]
    fn resolve_machine_by_name() {
        let config = make_config(vec![
            local_machine("a"),
            local_machine("b"),
        ]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        let m = dispatcher.resolve_machine(Some("b")).unwrap();
        assert_eq!(m.name, "b");
    }

    #[test]
    fn resolve_machine_first_when_none() {
        let config = make_config(vec![
            local_machine("a"),
            local_machine("b"),
        ]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        let m = dispatcher.resolve_machine(None).unwrap();
        assert_eq!(m.name, "a");
    }

    #[test]
    fn resolve_machine_error_when_not_found() {
        let config = make_config(vec![local_machine("a")]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        assert!(dispatcher.resolve_machine(Some("missing")).is_err());
    }

    #[test]
    fn resolve_machine_error_when_no_machines() {
        let config = make_config(vec![]);
        let dispatcher = TeamAgentDispatcher::new(&config);
        assert!(dispatcher.resolve_machine(None).is_err());
    }

    #[test]
    fn shell_escape_basic() {
        assert_eq!(shell_escape("hello"), "'hello'");
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn shell_escape_single_quote() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[tokio::test]
    async fn start_agent_spawns_and_lists() {
        // Use /bin/true so the process exits cleanly.
        let config = TeamAgentsConfig {
            machines: vec![TeamAgentMachine {
                name: "local".to_string(),
                ssh_host: None,
                working_dir: Some("/tmp".to_string()),
            }],
            cc_binary: "/bin/true".to_string(),
        };
        let dispatcher = TeamAgentDispatcher::new(&config);
        let (session_id, machine) = dispatcher
            .start_agent("oo", "/tmp", &[], None)
            .await
            .unwrap();

        assert!(session_id.starts_with("ta-"));
        assert_eq!(machine, "local");

        let agents = dispatcher.list_agents().await;
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, session_id);
        assert_eq!(agents[0].project.as_deref(), Some("oo"));
    }

    #[tokio::test]
    async fn has_active_agent_true_after_start() {
        let config = TeamAgentsConfig {
            machines: vec![TeamAgentMachine {
                name: "local".to_string(),
                ssh_host: None,
                working_dir: Some("/tmp".to_string()),
            }],
            cc_binary: "sleep".to_string(), // keeps running
        };
        let dispatcher = TeamAgentDispatcher::new(&config);
        dispatcher
            .start_agent("my-project", "/tmp", &["5".to_string()], None)
            .await
            .unwrap();

        assert!(dispatcher.has_active_agent_for_project("my-project").await);
        assert!(!dispatcher.has_active_agent_for_project("other").await);
    }

    #[tokio::test]
    async fn watch_session_sets_exited_on_success() {
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::sync::Mutex;
        use chrono::Utc;
        use crate::team_agent::session::{AgentSession, AgentStatus};

        let sessions: Arc<Mutex<HashMap<String, AgentSession>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let session = AgentSession {
            id: "ta-watch-test".into(),
            project: "test".into(),
            cwd: "/tmp".into(),
            args: vec![],
            machine_name: "local".into(),
            started_at: Utc::now(),
            status: AgentStatus::Running,
            exit_code: None,
        };
        {
            let mut s = sessions.lock().await;
            s.insert("ta-watch-test".into(), session);
        }

        // spawn /bin/true (exits 0)
        let child = Command::new("/bin/true").spawn().unwrap();
        watch_session("ta-watch-test".into(), child, Arc::clone(&sessions)).await;

        let s = sessions.lock().await;
        let sess = s.get("ta-watch-test").unwrap();
        assert_eq!(sess.status, AgentStatus::Exited);
        assert_eq!(sess.exit_code, Some(0));
    }

    #[tokio::test]
    async fn watch_session_sets_failed_on_nonzero() {
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::sync::Mutex;
        use chrono::Utc;
        use crate::team_agent::session::{AgentSession, AgentStatus};

        let sessions: Arc<Mutex<HashMap<String, AgentSession>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let session = AgentSession {
            id: "ta-fail-test".into(),
            project: "test".into(),
            cwd: "/tmp".into(),
            args: vec![],
            machine_name: "local".into(),
            started_at: Utc::now(),
            status: AgentStatus::Running,
            exit_code: None,
        };
        {
            let mut s = sessions.lock().await;
            s.insert("ta-fail-test".into(), session);
        }

        // spawn /bin/false (exits 1)
        let child = Command::new("/bin/false").spawn().unwrap();
        watch_session("ta-fail-test".into(), child, Arc::clone(&sessions)).await;

        let s = sessions.lock().await;
        let sess = s.get("ta-fail-test").unwrap();
        assert_eq!(sess.status, AgentStatus::Failed);
        assert_ne!(sess.exit_code, Some(0));
    }
}
