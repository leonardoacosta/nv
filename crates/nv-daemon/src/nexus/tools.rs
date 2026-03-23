use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use super::client::{NexusClient, SessionSummary};
use crate::bash;

/// Format all sessions from all connected agents into a tool response.
///
/// Used by the `query_nexus` tool and digest gathering.
pub async fn format_query_sessions(client: &NexusClient) -> Result<String> {
    let sessions = client.query_sessions().await?;
    let status = client.status_summary().await;

    let mut output = String::new();

    // Report connectivity
    let connected: Vec<_> = status.iter().filter(|(_, s)| {
        *s == super::connection::ConnectionStatus::Connected
    }).collect();
    let disconnected: Vec<_> = status.iter().filter(|(_, s)| {
        *s != super::connection::ConnectionStatus::Connected
    }).collect();

    if !disconnected.is_empty() {
        let names: Vec<&str> = disconnected.iter().map(|(n, _)| n.as_str()).collect();
        output.push_str(&format!(
            "Note: {} agent(s) unreachable: {}\n\n",
            disconnected.len(),
            names.join(", ")
        ));
    }

    if sessions.is_empty() {
        if connected.is_empty() {
            output.push_str("No Nexus agents connected. Cannot query sessions.");
        } else {
            output.push_str("No active sessions across connected agents.");
        }
        return Ok(output);
    }

    output.push_str(&format!("{} session(s):\n", sessions.len()));
    for s in &sessions {
        output.push_str(&format_session_line(s));
        output.push('\n');
    }

    Ok(output)
}

/// Format a detailed session query for the `query_session` tool.
pub async fn format_query_session(client: &NexusClient, session_id: &str) -> Result<String> {
    let detail = client.query_session(session_id).await?;

    let Some(d) = detail else {
        return Ok(format!("Session '{}' not found across any connected agent.", session_id));
    };

    let mut output = String::new();
    output.push_str(&format!("Session: {}\n", d.id));
    output.push_str(&format!("Agent: {}\n", d.agent_name));
    output.push_str(&format!("Status: {}\n", d.status));
    output.push_str(&format!("Type: {}\n", d.session_type));

    if let Some(project) = &d.project {
        output.push_str(&format!("Project: {}\n", project));
    }
    if let Some(branch) = &d.branch {
        output.push_str(&format!("Branch: {}\n", branch));
    }
    if let Some(spec) = &d.spec {
        output.push_str(&format!("Spec: {}\n", spec));
    }

    output.push_str(&format!("CWD: {}\n", d.cwd));
    output.push_str(&format!("Duration: {}\n", d.duration_display));

    if let Some(cmd) = &d.command {
        output.push_str(&format!("Command: {}\n", cmd));
    }
    if let Some(model) = &d.model {
        output.push_str(&format!("Model: {}\n", model));
    }
    if let Some(cost) = d.cost_usd {
        output.push_str(&format!("Cost: ${:.2}\n", cost));
    }

    Ok(output)
}

/// Format a single session as a one-line summary.
fn format_session_line(s: &SessionSummary) -> String {
    let project = s.project.as_deref().unwrap_or("(no project)");
    let spec_suffix = s
        .spec
        .as_ref()
        .map(|sp| format!(" [{}]", sp))
        .unwrap_or_default();
    format!(
        "[{}] {}: {} -- {} ({}){}",
        s.agent_name, s.id, project, s.status, s.duration_display, spec_suffix
    )
}

/// Format the `bd ready` output for a project via scoped bash.
///
/// Used by the `nexus_project_ready` tool.
pub async fn format_project_ready(
    project_code: &str,
    project_registry: &HashMap<String, PathBuf>,
) -> Result<String> {
    let project_path = bash::validate_project(project_code, project_registry)?;
    let output = bash::execute_command(
        &bash::AllowedCommand::BdReady,
        project_path,
    )
    .await?;

    if output.trim().is_empty() {
        Ok(format!("[{project_code}] No issues ready for work."))
    } else {
        Ok(format!("[{project_code}] Ready queue:\n{output}"))
    }
}

/// List open proposals in `openspec/changes/` for a project.
///
/// Used by the `nexus_project_proposals` tool.
pub async fn format_project_proposals(
    project_code: &str,
    project_registry: &HashMap<String, PathBuf>,
) -> Result<String> {
    let project_path = bash::validate_project(project_code, project_registry)?;
    let changes_dir = project_path.join("openspec").join("changes");

    if !changes_dir.is_dir() {
        return Ok(format!(
            "[{project_code}] No openspec/changes/ directory found."
        ));
    }

    let mut proposals = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&changes_dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip archive directory
        if name == "archive" {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Check for proposal.md and tasks.md
        let has_proposal = path.join("proposal.md").exists();
        let has_tasks = path.join("tasks.md").exists();

        let status = if has_proposal && has_tasks {
            "ready"
        } else if has_proposal {
            "proposal only"
        } else {
            "incomplete"
        };

        proposals.push(format!("  {name} ({status})"));
    }

    if proposals.is_empty() {
        Ok(format!("[{project_code}] No open proposals."))
    } else {
        Ok(format!(
            "[{project_code}] {} proposal(s):\n{}",
            proposals.len(),
            proposals.join("\n")
        ))
    }
}

/// Format health info from all connected Nexus agents.
///
/// Used by the `query_nexus_health` tool.
pub async fn format_query_health(client: &NexusClient) -> Result<String> {
    let (health_list, unreachable) = client.get_health().await?;

    let mut output = String::new();

    if !unreachable.is_empty() {
        output.push_str(&format!(
            "Unreachable: {}\n\n",
            unreachable.join(", ")
        ));
    }

    if health_list.is_empty() {
        output.push_str("No Nexus agents reachable for health check.");
        return Ok(output);
    }

    for (agent_name, health) in &health_list {
        output.push_str(&format!("── {} ──\n", agent_name));
        output.push_str(&format!("  Uptime: {}s\n", health.uptime_seconds));
        output.push_str(&format!("  Sessions: {}\n", health.session_count));

        if let Some(machine) = &health.machine {
            output.push_str(&format!(
                "  CPU: {:.1}%  Memory: {:.1}/{:.1} GB  Disk: {:.1}/{:.1} GB\n",
                machine.cpu_percent,
                machine.memory_used_gb,
                machine.memory_total_gb,
                machine.disk_used_gb,
                machine.disk_total_gb,
            ));
            if !machine.load_avg.is_empty() {
                let loads: Vec<String> = machine.load_avg.iter().map(|l| format!("{:.2}", l)).collect();
                output.push_str(&format!("  Load: {}\n", loads.join(" ")));
            }
            if !machine.docker_containers.is_empty() {
                let running: Vec<&str> = machine
                    .docker_containers
                    .iter()
                    .filter(|c| c.running)
                    .map(|c| c.name.as_str())
                    .collect();
                let stopped: Vec<&str> = machine
                    .docker_containers
                    .iter()
                    .filter(|c| !c.running)
                    .map(|c| c.name.as_str())
                    .collect();
                output.push_str(&format!(
                    "  Docker: {} running, {} stopped\n",
                    running.len(),
                    stopped.len()
                ));
            }
        }

        if let Some(rl) = &health.latest_rate_limit {
            output.push_str(&format!(
                "  Rate Limit: {:.1}% ({}){}\n",
                rl.utilization_percent,
                rl.rate_limit_type,
                if rl.surpassed_threshold { " EXCEEDED" } else { "" },
            ));
        }

        output.push('\n');
    }

    Ok(output.trim_end().to_string())
}

/// Format the list of projects known to connected Nexus agents.
///
/// Used by the `query_nexus_projects` tool.
pub async fn format_query_projects(client: &NexusClient) -> Result<String> {
    let projects = client.list_projects().await?;

    if projects.is_empty() {
        return Ok("No projects found across connected Nexus agents.".to_string());
    }

    let mut output = format!("{} project(s):\n", projects.len());
    for project in &projects {
        output.push_str(&format!("  {project}\n"));
    }

    Ok(output.trim_end().to_string())
}

/// Format details about all configured Nexus agents.
///
/// Used by the `query_nexus_agents` tool.
pub async fn format_query_agents(client: &NexusClient) -> Result<String> {
    let details = client.agent_details().await;

    if details.is_empty() {
        return Ok("No Nexus agents configured.".to_string());
    }

    let mut output = format!("{} agent(s):\n", details.len());
    for (name, endpoint, status, last_seen) in &details {
        let seen = last_seen
            .map(|t| t.format("%H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "never".to_string());
        output.push_str(&format!(
            "  {name}: {status} ({endpoint}) — last seen: {seen}\n",
        ));
    }

    Ok(output.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_session_line_basic() {
        let s = SessionSummary {
            id: "s-abc123".into(),
            project: Some("otaku-odyssey".into()),
            status: "active".into(),
            agent_name: "homelab".into(),
            started_at: None,
            duration_display: "15m".into(),
            branch: None,
            spec: None,
        };
        let line = format_session_line(&s);
        assert!(line.contains("[homelab]"));
        assert!(line.contains("s-abc123"));
        assert!(line.contains("otaku-odyssey"));
        assert!(line.contains("active"));
        assert!(line.contains("15m"));
    }

    #[test]
    fn format_session_line_no_project() {
        let s = SessionSummary {
            id: "s-xyz".into(),
            project: None,
            status: "idle".into(),
            agent_name: "macbook".into(),
            started_at: None,
            duration_display: "2h30m".into(),
            branch: None,
            spec: None,
        };
        let line = format_session_line(&s);
        assert!(line.contains("(no project)"));
    }

    #[test]
    fn format_session_line_with_spec() {
        let s = SessionSummary {
            id: "s-1".into(),
            project: Some("nv".into()),
            status: "active".into(),
            agent_name: "homelab".into(),
            started_at: None,
            duration_display: "5m".into(),
            branch: Some("main".into()),
            spec: Some("nexus-integration".into()),
        };
        let line = format_session_line(&s);
        assert!(line.contains("[nexus-integration]"));
    }

    #[tokio::test]
    async fn format_query_sessions_no_agents() {
        let client = NexusClient::new(&[]);
        let result = format_query_sessions(&client).await.unwrap();
        assert!(result.contains("No Nexus agents connected"));
    }

    #[tokio::test]
    async fn format_query_sessions_disconnected() {
        let client = NexusClient::new(&[nv_core::config::NexusAgent {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port: 7400,
        }]);
        let result = format_query_sessions(&client).await.unwrap();
        assert!(result.contains("unreachable"));
        assert!(result.contains("No active sessions") || result.contains("No Nexus agents"));
    }

    #[tokio::test]
    async fn format_query_session_not_found() {
        let client = NexusClient::new(&[nv_core::config::NexusAgent {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port: 7400,
        }]);
        let result = format_query_session(&client, "nonexistent").await.unwrap();
        assert!(result.contains("not found"));
    }
}
