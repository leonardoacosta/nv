use anyhow::Result;

use super::client::{NexusClient, SessionSummary};

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
