//! CcSessionManager — manages Claude Code subprocess sessions.
//!
//! This is the canonical replacement for the nexus callback pattern.
//! `CcSessionManager` wraps `TeamAgentDispatcher` with:
//!
//! - Typed `CcSessionState` tracking lifecycle with error state
//! - `CcSessionHandle` for subprocess-level metadata
//! - Health-monitor task with 3-attempt auto-restart cap
//! - Query helpers used by the `/sessions`, `/start`, `/stop` bot commands

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use tokio::sync::Mutex;

use crate::team_agent::TeamAgentDispatcher;

// ── Constants ─────────────────────────────────────────────────────────────

/// Seconds between health-monitor polls.
const HEALTH_POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum consecutive restart attempts before a session is left in Error state.
const MAX_RESTART_ATTEMPTS: u32 = 3;

// ── CcSessionState ────────────────────────────────────────────────────────

/// Lifecycle state for a managed CC subprocess session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CcSessionState {
    /// Process is running.
    Running,
    /// Process exited cleanly (exit code 0).
    Completed,
    /// Process exited with a non-zero code; restart may be attempted.
    Failed {
        exit_code: i32,
        restart_attempts: u32,
    },
    /// Max restart attempts exhausted; session will not be relaunched.
    Error { reason: String },
    /// Session was explicitly stopped by the user.
    Stopped,
}

impl std::fmt::Display for CcSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CcSessionState::Running => write!(f, "running"),
            CcSessionState::Completed => write!(f, "completed"),
            CcSessionState::Failed {
                exit_code,
                restart_attempts,
            } => write!(f, "failed(exit={exit_code},retries={restart_attempts})"),
            CcSessionState::Error { reason } => write!(f, "error({reason})"),
            CcSessionState::Stopped => write!(f, "stopped"),
        }
    }
}

// ── CcSessionHandle ───────────────────────────────────────────────────────

/// Metadata for a session managed by `CcSessionManager`.
#[derive(Debug, Clone)]
pub struct CcSessionHandle {
    /// Stable session ID (from TeamAgentDispatcher, e.g. "ta-<uuid>").
    pub id: String,
    /// Project code (e.g. "oo").
    pub project: String,
    /// Working directory the CC process runs in.
    pub cwd: String,
    /// Arguments passed to the CC binary.
    pub args: Vec<String>,
    /// Machine name that ran the session.
    pub machine_name: String,
    /// When the session was started.
    pub started_at: DateTime<Utc>,
    /// Current lifecycle state.
    pub state: CcSessionState,
    /// Number of consecutive restart attempts made by the health monitor.
    pub restart_attempts: u32,
}

// ── CcSessionSummary ──────────────────────────────────────────────────────

/// Lightweight summary for list/status views.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CcSessionSummary {
    pub id: String,
    pub project: String,
    pub state: String,
    pub machine_name: String,
    pub started_at: String,
    pub duration_display: String,
    pub restart_attempts: u32,
}

impl From<&CcSessionHandle> for CcSessionSummary {
    fn from(h: &CcSessionHandle) -> Self {
        CcSessionSummary {
            id: h.id.clone(),
            project: h.project.clone(),
            state: h.state.to_string(),
            machine_name: h.machine_name.clone(),
            started_at: h.started_at.to_rfc3339(),
            duration_display: compute_duration(h.started_at),
            restart_attempts: h.restart_attempts,
        }
    }
}

// ── CcSessionManager ──────────────────────────────────────────────────────

/// Manages CC subprocess sessions on top of `TeamAgentDispatcher`.
///
/// Thread-safe via `Arc<Mutex<>>` on the sessions map. Callers should clone
/// the manager cheaply — the inner `Arc` is shared.
#[derive(Clone)]
pub struct CcSessionManager {
    dispatcher: TeamAgentDispatcher,
    sessions: Arc<Mutex<HashMap<String, CcSessionHandle>>>,
    /// Optional Postgres pool for writing session lifecycle to the `sessions` table.
    pg_pool: Option<crate::pg_pool::PgPool>,
}

impl CcSessionManager {
    /// Create a manager wrapping the given dispatcher.
    pub fn new(dispatcher: TeamAgentDispatcher) -> Self {
        Self {
            dispatcher,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            pg_pool: None,
        }
    }

    /// Attach a PgPool for writing session lifecycle to Postgres.
    pub fn with_pg_pool(mut self, pool: crate::pg_pool::PgPool) -> Self {
        self.pg_pool = Some(pool);
        self
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────

    /// Start a new CC session.
    ///
    /// Returns the `session_id` on success.
    pub async fn start(
        &self,
        project: &str,
        cwd: &str,
        args: &[String],
        machine_name: Option<&str>,
    ) -> Result<String> {
        let (session_id, machine) = self
            .dispatcher
            .start_agent(project, cwd, args, machine_name)
            .await?;

        let handle = CcSessionHandle {
            id: session_id.clone(),
            project: project.to_string(),
            cwd: cwd.to_string(),
            args: args.to_vec(),
            machine_name: machine,
            started_at: Utc::now(),
            state: CcSessionState::Running,
            restart_attempts: 0,
        };

        {
            let mut sessions = self.sessions.lock().await;
            sessions.insert(session_id.clone(), handle);
        }

        // Write session start to Postgres (fire-and-forget).
        if let Some(ref pg_pool) = self.pg_pool {
            // Extract UUID portion from "ta-<uuid>" format for the PG uuid column.
            let pg_id = session_id
                .strip_prefix("ta-")
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                .unwrap_or_else(uuid::Uuid::new_v4);

            let project_owned = project.to_string();
            let command = args.first().cloned().unwrap_or_default();
            let pool = pg_pool.clone();
            tokio::spawn(async move {
                if let Some(guard) = pool.client().await {
                    let client = guard.get();
                    if let Err(e) = client
                        .execute(
                            "INSERT INTO sessions (id, project, command, status, trigger_type, started_at)
                             VALUES ($1, $2, $3, 'running', 'agent', NOW())",
                            &[&pg_id, &project_owned, &command],
                        )
                        .await
                    {
                        tracing::warn!(error = %e, session_id = %pg_id, "pg: session start insert failed");
                    }
                }
            });
        }

        tracing::info!(session_id = %session_id, project, "CcSession started");
        Ok(session_id)
    }

    /// Stop a session by ID (explicit user stop).
    ///
    /// Marks it `Stopped` regardless of subprocess result.
    pub async fn stop(&self, session_id: &str) -> Result<String> {
        let result = self.dispatcher.stop_agent(session_id).await?;

        {
            let mut sessions = self.sessions.lock().await;
            if let Some(h) = sessions.get_mut(session_id) {
                h.state = CcSessionState::Stopped;
            }
        }

        // Write session stop to Postgres (fire-and-forget).
        self.pg_update_session_status(session_id, "stopped").await;

        tracing::info!(session_id, "CcSession stopped by user");
        Ok(result)
    }

    // ── Postgres Helpers ───────────────────────────────────────────────────

    /// Update session status and stopped_at in Postgres (fire-and-forget).
    async fn pg_update_session_status(&self, session_id: &str, status: &str) {
        let Some(ref pg_pool) = self.pg_pool else {
            return;
        };
        let pg_id = session_id
            .strip_prefix("ta-")
            .and_then(|s| uuid::Uuid::parse_str(s).ok());
        let Some(pg_id) = pg_id else {
            return;
        };

        let pool = pg_pool.clone();
        let status_owned = status.to_string();
        tokio::spawn(async move {
            if let Some(guard) = pool.client().await {
                let client = guard.get();
                if let Err(e) = client
                    .execute(
                        "UPDATE sessions SET status = $1, stopped_at = NOW() WHERE id = $2",
                        &[&status_owned, &pg_id],
                    )
                    .await
                {
                    tracing::warn!(error = %e, session_id = %pg_id, status = %status_owned, "pg: session status update failed");
                }
            }
        });
    }

    // ── Query ─────────────────────────────────────────────────────────────

    /// List all tracked sessions as summaries, newest first.
    pub async fn list(&self) -> Vec<CcSessionSummary> {
        let sessions = self.sessions.lock().await;
        let mut summaries: Vec<CcSessionSummary> =
            sessions.values().map(CcSessionSummary::from).collect();
        summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        summaries
    }

    /// Get status summary for a single session.
    pub async fn get_status(&self, session_id: &str) -> Option<CcSessionSummary> {
        let sessions = self.sessions.lock().await;
        sessions.get(session_id).map(CcSessionSummary::from)
    }

    /// Find a running session for the given project (for dedup guard).
    pub async fn find_by_project(&self, project: &str) -> Option<String> {
        let sessions = self.sessions.lock().await;
        sessions
            .values()
            .find(|h| h.project == project && h.state == CcSessionState::Running)
            .map(|h| h.id.clone())
    }

    /// Returns `true` when the dispatcher has at least one machine configured.
    pub fn is_available(&self) -> bool {
        self.dispatcher.is_available()
    }

    // ── Health Monitor ────────────────────────────────────────────────────

    /// Spawn the background health-monitor task.
    ///
    /// Polls running sessions every `HEALTH_POLL_INTERVAL` and attempts to
    /// restart failed ones up to `MAX_RESTART_ATTEMPTS` times. After that,
    /// transitions the session to `CcSessionState::Error`.
    pub fn spawn_health_monitor(&self) {
        let manager = self.clone();
        tokio::spawn(async move {
            tracing::info!("CcSessionManager health monitor started");
            loop {
                tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
                manager.poll_sessions().await;
            }
        });
    }

    /// Single health-monitor poll: sync dispatcher state into CcSessionHandle.
    async fn poll_sessions(&self) {
        // Collect dispatcher state (avoids holding both locks simultaneously).
        let dispatcher_sessions = self.dispatcher.list_agents().await;
        let dispatcher_map: HashMap<String, String> = dispatcher_sessions
            .into_iter()
            .map(|s| (s.id, s.status))
            .collect();

        let mut to_restart: Vec<(String, String, Vec<String>, Option<String>)> = Vec::new();
        // Session IDs that transitioned to a terminal state (for PG updates).
        let mut pg_status_updates: Vec<(String, String)> = Vec::new();

        {
            let mut sessions = self.sessions.lock().await;
            for handle in sessions.values_mut() {
                // Only monitor sessions that were Running from our perspective.
                if handle.state != CcSessionState::Running {
                    continue;
                }

                let dispatcher_status = dispatcher_map.get(&handle.id).map(|s| s.as_str());

                match dispatcher_status {
                    Some("active") | None => {
                        // Still running or not yet reflected — no action.
                    }
                    Some("exited") => {
                        tracing::info!(
                            session_id = %handle.id,
                            project = %handle.project,
                            "CcSession completed (exited 0)"
                        );
                        handle.state = CcSessionState::Completed;
                        pg_status_updates
                            .push((handle.id.clone(), "completed".to_string()));
                    }
                    Some("errored") | Some(_) => {
                        // Non-zero exit — attempt restart if budget remains.
                        if handle.restart_attempts < MAX_RESTART_ATTEMPTS {
                            tracing::warn!(
                                session_id = %handle.id,
                                project = %handle.project,
                                attempt = handle.restart_attempts + 1,
                                "CcSession failed — scheduling restart"
                            );
                            handle.restart_attempts += 1;
                            handle.state = CcSessionState::Failed {
                                exit_code: -1,
                                restart_attempts: handle.restart_attempts,
                            };
                            pg_status_updates
                                .push((handle.id.clone(), "failed".to_string()));
                            to_restart.push((
                                handle.project.clone(),
                                handle.cwd.clone(),
                                handle.args.clone(),
                                Some(handle.machine_name.clone()),
                            ));
                        } else {
                            let reason = format!(
                                "max restarts ({MAX_RESTART_ATTEMPTS}) exhausted"
                            );
                            tracing::error!(
                                session_id = %handle.id,
                                project = %handle.project,
                                reason = %reason,
                                "CcSession error — giving up"
                            );
                            handle.state = CcSessionState::Error {
                                reason: reason.clone(),
                            };
                            pg_status_updates
                                .push((handle.id.clone(), "error".to_string()));
                        }
                    }
                }
            }
        }

        // Update Postgres session status for transitions detected this cycle.
        for (sid, status) in pg_status_updates {
            self.pg_update_session_status(&sid, &status).await;
        }

        // Restart outside the lock.
        for (project, cwd, args, machine) in to_restart {
            if let Err(e) = self
                .start(&project, &cwd, &args, machine.as_deref())
                .await
            {
                tracing::error!(
                    project = %project,
                    error = %e,
                    "CcSession restart failed"
                );
            }
        }
    }

    // ── Callback Helpers ─────────────────────────────────────────────────

    /// Execute a confirmed `CcStartSession` callback action.
    ///
    /// Parses `project`, `command`, and optional `agent` from the JSON payload.
    /// Includes a pre-launch dedup guard.
    pub async fn execute_start(
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
        if let Some(existing) = self.find_by_project(project).await {
            tracing::info!(
                project,
                existing_session = %existing,
                "CcSession launch skipped — already active"
            );
            return Ok(format!(
                "Session already active for {project} (id: {existing}) — launch skipped"
            ));
        }

        let cwd = project_registry
            .get(project)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_default();
                format!("{home}/dev/{project}")
            });

        let machine = payload["agent"].as_str();
        let args: Vec<String> = command.split_whitespace().map(String::from).collect();

        let session_id = self.start(project, &cwd, &args, machine).await?;
        Ok(format!("CC session started: {session_id}"))
    }

    /// Execute a confirmed `CcStopSession` callback action.
    pub async fn execute_stop(&self, payload: &serde_json::Value) -> Result<String> {
        let session_id = payload["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'session_id' in payload"))?;
        self.stop(session_id).await
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn compute_duration(started_at: DateTime<Utc>) -> String {
    let elapsed = Utc::now() - started_at;
    let total_secs = elapsed.num_seconds();
    if total_secs < 60 {
        format!("{total_secs}s")
    } else if total_secs < 3600 {
        format!("{}m", total_secs / 60)
    } else {
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        format!("{h}h{m}m")
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::config::{TeamAgentMachine, TeamAgentsConfig};

    fn make_manager(cc_binary: &str) -> CcSessionManager {
        let config = TeamAgentsConfig {
            machines: vec![TeamAgentMachine {
                name: "local".to_string(),
                ssh_host: None,
                working_dir: Some("/tmp".to_string()),
            }],
            cc_binary: cc_binary.to_string(),
        };
        CcSessionManager::new(TeamAgentDispatcher::new(&config))
    }

    #[test]
    fn is_available_true_with_machines() {
        let m = make_manager("echo");
        assert!(m.is_available());
    }

    #[tokio::test]
    async fn list_empty_initially() {
        let m = make_manager("echo");
        assert!(m.list().await.is_empty());
    }

    #[tokio::test]
    async fn get_status_not_found() {
        let m = make_manager("echo");
        assert!(m.get_status("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn find_by_project_none_initially() {
        let m = make_manager("echo");
        assert!(m.find_by_project("oo").await.is_none());
    }

    #[tokio::test]
    async fn start_and_list() {
        let m = make_manager("echo");
        let id = m.start("oo", "/tmp", &[], None).await.unwrap();
        assert!(id.starts_with("ta-"));

        let list = m.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].project, "oo");
        assert_eq!(list[0].state, "running");
    }

    #[tokio::test]
    async fn find_by_project_after_start() {
        let m = make_manager("sleep");
        let id = m
            .start("my-project", "/tmp", &["5".to_string()], None)
            .await
            .unwrap();
        let found = m.find_by_project("my-project").await;
        assert_eq!(found, Some(id));
    }

    #[tokio::test]
    async fn find_by_project_none_for_wrong_project() {
        let m = make_manager("sleep");
        m.start("oo", "/tmp", &["5".to_string()], None).await.unwrap();
        assert!(m.find_by_project("other").await.is_none());
    }

    #[tokio::test]
    async fn stop_transitions_to_stopped() {
        let m = make_manager("sleep");
        let id = m
            .start("oo", "/tmp", &["5".to_string()], None)
            .await
            .unwrap();
        m.stop(&id).await.unwrap();
        let status = m.get_status(&id).await.unwrap();
        assert_eq!(status.state, "stopped");
    }

    #[tokio::test]
    async fn execute_start_missing_project() {
        let m = make_manager("echo");
        let payload = serde_json::json!({ "command": "/apply fix" });
        let result = m.execute_start(&payload, &HashMap::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn execute_stop_missing_session_id() {
        let m = make_manager("echo");
        let payload = serde_json::json!({});
        let result = m.execute_stop(&payload).await;
        assert!(result.is_err());
    }

    #[test]
    fn cc_session_state_display() {
        assert_eq!(CcSessionState::Running.to_string(), "running");
        assert_eq!(CcSessionState::Completed.to_string(), "completed");
        assert_eq!(CcSessionState::Stopped.to_string(), "stopped");
        assert_eq!(
            CcSessionState::Failed {
                exit_code: 1,
                restart_attempts: 2,
            }
            .to_string(),
            "failed(exit=1,retries=2)"
        );
        assert_eq!(
            CcSessionState::Error {
                reason: "max retries".to_string(),
            }
            .to_string(),
            "error(max retries)"
        );
    }
}
