//! Azure DevOps tools via REST API (dev.azure.com).
//!
//! Three read-only tools:
//! * `ado_projects()` — list all projects in the configured org.
//! * `ado_pipelines(project)` — list pipeline definitions for a project.
//! * `ado_builds(project, pipeline_id)` — list recent builds for a pipeline.
//!
//! Auth: Basic auth with PAT via `ADO_PAT` env var. Org via `ADO_ORG`.

use std::time::Duration;

use anyhow::{anyhow, Result};
use base64::Engine;
use serde::Deserialize;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_PIPELINES: usize = 50;
const MAX_BUILDS: usize = 10;

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct AdoProject {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub state: String,
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectsResponse {
    value: Vec<AdoProject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdoPipeline {
    pub id: u32,
    pub name: String,
    pub folder: Option<String>,
    #[allow(dead_code)]
    pub revision: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct PipelinesResponse {
    value: Vec<AdoPipeline>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdoBuild {
    #[serde(rename = "buildNumber")]
    pub build_number: Option<String>,
    pub status: Option<String>,
    pub result: Option<String>,
    #[serde(rename = "queueTime")]
    pub queue_time: Option<String>,
    #[serde(rename = "finishTime")]
    pub finish_time: Option<String>,
    #[serde(rename = "sourceBranch")]
    pub source_branch: Option<String>,
    #[serde(rename = "requestedFor")]
    pub requested_for: Option<AdoIdentity>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdoIdentity {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BuildsResponse {
    value: Vec<AdoBuild>,
}

// ── Client ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct AdoClient {
    http: reqwest::Client,
    org_url: String,
}

impl AdoClient {
    /// Create a new `AdoClient` from environment variables.
    ///
    /// Requires `ADO_ORG` and `ADO_PAT`. Returns an error if either is missing.
    pub fn from_env() -> Result<Self> {
        let org = std::env::var("ADO_ORG")
            .map_err(|_| anyhow!("Azure DevOps not configured — ADO_ORG env var not set"))?;
        let pat = std::env::var("ADO_PAT")
            .map_err(|_| anyhow!("Azure DevOps not configured — ADO_PAT env var not set"))?;

        let org_url = format!("https://dev.azure.com/{org}");

        // Basic auth: empty username + PAT as password
        let credentials = base64::engine::general_purpose::STANDARD
            .encode(format!(":{pat}"));

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Basic {credentials}")
                .parse()
                .expect("valid auth header"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()?;

        Ok(Self { http, org_url })
    }

    /// List all projects in the configured org.
    pub async fn projects(&self) -> Result<Vec<AdoProject>> {
        let url = format!(
            "{}/_apis/projects?api-version=7.0",
            self.org_url
        );
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ADO API error ({status}): {body}");
        }

        let data: ProjectsResponse = resp.json().await?;
        Ok(data.value)
    }

    /// List pipeline definitions for a project.
    pub async fn pipelines(&self, project: &str) -> Result<Vec<AdoPipeline>> {
        let url = format!(
            "{}/{}/_apis/pipelines?api-version=7.1&$top={}",
            self.org_url, project, MAX_PIPELINES
        );
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ADO API error ({status}): {body}");
        }

        let data: PipelinesResponse = resp.json().await?;
        Ok(data.value)
    }

    /// List recent builds for a pipeline in a project.
    pub async fn builds(&self, project: &str, pipeline_id: u32) -> Result<Vec<AdoBuild>> {
        let url = format!(
            "{}/{}/_apis/build/builds?definitions={}&$top={}&api-version=7.1",
            self.org_url, project, pipeline_id, MAX_BUILDS
        );
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ADO API error ({status}): {body}");
        }

        let data: BuildsResponse = resp.json().await?;
        Ok(data.value)
    }
}

// ── Tool Definitions ─────────────────────────────────────────────────

pub fn ado_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "ado_projects".into(),
            description: "List all Azure DevOps projects in the configured organization. \
                Returns project name, state, and last update date. \
                Use this to discover available projects before calling ado_pipelines or ado_builds."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "ado_pipelines".into(),
            description: "List Azure DevOps pipeline definitions for a project. \
                Returns pipeline ID, name, and folder. Max 50 pipelines."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Azure DevOps project name"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "ado_builds".into(),
            description: "List recent Azure DevOps builds for a pipeline. \
                Returns build number, status, result, branch, requester, and timestamps. \
                Last 10 builds."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Azure DevOps project name"
                    },
                    "pipeline_id": {
                        "type": "integer",
                        "description": "Pipeline definition ID (from ado_pipelines)"
                    }
                },
                "required": ["project", "pipeline_id"]
            }),
        },
    ]
}

// ── Formatting ───────────────────────────────────────────────────────

/// Format projects as a readable list.
pub fn format_projects(projects: &[AdoProject]) -> String {
    if projects.is_empty() {
        return "(no projects found)".to_string();
    }

    let mut lines = vec![format!("Projects ({}):", projects.len())];
    for p in projects {
        let date = p
            .last_update_time
            .as_deref()
            .and_then(|s| s.split('T').next())
            .unwrap_or("unknown");
        lines.push(format!("  {} ({}) — last updated {}", p.name, p.state, date));
    }
    lines.join("\n")
}

/// Format pipeline definitions as a readable list.
pub fn format_pipelines(pipelines: &[AdoPipeline]) -> String {
    if pipelines.is_empty() {
        return "(no pipelines found)".to_string();
    }

    let mut lines = vec![format!("Pipelines ({}):", pipelines.len())];
    for p in pipelines {
        let folder = p.folder.as_deref().unwrap_or("/");
        lines.push(format!("  [{}] {} (folder: {})", p.id, p.name, folder));
    }
    lines.join("\n")
}

/// Format builds as a readable list.
pub fn format_builds(builds: &[AdoBuild]) -> String {
    if builds.is_empty() {
        return "(no builds found)".to_string();
    }

    let mut lines = vec![format!("Recent builds ({}):", builds.len())];
    for b in builds {
        let number = b.build_number.as_deref().unwrap_or("?");
        let status = b.status.as_deref().unwrap_or("unknown");
        let result = b.result.as_deref().unwrap_or("-");
        let branch = b
            .source_branch
            .as_deref()
            .unwrap_or("?")
            .trim_start_matches("refs/heads/");
        let requester = b
            .requested_for
            .as_ref()
            .and_then(|r| r.display_name.as_deref())
            .unwrap_or("unknown");
        let queued = b.queue_time.as_deref().unwrap_or("-");
        let finished = b.finish_time.as_deref().unwrap_or("-");

        lines.push(format!(
            "  #{number} | {status}/{result} | {branch} | by {requester} | queued: {queued} | finished: {finished}"
        ));
    }
    lines.join("\n")
}

// ── Public Entry Points ──────────────────────────────────────────────

/// Execute ado_projects: list all projects in the configured org.
pub async fn ado_projects() -> Result<String> {
    let client = AdoClient::from_env()?;
    let projects = client.projects().await?;
    tracing::info!(count = projects.len(), "ado_projects completed");
    Ok(format_projects(&projects))
}

/// Execute ado_pipelines: fetch pipeline definitions for a project.
pub async fn ado_pipelines(project: &str) -> Result<String> {
    let client = AdoClient::from_env()?;
    let pipelines = client.pipelines(project).await?;
    tracing::info!(project, count = pipelines.len(), "ado_pipelines completed");
    Ok(format_pipelines(&pipelines))
}

/// Execute ado_builds: fetch recent builds for a pipeline.
pub async fn ado_builds(project: &str, pipeline_id: u32) -> Result<String> {
    let client = AdoClient::from_env()?;
    let builds = client.builds(project, pipeline_id).await?;
    tracing::info!(project, pipeline_id, count = builds.len(), "ado_builds completed");
    Ok(format_builds(&builds))
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_projects_empty() {
        assert_eq!(format_projects(&[]), "(no projects found)");
    }

    #[test]
    fn test_format_projects_list() {
        let projects = vec![
            AdoProject {
                id: "abc-123".into(),
                name: "MyProject".into(),
                state: "wellFormed".into(),
                last_update_time: Some("2026-03-15T10:00:00Z".into()),
            },
            AdoProject {
                id: "def-456".into(),
                name: "OtherProject".into(),
                state: "wellFormed".into(),
                last_update_time: None,
            },
        ];
        let output = format_projects(&projects);
        assert!(output.contains("Projects (2)"));
        assert!(output.contains("MyProject (wellFormed)"));
        assert!(output.contains("last updated 2026-03-15"));
        assert!(output.contains("OtherProject (wellFormed)"));
        assert!(output.contains("last updated unknown"));
    }

    #[test]
    fn test_format_pipelines_empty() {
        assert_eq!(format_pipelines(&[]), "(no pipelines found)");
    }

    #[test]
    fn test_format_pipelines_list() {
        let pipelines = vec![
            AdoPipeline {
                id: 1,
                name: "CI Build".into(),
                folder: Some("\\builds".into()),
                revision: Some(5),
            },
            AdoPipeline {
                id: 2,
                name: "CD Deploy".into(),
                folder: None,
                revision: Some(3),
            },
        ];
        let output = format_pipelines(&pipelines);
        assert!(output.contains("Pipelines (2)"));
        assert!(output.contains("[1] CI Build"));
        assert!(output.contains("[2] CD Deploy"));
        assert!(output.contains("folder: \\builds"));
    }

    #[test]
    fn test_format_builds_empty() {
        assert_eq!(format_builds(&[]), "(no builds found)");
    }

    #[test]
    fn test_format_builds_list() {
        let builds = vec![AdoBuild {
            build_number: Some("20260322.1".into()),
            status: Some("completed".into()),
            result: Some("succeeded".into()),
            queue_time: Some("2026-03-22T10:00:00Z".into()),
            finish_time: Some("2026-03-22T10:05:00Z".into()),
            source_branch: Some("refs/heads/main".into()),
            requested_for: Some(AdoIdentity {
                display_name: Some("Leo".into()),
            }),
        }];
        let output = format_builds(&builds);
        assert!(output.contains("Recent builds (1)"));
        assert!(output.contains("#20260322.1"));
        assert!(output.contains("completed/succeeded"));
        assert!(output.contains("main"));
        assert!(output.contains("by Leo"));
    }

    #[test]
    fn test_ado_client_from_env_missing_org() {
        std::env::remove_var("ADO_ORG");
        std::env::remove_var("ADO_PAT");
        let result = AdoClient::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ADO_ORG"));
    }
}

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for AdoClient {
    fn name(&self) -> &str {
        "ado"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;
        // GET /_apis/projects — list projects at the org level
        let url = format!("{}/_apis/projects?api-version=7.1", self.org_url);
        let (latency, result) =
            timed(|| async { self.http.get(&url).send().await }).await;
        match result {
            Ok(resp) if resp.status().is_success() => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "projects endpoint reachable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => crate::tools::CheckResult::Unhealthy {
                error: "PAT invalid or expired (401) — check ADO_PAT".into(),
            },
            Ok(resp) if resp.status().as_u16() == 403 => crate::tools::CheckResult::Unhealthy {
                error: "PAT lacks read permission (403) — check ADO_PAT scopes".into(),
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
