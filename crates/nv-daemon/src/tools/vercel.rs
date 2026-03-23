//! Vercel deployment tools — read-only data source via Vercel REST API.
//!
//! Uses `reqwest` with Bearer token authentication (`VERCEL_TOKEN` env var)
//! to query deployments and build logs.  Two tools:
//!
//! * `vercel_deployments(project)` — list recent deployments with state/URL/branch.
//! * `vercel_logs(deploy_id)` — get build log events (filtered to errors/warnings).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

/// Base URL for the Vercel REST API.
const VERCEL_API: &str = "https://api.vercel.com";

/// HTTP request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Maximum build log events returned.
const MAX_LOG_EVENTS: usize = 50;

// ── Types ────────────────────────────────────────────────────────────

/// Summary of a single Vercel deployment (from the deployments list API).
#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentSummary {
    #[allow(dead_code)]
    pub uid: String,
    pub state: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<u64>,
    #[serde(rename = "ready")]
    #[allow(dead_code)]
    pub ready_at: Option<u64>,
    pub meta: Option<DeploymentMeta>,
}

/// Git metadata attached to a deployment.
#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentMeta {
    #[serde(rename = "githubCommitRef")]
    pub github_commit_ref: Option<String>,
    #[serde(rename = "githubCommitMessage")]
    pub github_commit_message: Option<String>,
}

/// Envelope returned by `GET /v6/deployments`.
#[derive(Debug, Deserialize)]
pub struct DeploymentsResponse {
    pub deployments: Vec<DeploymentSummary>,
}

/// Envelope returned by `GET /v9/projects/{name}`.
#[derive(Debug, Deserialize)]
pub struct ProjectResponse {
    pub id: String,
}

/// A single build log event from `GET /v2/deployments/{id}/events`.
#[derive(Debug, Clone, Deserialize)]
pub struct BuildEvent {
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub text: Option<String>,
    pub payload: Option<BuildEventPayload>,
}

/// Payload inside a build event (often contains the actual log text).
#[derive(Debug, Clone, Deserialize)]
pub struct BuildEventPayload {
    pub text: Option<String>,
}

// ── Client ───────────────────────────────────────────────────────────

/// HTTP client for the Vercel REST API.
#[derive(Debug)]
pub struct VercelClient {
    http: reqwest::Client,
    token: String,
    /// Cache: project name -> project ID.
    project_cache: Mutex<HashMap<String, String>>,
}

impl VercelClient {
    /// Create a new client from the `VERCEL_TOKEN` environment variable.
    ///
    /// Returns `Err` if the env var is not set or empty.
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("VERCEL_TOKEN")
            .map_err(|_| anyhow!("VERCEL_TOKEN env var not set"))?;
        if token.is_empty() {
            bail!("VERCEL_TOKEN env var is empty");
        }
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()?,
            token,
            project_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Build a GET request with Bearer auth header.
    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
    }

    /// Map common HTTP error codes to actionable messages.
    fn map_status(status: reqwest::StatusCode, context: &str) -> anyhow::Error {
        match status.as_u16() {
            401 => anyhow!("Vercel token expired or invalid (401) — regenerate at vercel.com/account/tokens"),
            403 => anyhow!("Vercel token lacks permissions (403) — check token scopes"),
            404 => anyhow!("{context} not found (404)"),
            429 => anyhow!("Vercel rate limit exceeded (429) — wait a few minutes"),
            code => anyhow!("Vercel API error ({code}) for {context}"),
        }
    }

    // ── Project Resolution ───────────────────────────────────────────

    /// Resolve a project name to its ID via `GET /v9/projects/{name}`.
    ///
    /// Results are cached in-memory for the lifetime of the client.
    pub async fn resolve_project_id(&self, name: &str) -> Result<String> {
        // Check cache first
        if let Some(id) = self.project_cache.lock().unwrap().get(name) {
            return Ok(id.clone());
        }

        let url = format!("{VERCEL_API}/v9/projects/{name}");
        let resp = self.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), &format!("Project '{name}'")));
        }

        let project: ProjectResponse = resp.json().await?;
        self.project_cache
            .lock()
            .unwrap()
            .insert(name.to_string(), project.id.clone());
        Ok(project.id)
    }

    // ── Deployments ──────────────────────────────────────────────────

    /// List recent deployments for a project.
    ///
    /// `project` can be a name or an ID.  If it looks like a name (no `prj_`
    /// prefix), we resolve it to an ID first.
    pub async fn list_deployments(&self, project: &str) -> Result<Vec<DeploymentSummary>> {
        let project_id = if project.starts_with("prj_") {
            project.to_string()
        } else {
            self.resolve_project_id(project).await?
        };

        let url = format!(
            "{VERCEL_API}/v6/deployments?projectId={project_id}&limit=10"
        );
        let resp = self.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(Self::map_status(
                resp.status(),
                &format!("Deployments for '{project}'"),
            ));
        }

        let body: DeploymentsResponse = resp.json().await?;
        Ok(body.deployments)
    }

    // ── Build Logs ───────────────────────────────────────────────────

    /// Get build log events for a deployment, filtered to errors and warnings.
    ///
    /// Returns at most `MAX_LOG_EVENTS` events.  If the full log is larger,
    /// only the last `MAX_LOG_EVENTS` are returned.
    pub async fn get_build_logs(&self, deploy_id: &str) -> Result<Vec<BuildEvent>> {
        let url = format!("{VERCEL_API}/v2/deployments/{deploy_id}/events");
        let resp = self.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(Self::map_status(
                resp.status(),
                &format!("Build logs for '{deploy_id}'"),
            ));
        }

        let events: Vec<BuildEvent> = resp.json().await?;

        // Filter to error/warning events, or all events if no errors found
        let filtered: Vec<BuildEvent> = events
            .iter()
            .filter(|e| {
                let text = e.text_content();
                let t = e.event_type.as_deref().unwrap_or("");
                t == "error" || t == "warning"
                    || text.contains("error")
                    || text.contains("Error")
                    || text.contains("ERR!")
                    || text.contains("warning")
                    || text.contains("WARN")
            })
            .cloned()
            .collect();

        // If we found error/warning lines, return those; otherwise return tail of all events
        let result = if filtered.is_empty() {
            let skip = events.len().saturating_sub(MAX_LOG_EVENTS);
            events.into_iter().skip(skip).collect()
        } else {
            let skip = filtered.len().saturating_sub(MAX_LOG_EVENTS);
            filtered.into_iter().skip(skip).collect()
        };

        Ok(result)
    }
}

// ── Telegram Formatters ──────────────────────────────────────────────

impl BuildEvent {
    /// Extract the text content from either the top-level `text` or `payload.text`.
    pub fn text_content(&self) -> &str {
        self.text
            .as_deref()
            .or_else(|| self.payload.as_ref().and_then(|p| p.text.as_deref()))
            .unwrap_or("")
    }
}

impl DeploymentSummary {
    /// Format a single deployment for Telegram output.
    pub fn format_for_telegram(&self) -> String {
        let state = self.state.as_deref().unwrap_or("UNKNOWN");
        let icon = match state {
            "READY" => "\u{2705}",     // green check
            "ERROR" => "\u{274c}",     // red X
            "BUILDING" => "\u{1f504}", // arrows
            "QUEUED" => "\u{23f3}",    // hourglass
            "CANCELED" => "\u{23f9}",  // stop
            _ => "\u{2753}",           // question mark
        };

        let branch = self
            .meta
            .as_ref()
            .and_then(|m| m.github_commit_ref.as_deref())
            .unwrap_or("?");

        let commit_msg = self
            .meta
            .as_ref()
            .and_then(|m| m.github_commit_message.as_deref())
            .map(|msg| {
                if msg.len() > 60 {
                    format!("{}...", &msg[..57])
                } else {
                    msg.to_string()
                }
            })
            .unwrap_or_default();

        let age = self
            .created_at
            .map(|ms| {
                // Convert ms to an ISO-8601-like string for relative_time
                let secs = ms / 1000;
                let rt = format_age(ms);
                // format_age already gives "5m ago" etc. — reuse it
                let _ = secs;
                rt
            })
            .unwrap_or_default();

        let url_part = self
            .url
            .as_deref()
            .map(|u| format!("\n   URL: {u}"))
            .unwrap_or_default();

        let age_part = if age.is_empty() {
            String::new()
        } else {
            format!(" | {age}")
        };

        format!(
            "\u{1f3d7}\u{fe0f} {icon} **{branch}** \u{2014} {state}\n   {commit_msg}{age_part}{url_part}"
        )
    }
}

/// Format a timestamp (ms since epoch) as a relative age string.
fn format_age(created_at_ms: u64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    if now_ms <= created_at_ms {
        return String::new();
    }

    let diff_secs = (now_ms - created_at_ms) / 1000;
    let age = if diff_secs < 60 {
        format!("{diff_secs}s ago")
    } else if diff_secs < 3600 {
        format!("{}m ago", diff_secs / 60)
    } else if diff_secs < 86400 {
        format!("{}h ago", diff_secs / 3600)
    } else {
        format!("{}d ago", diff_secs / 86400)
    };

    format!(" ({age})")
}

/// Format a list of deployments for Telegram output.
pub fn format_deployments_for_telegram(
    project: &str,
    deployments: &[DeploymentSummary],
) -> String {
    if deployments.is_empty() {
        return format!("No deployments found for {project}.");
    }
    let mut lines = vec![format!(
        "\u{1f3d7}\u{fe0f} **{project}** \u{2014} {} deployment(s):",
        deployments.len()
    )];
    for d in deployments {
        lines.push(d.format_for_telegram());
    }
    lines.join("\n\n")
}

/// Format build log events for Telegram output.
pub fn format_build_logs_for_telegram(deploy_id: &str, events: &[BuildEvent]) -> String {
    if events.is_empty() {
        return format!("No build log events for {deploy_id}.");
    }

    let mut lines = vec![format!(
        "\u{1f3d7}\u{fe0f} Build log: {deploy_id} ({} event(s)):",
        events.len()
    )];
    for event in events {
        let text = event.text_content();
        if !text.is_empty() {
            // Highlight error lines
            let t = event.event_type.as_deref().unwrap_or("");
            if t == "error"
                || text.contains("error")
                || text.contains("Error")
                || text.contains("ERR!")
            {
                lines.push(format!("\u{274c} {text}"));
            } else {
                lines.push(text.to_string());
            }
        }
    }
    lines.join("\n")
}

// ── Public Tool Handlers ─────────────────────────────────────────────

/// List recent deployments for a Vercel project.
pub async fn vercel_deployments(client: &VercelClient, project: &str) -> Result<String> {
    if project.is_empty() {
        bail!("project name cannot be empty");
    }
    let deployments = client.list_deployments(project).await?;
    Ok(format_deployments_for_telegram(project, &deployments))
}

/// Get build logs for a Vercel deployment.
pub async fn vercel_logs(client: &VercelClient, deploy_id: &str) -> Result<String> {
    if deploy_id.is_empty() {
        bail!("deploy_id cannot be empty");
    }
    let events = client.get_build_logs(deploy_id).await?;
    Ok(format_build_logs_for_telegram(deploy_id, &events))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all Vercel tools.
pub fn vercel_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "vercel_deployments".into(),
            description: "List recent Vercel deployments for a project. Returns deployment state (READY/ERROR/BUILDING/QUEUED/CANCELED), URL, git branch, commit message, and age. Project can be a name or ID.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Vercel project name or ID (e.g. 'otaku-odyssey', 'prj_abc123')"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "vercel_logs".into(),
            description: "Get build log events for a Vercel deployment. Returns error and warning lines from the build output. Use a deployment ID from vercel_deployments.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "deploy_id": {
                        "type": "string",
                        "description": "The deployment ID (uid) from vercel_deployments"
                    }
                },
                "required": ["deploy_id"]
            }),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vercel_tool_definitions_returns_two_tools() {
        let tools = vercel_tool_definitions();
        assert_eq!(tools.len(), 2);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"vercel_deployments"));
        assert!(names.contains(&"vercel_logs"));
    }

    #[test]
    fn tool_definitions_have_correct_schema() {
        let tools = vercel_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn vercel_deployments_schema_requires_project() {
        let tools = vercel_tool_definitions();
        let vd = tools
            .iter()
            .find(|t| t.name == "vercel_deployments")
            .unwrap();
        let required = vd.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("project")));
    }

    #[test]
    fn vercel_logs_schema_requires_deploy_id() {
        let tools = vercel_tool_definitions();
        let vl = tools.iter().find(|t| t.name == "vercel_logs").unwrap();
        let required = vl.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("deploy_id")));
    }

    #[test]
    fn parse_deployment_summary() {
        let json = r#"{
            "uid": "dpl_abc123",
            "state": "READY",
            "url": "my-app-abc123.vercel.app",
            "createdAt": 1711100000000,
            "ready": 1711100060000,
            "meta": {
                "githubCommitRef": "main",
                "githubCommitMessage": "fix: resolve login bug"
            }
        }"#;
        let d: DeploymentSummary = serde_json::from_str(json).unwrap();
        assert_eq!(d.uid, "dpl_abc123");
        assert_eq!(d.state.as_deref(), Some("READY"));
        assert_eq!(d.url.as_deref(), Some("my-app-abc123.vercel.app"));
        assert_eq!(
            d.meta.as_ref().unwrap().github_commit_ref.as_deref(),
            Some("main")
        );
    }

    #[test]
    fn parse_deployments_response() {
        let json = r#"{"deployments": [
            {"uid": "dpl_1", "state": "READY", "url": null, "createdAt": null, "ready": null, "meta": null},
            {"uid": "dpl_2", "state": "ERROR", "url": null, "createdAt": null, "ready": null, "meta": null}
        ]}"#;
        let resp: DeploymentsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.deployments.len(), 2);
        assert_eq!(resp.deployments[0].uid, "dpl_1");
        assert_eq!(resp.deployments[1].state.as_deref(), Some("ERROR"));
    }

    #[test]
    fn parse_build_event() {
        let json = r#"{
            "type": "error",
            "text": "Type error: cannot find module",
            "payload": null
        }"#;
        let e: BuildEvent = serde_json::from_str(json).unwrap();
        assert_eq!(e.event_type.as_deref(), Some("error"));
        assert_eq!(e.text_content(), "Type error: cannot find module");
    }

    #[test]
    fn parse_build_event_with_payload() {
        let json = r#"{
            "type": "stdout",
            "text": null,
            "payload": {"text": "Building..."}
        }"#;
        let e: BuildEvent = serde_json::from_str(json).unwrap();
        assert_eq!(e.text_content(), "Building...");
    }

    #[test]
    fn parse_project_response() {
        let json = r#"{"id": "prj_abc123"}"#;
        let p: ProjectResponse = serde_json::from_str(json).unwrap();
        assert_eq!(p.id, "prj_abc123");
    }

    #[test]
    fn format_deployment_ready() {
        let d = DeploymentSummary {
            uid: "dpl_1".into(),
            state: Some("READY".into()),
            url: Some("my-app.vercel.app".into()),
            created_at: None,
            ready_at: None,
            meta: Some(DeploymentMeta {
                github_commit_ref: Some("main".into()),
                github_commit_message: Some("fix: login".into()),
            }),
        };
        let formatted = d.format_for_telegram();
        assert!(formatted.contains("\u{2705}"));
        assert!(formatted.contains("READY"));
        assert!(formatted.contains("main"));
        assert!(formatted.contains("fix: login"));
        assert!(formatted.contains("my-app.vercel.app"));
    }

    #[test]
    fn format_deployment_error() {
        let d = DeploymentSummary {
            uid: "dpl_2".into(),
            state: Some("ERROR".into()),
            url: None,
            created_at: None,
            ready_at: None,
            meta: Some(DeploymentMeta {
                github_commit_ref: Some("feat/x".into()),
                github_commit_message: Some("wip".into()),
            }),
        };
        let formatted = d.format_for_telegram();
        assert!(formatted.contains("\u{274c}"));
        assert!(formatted.contains("ERROR"));
        assert!(formatted.contains("feat/x"));
    }

    #[test]
    fn format_deployment_truncates_long_commit_msg() {
        let long_msg = "a".repeat(80);
        let d = DeploymentSummary {
            uid: "dpl_3".into(),
            state: Some("READY".into()),
            url: None,
            created_at: None,
            ready_at: None,
            meta: Some(DeploymentMeta {
                github_commit_ref: Some("main".into()),
                github_commit_message: Some(long_msg),
            }),
        };
        let formatted = d.format_for_telegram();
        assert!(formatted.contains("..."));
        // Truncated to 57 chars + "..."
        assert!(formatted.len() < 200);
    }

    #[test]
    fn format_deployments_empty() {
        let output = format_deployments_for_telegram("test-project", &[]);
        assert!(output.contains("No deployments found"));
        assert!(output.contains("test-project"));
    }

    #[test]
    fn format_deployments_list() {
        let deployments = vec![
            DeploymentSummary {
                uid: "dpl_1".into(),
                state: Some("READY".into()),
                url: None,
                created_at: None,
                ready_at: None,
                meta: None,
            },
            DeploymentSummary {
                uid: "dpl_2".into(),
                state: Some("ERROR".into()),
                url: None,
                created_at: None,
                ready_at: None,
                meta: None,
            },
        ];
        let output = format_deployments_for_telegram("my-app", &deployments);
        assert!(output.contains("my-app"));
        assert!(output.contains("2 deployment(s)"));
        assert!(output.contains("READY"));
        assert!(output.contains("ERROR"));
    }

    #[test]
    fn format_build_logs_empty() {
        let output = format_build_logs_for_telegram("dpl_1", &[]);
        assert!(output.contains("No build log events"));
    }

    #[test]
    fn format_build_logs_with_errors() {
        let events = vec![
            BuildEvent {
                event_type: Some("error".into()),
                text: Some("Type error in main.ts".into()),
                payload: None,
            },
            BuildEvent {
                event_type: Some("stdout".into()),
                text: Some("Compiling...".into()),
                payload: None,
            },
        ];
        let output = format_build_logs_for_telegram("dpl_1", &events);
        assert!(output.contains("2 event(s)"));
        assert!(output.contains("\u{274c} Type error in main.ts"));
        assert!(output.contains("Compiling..."));
        assert!(output.contains("dpl_1"));
    }

    #[test]
    fn format_age_recent() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        // 30 seconds ago
        let age = format_age(now_ms - 30_000);
        assert!(age.contains("30s ago"));
    }

    #[test]
    fn format_age_minutes() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        // 5 minutes ago
        let age = format_age(now_ms - 300_000);
        assert!(age.contains("5m ago"));
    }

    #[test]
    fn format_age_hours() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        // 2 hours ago
        let age = format_age(now_ms - 7_200_000);
        assert!(age.contains("2h ago"));
    }

    #[test]
    fn format_age_future_returns_empty() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let age = format_age(now_ms + 60_000);
        assert!(age.is_empty());
    }

    #[test]
    fn build_event_text_content_prefers_text() {
        let e = BuildEvent {
            event_type: None,
            text: Some("direct text".into()),
            payload: Some(BuildEventPayload {
                text: Some("payload text".into()),
            }),
        };
        assert_eq!(e.text_content(), "direct text");
    }

    #[test]
    fn build_event_text_content_falls_back_to_payload() {
        let e = BuildEvent {
            event_type: None,
            text: None,
            payload: Some(BuildEventPayload {
                text: Some("from payload".into()),
            }),
        };
        assert_eq!(e.text_content(), "from payload");
    }

    #[test]
    fn build_event_text_content_empty_when_none() {
        let e = BuildEvent {
            event_type: None,
            text: None,
            payload: None,
        };
        assert_eq!(e.text_content(), "");
    }

    #[test]
    fn client_from_env_fails_without_token() {
        // Temporarily unset VERCEL_TOKEN if it's set
        let saved = std::env::var("VERCEL_TOKEN").ok();
        std::env::remove_var("VERCEL_TOKEN");
        let result = VercelClient::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("VERCEL_TOKEN env var not set"));
        // Restore
        if let Some(val) = saved {
            std::env::set_var("VERCEL_TOKEN", val);
        }
    }
}

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for VercelClient {
    fn name(&self) -> &str {
        "vercel"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;
        let (latency, result) = timed(|| async {
            self.get(&format!("{VERCEL_API}/v2/user")).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "user endpoint reachable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => crate::tools::CheckResult::Unhealthy {
                error: "token expired or invalid (401) — check VERCEL_TOKEN".into(),
            },
            Ok(resp) => crate::tools::CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}
