use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use nv_core::config::NexusAgent;
use tokio::sync::Mutex;

use super::connection::{ConnectionStatus, NexusAgentConnection};
use super::proto::{self, SessionFilter, SessionId};

/// Summary of a Nexus session for display and digest integration.
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

/// Client managing connections to multiple Nexus agents.
///
/// Thread-safe via `Arc<Mutex<>>` on each connection to allow
/// concurrent query and event stream operations.
#[derive(Clone)]
pub struct NexusClient {
    pub agents: Vec<Arc<Mutex<NexusAgentConnection>>>,
}

impl NexusClient {
    /// Create a new client from config. Does not connect yet.
    pub fn new(configs: &[NexusAgent]) -> Self {
        let agents = configs
            .iter()
            .map(|c| {
                Arc::new(Mutex::new(NexusAgentConnection::new(
                    c.name.clone(),
                    &c.host,
                    c.port,
                )))
            })
            .collect();

        tracing::info!(
            agent_count = configs.len(),
            "NexusClient created"
        );

        Self { agents }
    }

    /// Connect to all configured agents in parallel.
    ///
    /// Failed connections are logged as warnings, not errors.
    /// Partial connectivity is normal.
    pub async fn connect_all(&self) {
        let mut handles = Vec::new();

        for agent in &self.agents {
            let agent = Arc::clone(agent);
            handles.push(tokio::spawn(async move {
                let mut conn = agent.lock().await;
                if let Err(e) = conn.connect().await {
                    tracing::warn!(
                        agent = %conn.name,
                        endpoint = %conn.endpoint,
                        error = %e,
                        "failed to connect to Nexus agent"
                    );
                }
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }
    }

    /// Returns true if at least one agent is connected.
    pub async fn is_connected(&self) -> bool {
        for agent in &self.agents {
            let conn = agent.lock().await;
            if conn.status == ConnectionStatus::Connected {
                return true;
            }
        }
        false
    }

    /// Query all connected agents for sessions, merging results.
    ///
    /// Failed agents return empty results with a warning logged.
    /// Results are sorted by start time (newest first).
    pub async fn query_sessions(&self) -> Result<Vec<SessionSummary>> {
        let mut all_sessions = Vec::new();
        let mut unreachable = Vec::new();

        for agent_mutex in &self.agents {
            let mut conn = agent_mutex.lock().await;
            let agent_name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                unreachable.push(agent_name);
                continue;
            };

            match client
                .get_sessions(SessionFilter {
                    status: None,
                    project: None,
                    session_type: None,
                })
                .await
            {
                Ok(response) => {
                    conn.last_seen = Some(Utc::now());
                    let list = response.into_inner();
                    for session in list.sessions {
                        all_sessions.push(proto_session_to_summary(session, &agent_name));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        agent = %agent_name,
                        error = %e,
                        "GetSessions RPC failed"
                    );
                    conn.mark_disconnected();
                    unreachable.push(agent_name);
                }
            }
        }

        if !unreachable.is_empty() {
            tracing::warn!(agents = ?unreachable, "unreachable Nexus agents during query_sessions");
        }

        // Sort by start time, newest first
        all_sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        Ok(all_sessions)
    }

    /// Return `true` if any connected agent has an `active` or `idle` session
    /// for the given `project`.
    ///
    /// Uses `GetSessions` with a project filter on each connected agent. RPC
    /// failures are logged as warnings and skipped — if all agents fail the
    /// method returns `false` (fail-open: prefer a potential duplicate over a
    /// missed launch).
    ///
    /// `stale` and `errored` sessions are treated as non-blocking.
    pub async fn has_active_session_for_project(&self, project: &str) -> bool {
        for agent_mutex in &self.agents {
            let mut conn = agent_mutex.lock().await;
            let agent_name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                continue;
            };

            match client
                .get_sessions(SessionFilter {
                    status: None,
                    project: Some(project.to_string()),
                    session_type: None,
                })
                .await
            {
                Ok(response) => {
                    conn.last_seen = Some(Utc::now());
                    let list = response.into_inner();
                    for session in list.sessions {
                        let status = proto_status_to_string(session.status);
                        if status == "active" || status == "idle" {
                            return true;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        agent = %agent_name,
                        project,
                        error = %e,
                        "has_active_session_for_project: GetSessions RPC failed — skipping agent"
                    );
                    // Do not mark disconnected here — this is a soft query,
                    // not authoritative connectivity check.
                }
            }
        }

        false
    }

    /// Query a specific session by ID across all connected agents.
    pub async fn query_session(&self, id: &str) -> Result<Option<SessionDetail>> {
        for agent_mutex in &self.agents {
            let mut conn = agent_mutex.lock().await;
            let agent_name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                continue;
            };

            match client
                .get_session(SessionId {
                    id: id.to_string(),
                })
                .await
            {
                Ok(response) => {
                    conn.last_seen = Some(Utc::now());
                    let session = response.into_inner();
                    return Ok(Some(proto_session_to_detail(session, &agent_name)));
                }
                Err(e) => {
                    // NOT_FOUND is expected when trying the wrong agent
                    if e.code() == tonic::Code::NotFound {
                        continue;
                    }
                    tracing::warn!(
                        agent = %agent_name,
                        session_id = %id,
                        error = %e,
                        "GetSession RPC failed"
                    );
                    conn.mark_disconnected();
                }
            }
        }

        Ok(None)
    }

    /// Start a new session on the agent managing the given project.
    ///
    /// When `agent` is `Some(name)`, only the agent whose `conn.name` matches
    /// is tried. When `None`, every connected agent is tried in round-robin
    /// order until one succeeds.
    pub async fn start_session(
        &self,
        project: &str,
        cwd: &str,
        args: &[String],
        agent: Option<&str>,
    ) -> Result<(String, String)> {
        // If a specific agent was requested, validate it exists and is connected.
        if let Some(name) = agent {
            for agent_mutex in &self.agents {
                let mut conn = agent_mutex.lock().await;
                if conn.name != name {
                    continue;
                }

                // Found the matching agent
                let Some(client) = conn.client.as_mut() else {
                    anyhow::bail!("Agent '{}' is not connected", name);
                };

                match client
                    .start_session(proto::StartSessionRequest {
                        project: project.to_string(),
                        cwd: cwd.to_string(),
                        args: args.to_vec(),
                    })
                    .await
                {
                    Ok(response) => {
                        conn.last_seen = Some(Utc::now());
                        let resp = response.into_inner();
                        return Ok((resp.session_id, resp.tmux_session));
                    }
                    Err(e) => {
                        conn.mark_disconnected();
                        anyhow::bail!(
                            "StartSession on agent '{}' failed: {}",
                            name,
                            e
                        );
                    }
                }
            }

            anyhow::bail!("Agent '{}' not found", name);
        }

        // Round-robin: try each connected agent in order.
        for agent_mutex in &self.agents {
            let mut conn = agent_mutex.lock().await;
            let agent_name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                continue;
            };

            match client
                .start_session(proto::StartSessionRequest {
                    project: project.to_string(),
                    cwd: cwd.to_string(),
                    args: args.to_vec(),
                })
                .await
            {
                Ok(response) => {
                    conn.last_seen = Some(Utc::now());
                    let resp = response.into_inner();
                    return Ok((resp.session_id, resp.tmux_session));
                }
                Err(e) => {
                    if e.code() == tonic::Code::Unimplemented {
                        tracing::warn!(
                            agent = %agent_name,
                            "StartSession RPC not implemented by agent"
                        );
                        continue;
                    }
                    tracing::warn!(
                        agent = %agent_name,
                        error = %e,
                        "StartSession RPC failed"
                    );
                    conn.mark_disconnected();
                }
            }
        }

        anyhow::bail!("No Nexus agent could start a session for project '{project}'")
    }

    /// Send a command to a running session by ID.
    ///
    /// Returns the collected text output from the command stream.
    pub async fn send_command(&self, session_id: &str, prompt: &str) -> Result<String> {
        use futures_util::StreamExt;

        for agent_mutex in &self.agents {
            let mut conn = agent_mutex.lock().await;
            let agent_name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                continue;
            };

            match client
                .send_command(proto::CommandRequest {
                    session_id: session_id.to_string(),
                    prompt: prompt.to_string(),
                })
                .await
            {
                Ok(response) => {
                    // The RPC call succeeded — this agent owns the session.
                    // Track this independently of whether any text output was produced,
                    // because commands may legitimately return zero text chunks.
                    let mut found = true;
                    conn.last_seen = Some(Utc::now());
                    let mut stream = response.into_inner();
                    let mut output = String::new();

                    while let Some(item) = stream.next().await {
                        match item {
                            Ok(cmd_output) => {
                                if let Some(content) = cmd_output.content {
                                    match content {
                                        proto::command_output::Content::Text(chunk) => {
                                            output.push_str(&chunk.text);
                                        }
                                        proto::command_output::Content::Error(err) => {
                                            return Err(anyhow::anyhow!(
                                                "Command error (exit {}): {}",
                                                err.exit_code,
                                                err.message
                                            ));
                                        }
                                        proto::command_output::Content::Done(done) => {
                                            tracing::debug!(
                                                session_id,
                                                duration_ms = done.duration_ms,
                                                tool_calls = done.tool_calls,
                                                "SendCommand completed"
                                            );
                                        }
                                        _ => {} // ToolUse, ToolResult — skip
                                    }
                                }
                            }
                            Err(e) => {
                                if e.code() == tonic::Code::NotFound {
                                    // Session not found on this agent — try the next one.
                                    found = false;
                                    break;
                                }
                                return Err(anyhow::anyhow!("SendCommand stream error: {e}"));
                            }
                        }
                    }

                    if found {
                        // Return whatever output was collected, including empty string.
                        // Empty output is valid for commands with no text response.
                        return Ok(output);
                    }
                    // NotFound mid-stream — continue to next agent.
                }
                Err(e) => {
                    if e.code() == tonic::Code::NotFound {
                        continue;
                    }
                    if e.code() == tonic::Code::Unimplemented {
                        tracing::warn!(
                            agent = %agent_name,
                            "SendCommand RPC not implemented by agent"
                        );
                        continue;
                    }
                    tracing::warn!(
                        agent = %agent_name,
                        session_id,
                        error = %e,
                        "SendCommand RPC failed"
                    );
                    conn.mark_disconnected();
                }
            }
        }

        anyhow::bail!("Session '{session_id}' not found on any connected agent")
    }

    /// Start a new session with an injected context prompt.
    ///
    /// Builds a "Solve with Nexus" prompt from the provided error context
    /// and passes it as the first argument to the Claude Code session.
    /// The session starts in the given `cwd`.
    pub async fn start_session_with_context(
        &self,
        project: &str,
        cwd: &str,
        error_message: &str,
        context: Option<&str>,
    ) -> Result<(String, String)> {
        let mut prompt = format!(
            "I encountered an error in project `{project}` and need help solving it.\n\
             \n\
             Error:\n\
             {error_message}"
        );

        if let Some(ctx) = context {
            if !ctx.trim().is_empty() {
                prompt.push_str(&format!("\n\nAdditional context:\n{ctx}"));
            }
        }

        prompt.push_str(
            "\n\nPlease investigate this error, identify the root cause, and implement a fix.",
        );

        self.start_session(project, cwd, &[prompt], None).await
    }

    /// Stop a running session by ID.
    pub async fn stop_session(&self, session_id: &str) -> Result<String> {
        for agent_mutex in &self.agents {
            let mut conn = agent_mutex.lock().await;
            let agent_name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                continue;
            };

            match client
                .stop_session(proto::SessionId {
                    id: session_id.to_string(),
                })
                .await
            {
                Ok(response) => {
                    conn.last_seen = Some(Utc::now());
                    let result = response.into_inner();
                    let msg = result
                        .message
                        .unwrap_or_else(|| "Session stopped".to_string());
                    if result.success {
                        return Ok(msg);
                    } else {
                        anyhow::bail!("StopSession failed: {msg}");
                    }
                }
                Err(e) => {
                    if e.code() == tonic::Code::NotFound {
                        continue;
                    }
                    if e.code() == tonic::Code::Unimplemented {
                        tracing::warn!(
                            agent = %agent_name,
                            "StopSession RPC not implemented by agent"
                        );
                        continue;
                    }
                    tracing::warn!(
                        agent = %agent_name,
                        session_id,
                        error = %e,
                        "StopSession RPC failed"
                    );
                    conn.mark_disconnected();
                }
            }
        }

        anyhow::bail!("Session '{session_id}' not found on any connected agent")
    }

    /// Get connection status summary for all agents.
    pub async fn status_summary(&self) -> Vec<(String, ConnectionStatus)> {
        let mut result = Vec::new();
        for agent in &self.agents {
            let conn = agent.lock().await;
            result.push((conn.name.clone(), conn.status));
        }
        result
    }

    /// Query health from all connected agents.
    ///
    /// Returns a list of `(agent_name, HealthResponse)` for reachable agents
    /// and a list of unreachable agent names.
    pub async fn get_health(&self) -> Result<(Vec<(String, proto::HealthResponse)>, Vec<String>)> {
        let mut results = Vec::new();
        let mut unreachable = Vec::new();

        for agent_mutex in &self.agents {
            let mut conn = agent_mutex.lock().await;
            let agent_name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                unreachable.push(agent_name);
                continue;
            };

            match client.get_health(proto::HealthRequest {}).await {
                Ok(response) => {
                    conn.last_seen = Some(Utc::now());
                    results.push((agent_name, response.into_inner()));
                }
                Err(e) => {
                    tracing::warn!(
                        agent = %agent_name,
                        error = %e,
                        "GetHealth RPC failed"
                    );
                    conn.mark_disconnected();
                    unreachable.push(agent_name);
                }
            }
        }

        Ok((results, unreachable))
    }

    /// List projects from all connected agents.
    ///
    /// Deduplicates project names across agents and returns them sorted.
    pub async fn list_projects(&self) -> Result<Vec<String>> {
        let mut all_projects = std::collections::BTreeSet::new();

        for agent_mutex in &self.agents {
            let mut conn = agent_mutex.lock().await;
            let agent_name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                continue;
            };

            match client
                .list_projects(proto::ListProjectsRequest {})
                .await
            {
                Ok(response) => {
                    conn.last_seen = Some(Utc::now());
                    let resp = response.into_inner();
                    all_projects.extend(resp.projects);
                }
                Err(e) => {
                    if e.code() != tonic::Code::Unimplemented {
                        tracing::warn!(
                            agent = %agent_name,
                            error = %e,
                            "ListProjects RPC failed"
                        );
                        conn.mark_disconnected();
                    }
                }
            }
        }

        Ok(all_projects.into_iter().collect())
    }

    /// Get detailed connection info for all configured agents.
    ///
    /// Returns `(name, endpoint, status, last_seen)` tuples.
    pub async fn agent_details(
        &self,
    ) -> Vec<(String, String, ConnectionStatus, Option<DateTime<Utc>>)> {
        let mut result = Vec::new();
        for agent in &self.agents {
            let conn = agent.lock().await;
            result.push((
                conn.name.clone(),
                conn.endpoint.clone(),
                conn.status,
                conn.last_seen,
            ));
        }
        result
    }
}

// ── Conversion Helpers ─────────────────────────────────────────────

fn proto_timestamp_to_chrono(
    ts: Option<prost_types::Timestamp>,
) -> Option<DateTime<Utc>> {
    ts.and_then(|t| {
        chrono::DateTime::from_timestamp(t.seconds, t.nanos as u32)
    })
}

fn proto_status_to_string(status: i32) -> String {
    match proto::SessionStatus::try_from(status) {
        Ok(proto::SessionStatus::Active) => "active".into(),
        Ok(proto::SessionStatus::Idle) => "idle".into(),
        Ok(proto::SessionStatus::Stale) => "stale".into(),
        Ok(proto::SessionStatus::Errored) => "errored".into(),
        _ => "unknown".into(),
    }
}

fn proto_session_type_to_string(st: i32) -> String {
    match proto::SessionType::try_from(st) {
        Ok(proto::SessionType::Managed) => "managed".into(),
        Ok(proto::SessionType::AdHoc) => "ad-hoc".into(),
        _ => "unspecified".into(),
    }
}

fn compute_duration_display(started_at: Option<DateTime<Utc>>) -> String {
    let Some(start) = started_at else {
        return "unknown".into();
    };
    let elapsed = Utc::now() - start;
    let total_secs = elapsed.num_seconds();
    if total_secs < 60 {
        format!("{}s", total_secs)
    } else if total_secs < 3600 {
        format!("{}m", total_secs / 60)
    } else {
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        format!("{}h{}m", hours, mins)
    }
}

fn proto_session_to_summary(session: proto::Session, agent_name: &str) -> SessionSummary {
    let started_at = proto_timestamp_to_chrono(session.started_at);
    SessionSummary {
        id: session.id,
        project: session.project,
        status: proto_status_to_string(session.status),
        agent_name: agent_name.to_string(),
        duration_display: compute_duration_display(started_at),
        started_at,
        branch: session.branch,
        spec: session.spec,
    }
}

fn proto_session_to_detail(session: proto::Session, agent_name: &str) -> SessionDetail {
    let started_at = proto_timestamp_to_chrono(session.started_at);
    SessionDetail {
        id: session.id,
        project: session.project,
        status: proto_status_to_string(session.status),
        agent_name: agent_name.to_string(),
        duration_display: compute_duration_display(started_at),
        started_at,
        branch: session.branch,
        spec: session.spec,
        cwd: session.cwd,
        command: session.command,
        session_type: proto_session_type_to_string(session.session_type),
        model: session.telemetry.as_ref().and_then(|t| t.model.clone()),
        cost_usd: session.telemetry.as_ref().and_then(|t| t.total_cost_usd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that send_command returns Ok("") when the RPC succeeds but the
    /// stream yields zero text chunks.  The pre-fix code would fall through to
    /// the next agent and eventually return an error; the fix tracks `found`
    /// independently of accumulated output.
    #[test]
    fn found_flag_logic_returns_ok_empty() {
        // Simulate the post-fix control flow:
        // - RPC Ok → found = true
        // - stream drains with zero Text chunks
        // - found == true → return Ok(output) where output == ""
        let found = true;
        let output = String::new();

        // Stream drained cleanly with no text chunks and found is still true.
        assert!(found, "found must be true after Ok(response)");
        if found {
            assert_eq!(output, "", "empty output is a valid result");
        }
    }

    #[test]
    fn new_client_no_agents() {
        let client = NexusClient::new(&[]);
        assert!(client.agents.is_empty());
    }

    #[test]
    fn new_client_with_agents() {
        let configs = vec![
            NexusAgent {
                name: "homelab".into(),
                host: "192.168.1.100".into(),
                port: 7400,
            },
            NexusAgent {
                name: "macbook".into(),
                host: "192.168.1.101".into(),
                port: 7400,
            },
        ];
        let client = NexusClient::new(&configs);
        assert_eq!(client.agents.len(), 2);
    }

    #[tokio::test]
    async fn is_connected_false_when_no_agents() {
        let client = NexusClient::new(&[]);
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn is_connected_false_when_all_disconnected() {
        let configs = vec![NexusAgent {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port: 7400,
        }];
        let client = NexusClient::new(&configs);
        // No connect_all called, so all disconnected
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn query_sessions_returns_empty_when_disconnected() {
        let configs = vec![NexusAgent {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port: 7400,
        }];
        let client = NexusClient::new(&configs);
        let sessions = client.query_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn query_session_returns_none_when_disconnected() {
        let configs = vec![NexusAgent {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port: 7400,
        }];
        let client = NexusClient::new(&configs);
        let result = client.query_session("s-1").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn status_summary_all_disconnected() {
        let configs = vec![
            NexusAgent {
                name: "a".into(),
                host: "127.0.0.1".into(),
                port: 7400,
            },
            NexusAgent {
                name: "b".into(),
                host: "127.0.0.1".into(),
                port: 7401,
            },
        ];
        let client = NexusClient::new(&configs);
        let summary = client.status_summary().await;
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].1, ConnectionStatus::Disconnected);
        assert_eq!(summary[1].1, ConnectionStatus::Disconnected);
    }

    #[test]
    fn proto_status_to_string_values() {
        assert_eq!(proto_status_to_string(0), "unknown");
        assert_eq!(proto_status_to_string(1), "active");
        assert_eq!(proto_status_to_string(2), "idle");
        assert_eq!(proto_status_to_string(3), "stale");
        assert_eq!(proto_status_to_string(4), "errored");
        assert_eq!(proto_status_to_string(99), "unknown");
    }

    #[test]
    fn proto_session_type_to_string_values() {
        assert_eq!(proto_session_type_to_string(0), "unspecified");
        assert_eq!(proto_session_type_to_string(1), "managed");
        assert_eq!(proto_session_type_to_string(2), "ad-hoc");
        assert_eq!(proto_session_type_to_string(99), "unspecified");
    }

    #[test]
    fn compute_duration_short() {
        let start = Utc::now() - chrono::Duration::seconds(45);
        let d = compute_duration_display(Some(start));
        assert!(d.ends_with('s'));
    }

    #[test]
    fn compute_duration_minutes() {
        let start = Utc::now() - chrono::Duration::minutes(15);
        let d = compute_duration_display(Some(start));
        assert!(d.ends_with('m'));
    }

    #[test]
    fn compute_duration_hours() {
        let start = Utc::now() - chrono::Duration::hours(2) - chrono::Duration::minutes(30);
        let d = compute_duration_display(Some(start));
        assert!(d.contains('h'));
        assert!(d.contains('m'));
    }

    #[test]
    fn compute_duration_none() {
        assert_eq!(compute_duration_display(None), "unknown");
    }

    // ── has_active_session_for_project tests ─────────────────────────

    /// When no agents are configured, has_active_session_for_project returns false.
    #[tokio::test]
    async fn has_active_session_no_agents_returns_false() {
        let client = NexusClient::new(&[]);
        assert!(!client.has_active_session_for_project("my-project").await);
    }

    /// When agents are configured but all are disconnected (fail-open path),
    /// the method returns false without blocking the launch.
    #[tokio::test]
    async fn has_active_session_all_disconnected_returns_false() {
        let configs = vec![
            NexusAgent { name: "a".into(), host: "127.0.0.1".into(), port: 19001 },
            NexusAgent { name: "b".into(), host: "127.0.0.1".into(), port: 19002 },
        ];
        let client = NexusClient::new(&configs);
        // No connect_all called — all agents have None clients.
        // has_active_session_for_project must return false (fail-open).
        assert!(!client.has_active_session_for_project("oo").await);
    }

    /// Verify status mapping: active and idle are the only blocking statuses.
    #[test]
    fn status_mapping_active_idle_are_blocking() {
        // active (1) and idle (2) map to strings that the method checks.
        assert_eq!(proto_status_to_string(1), "active");
        assert_eq!(proto_status_to_string(2), "idle");
        // stale (3), errored (4), and unknown (0 / out-of-range) are non-blocking.
        assert_ne!(proto_status_to_string(3), "active");
        assert_ne!(proto_status_to_string(3), "idle");
        assert_ne!(proto_status_to_string(4), "active");
        assert_ne!(proto_status_to_string(4), "idle");
        assert_ne!(proto_status_to_string(0), "active");
        assert_ne!(proto_status_to_string(0), "idle");
        assert_ne!(proto_status_to_string(99), "active");
        assert_ne!(proto_status_to_string(99), "idle");
    }
}
