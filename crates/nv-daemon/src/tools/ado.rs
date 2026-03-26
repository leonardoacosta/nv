//! Azure DevOps tools via REST API (dev.azure.com).
//!
//! Four read-only tools:
//! * `ado_projects()` — list all projects in the configured org.
//! * `ado_pipelines(project)` — list pipeline definitions for a project.
//! * `ado_builds(project, pipeline_id)` — list recent builds for a pipeline.
//! * `query_ado_work_items(project, ...)` — query work items via WIQL.
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

// ── Work Item Types ───────────────────────────────────────────────────

/// Fields for an ADO work item (populated via batch fetch).
#[derive(Debug, Clone, Deserialize)]
pub struct AdoWorkItemFields {
    #[serde(rename = "System.Id")]
    pub system_id: Option<u32>,
    #[serde(rename = "System.Title")]
    pub system_title: Option<String>,
    #[serde(rename = "System.State")]
    pub system_state: Option<String>,
    #[serde(rename = "System.WorkItemType")]
    pub system_work_item_type: Option<String>,
    #[serde(rename = "System.AssignedTo")]
    pub system_assigned_to: Option<AdoIdentity>,
    #[serde(rename = "System.ChangedDate")]
    pub system_changed_date: Option<String>,
}

/// A single ADO work item with ID and fields.
#[derive(Debug, Clone, Deserialize)]
pub struct AdoWorkItem {
    pub id: u32,
    pub fields: AdoWorkItemFields,
}

/// WIQL query result — list of work item references.
#[derive(Debug, Deserialize)]
struct WiqlResult {
    #[serde(rename = "workItems")]
    work_items: Vec<WiqlWorkItemRef>,
}

#[derive(Debug, Deserialize)]
struct WiqlWorkItemRef {
    id: u32,
}

/// Batch work item fetch response.
#[derive(Debug, Deserialize)]
struct WorkItemsBatchResponse {
    value: Vec<AdoWorkItem>,
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

    /// Create a client using an AAD Bearer token instead of PAT (Req-5 optional).
    ///
    /// Useful for organizations that require AAD-only auth.
    pub fn from_aad_token(org: &str, token: &str) -> Result<Self> {
        let org_url = format!("https://dev.azure.com/{org}");

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}")
                .parse()
                .map_err(|_| anyhow!("invalid AAD token"))?,
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()?;

        Ok(Self { http, org_url })
    }

    /// Query work items via WIQL and batch-fetch their details.
    ///
    /// Runs the WIQL query against `/{project}/_apis/wit/wiql`, extracts
    /// work item IDs (capped at `limit`), then fetches full details in one
    /// batch request.
    pub async fn work_items_by_wiql(
        &self,
        project: &str,
        wiql: &str,
        limit: usize,
    ) -> Result<Vec<AdoWorkItem>> {
        // Step 1: WIQL query
        let wiql_url = format!(
            "{org_url}/{project}/_apis/wit/wiql?api-version=7.1",
            org_url = self.org_url
        );
        let body = serde_json::json!({ "query": wiql });

        let wiql_resp = self
            .http
            .post(&wiql_url)
            .json(&body)
            .send()
            .await?;

        if !wiql_resp.status().is_success() {
            let status = wiql_resp.status();
            let text = wiql_resp.text().await.unwrap_or_default();
            anyhow::bail!("ADO WIQL error ({status}): {text}");
        }

        let wiql_result: WiqlResult = wiql_resp.json().await?;

        let ids: Vec<u32> = wiql_result
            .work_items
            .into_iter()
            .take(limit)
            .map(|r| r.id)
            .collect();

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: Batch fetch work item details
        let ids_csv: String = ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
        let fields = "System.Id,System.Title,System.State,System.WorkItemType,System.AssignedTo,System.ChangedDate";
        let batch_url = format!(
            "{org_url}/_apis/wit/workitems?ids={ids_csv}&fields={fields}&api-version=7.1",
            org_url = self.org_url
        );

        let batch_resp = self.http.get(&batch_url).send().await?;

        if !batch_resp.status().is_success() {
            let status = batch_resp.status();
            let text = batch_resp.text().await.unwrap_or_default();
            anyhow::bail!("ADO work items batch error ({status}): {text}");
        }

        let batch: WorkItemsBatchResponse = batch_resp.json().await?;
        Ok(batch.value)
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
        ToolDefinition {
            name: "query_ado_work_items".into(),
            description: "Query Azure DevOps work items assigned to a user using WIQL. \
                Returns work item ID, type, title, state, assignee, and last changed date. \
                Defaults to work items assigned to @Me (PAT identity) that are not Closed. \
                Use this to check active tasks, bugs, and features in an ADO project."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Azure DevOps project name (default: ADO_PROJECT env var)."
                    },
                    "assigned_to": {
                        "type": "string",
                        "description": "Filter by assignee — use '@Me' for the authenticated PAT user, or a display name (default: '@Me')."
                    },
                    "state": {
                        "type": "string",
                        "description": "Filter by state: 'active', 'new', 'resolved', or 'all' (default: 'active' — excludes Closed).",
                        "enum": ["active", "new", "resolved", "all"]
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum work items to return (default: 20, max: 50).",
                        "minimum": 1,
                        "maximum": 50
                    }
                },
                "required": []
            }),
        },
    ]
}

// ── Formatting ───────────────────────────────────────────────────────

/// Format projects as a mobile-friendly list.
pub fn format_projects(projects: &[AdoProject]) -> String {
    if projects.is_empty() {
        return "No projects found.".to_string();
    }

    let mut lines = vec![format!("ADO projects ({}):", projects.len())];
    for p in projects {
        let date = p
            .last_update_time
            .as_deref()
            .map(crate::tools::relative_time)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        lines.push(format!("\u{1f4c1} **{}** ({})", p.name, p.state));
        lines.push(format!("   Last updated: {date}"));
    }
    lines.join("\n")
}

/// Format pipeline definitions as a mobile-friendly list.
pub fn format_pipelines(pipelines: &[AdoPipeline]) -> String {
    if pipelines.is_empty() {
        return "No pipelines found.".to_string();
    }

    let mut lines = vec![format!("ADO pipelines ({}):", pipelines.len())];
    for p in pipelines {
        let folder = p.folder.as_deref().unwrap_or("/");
        lines.push(format!("\u{1f504} [{}] **{}**", p.id, p.name));
        lines.push(format!("   Folder: {folder}"));
    }
    lines.join("\n")
}

/// Format builds as a mobile-friendly list.
pub fn format_builds(builds: &[AdoBuild]) -> String {
    if builds.is_empty() {
        return "No builds found.".to_string();
    }

    let mut lines = vec![format!("Recent builds ({}):", builds.len())];
    for b in builds {
        let number = b.build_number.as_deref().unwrap_or("?");
        let status = b.status.as_deref().unwrap_or("unknown");
        let result = b.result.as_deref().unwrap_or("-");
        let status_icon = match result {
            "succeeded" => "\u{2705}",
            "failed" => "\u{274c}",
            "canceled" => "\u{23f9}",
            _ => match status {
                "inProgress" => "\u{1f504}",
                "notStarted" => "\u{23f3}",
                _ => "\u{2753}",
            },
        };
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
        let queued = b
            .queue_time
            .as_deref()
            .map(crate::tools::relative_time)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".to_string());
        let finished = b
            .finish_time
            .as_deref()
            .map(crate::tools::relative_time)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".to_string());

        lines.push(format!(
            "\u{1f3d7}\u{fe0f} #{number} {status_icon} {result} \u{2014} {branch}"
        ));
        lines.push(format!(
            "   By {requester} | Queued: {queued} | Finished: {finished}"
        ));
    }
    lines.join("\n")
}

/// Format work items as a mobile-friendly list.
pub fn format_work_items(project: &str, items: &[AdoWorkItem]) -> String {
    let active_count = items.len();
    let mut lines = vec![format!(
        "Work Items — {project} ({active_count} active)"
    )];

    if items.is_empty() {
        lines.push("(no work items found)".to_string());
        return lines.join("\n");
    }

    for item in items {
        let id = item.fields.system_id.unwrap_or(item.id);
        let title = item.fields.system_title.as_deref().unwrap_or("(no title)");
        let state = item.fields.system_state.as_deref().unwrap_or("Unknown");
        let work_type = item.fields.system_work_item_type.as_deref().unwrap_or("Item");

        let assignee = item
            .fields
            .system_assigned_to
            .as_ref()
            .and_then(|a| a.display_name.as_deref())
            .unwrap_or("Unassigned");

        let changed = item
            .fields
            .system_changed_date
            .as_deref()
            .and_then(|dt| dt.get(..10))
            .unwrap_or("unknown");

        lines.push(format!("[#{id}] {work_type} — {title} [{state}]"));
        lines.push(format!("  Assigned: {assignee} | Changed: {changed}"));
    }

    lines.join("\n")
}

/// Build a WIQL query string from the given filter parameters.
pub fn build_wiql(project: &str, assigned_to: &str, state_filter: &str) -> String {
    let state_clause = match state_filter {
        "all" => String::new(),
        "new" => " AND [System.State] = 'New'".to_string(),
        "resolved" => " AND [System.State] = 'Resolved'".to_string(),
        // "active" and default
        _ => " AND [System.State] <> 'Closed'".to_string(),
    };

    let assigned_clause = if assigned_to == "@Me" {
        " AND [System.AssignedTo] = @Me".to_string()
    } else {
        format!(" AND [System.AssignedTo] = '{assigned_to}'")
    };

    format!(
        "SELECT [System.Id], [System.Title], [System.State], [System.WorkItemType], \
         [System.AssignedTo] FROM WorkItems \
         WHERE [System.TeamProject] = '{project}'{assigned_clause}{state_clause} \
         ORDER BY [System.ChangedDate] DESC"
    )
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

/// Execute query_ado_work_items: fetch work items via WIQL.
///
/// `project` defaults to `ADO_PROJECT` env var. `assigned_to` defaults to `@Me`.
/// `state_filter`: `"active"` (default), `"new"`, `"resolved"`, or `"all"`.
pub async fn query_ado_work_items(
    project: Option<&str>,
    assigned_to: &str,
    state_filter: &str,
    limit: usize,
) -> Result<String> {
    let client = AdoClient::from_env()?;

    let project_name = project
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ADO_PROJECT").ok())
        .ok_or_else(|| {
            anyhow!(
                "Azure DevOps project not configured — pass 'project' parameter or set ADO_PROJECT env var"
            )
        })?;

    let wiql = build_wiql(&project_name, assigned_to, state_filter);
    let items = client.work_items_by_wiql(&project_name, &wiql, limit).await?;

    tracing::info!(
        project = %project_name,
        count = items.len(),
        "query_ado_work_items completed"
    );

    Ok(format_work_items(&project_name, &items))
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
            timed(std::time::Duration::from_secs(15), || async { self.http.get(&url).send().await }).await;
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

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_projects_empty() {
        assert_eq!(format_projects(&[]), "No projects found.");
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
        assert!(output.contains("ADO projects (2)"));
        assert!(output.contains("MyProject"));
        assert!(output.contains("wellFormed"));
        assert!(output.contains("Last updated:"));
        assert!(output.contains("OtherProject"));
    }

    #[test]
    fn test_format_pipelines_empty() {
        assert_eq!(format_pipelines(&[]), "No pipelines found.");
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
        assert!(output.contains("ADO pipelines (2)"));
        assert!(output.contains("[1]"));
        assert!(output.contains("CI Build"));
        assert!(output.contains("[2]"));
        assert!(output.contains("CD Deploy"));
        assert!(output.contains("Folder: \\builds"));
    }

    #[test]
    fn test_format_builds_empty() {
        assert_eq!(format_builds(&[]), "No builds found.");
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
        assert!(output.contains("succeeded"));
        assert!(output.contains("main"));
        assert!(output.contains("By Leo"));
    }

    #[test]
    fn test_ado_client_from_env_missing_org() {
        std::env::remove_var("ADO_ORG");
        std::env::remove_var("ADO_PAT");
        let result = AdoClient::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ADO_ORG"));
    }

    // ── Work Item Tests ───────────────────────────────────────────────

    #[test]
    fn test_format_work_items_empty() {
        let output = format_work_items("Wholesale Architecture", &[]);
        assert!(output.contains("Wholesale Architecture"));
        assert!(output.contains("0 active"));
        assert!(output.contains("no work items found"));
    }

    #[test]
    fn test_format_work_items_list() {
        let items = vec![
            AdoWorkItem {
                id: 12345,
                fields: AdoWorkItemFields {
                    system_id: Some(12345),
                    system_title: Some("Login timeout on Azure AD redirect".into()),
                    system_state: Some("Active".into()),
                    system_work_item_type: Some("Bug".into()),
                    system_assigned_to: Some(AdoIdentity {
                        display_name: Some("Leonardo Acosta".into()),
                    }),
                    system_changed_date: Some("2026-03-24T10:00:00Z".into()),
                },
            },
            AdoWorkItem {
                id: 12301,
                fields: AdoWorkItemFields {
                    system_id: Some(12301),
                    system_title: Some("Migrate event schema to v2".into()),
                    system_state: Some("New".into()),
                    system_work_item_type: Some("Task".into()),
                    system_assigned_to: Some(AdoIdentity {
                        display_name: Some("Leonardo Acosta".into()),
                    }),
                    system_changed_date: Some("2026-03-23T09:00:00Z".into()),
                },
            },
        ];

        let output = format_work_items("Wholesale Architecture", &items);
        assert!(output.contains("Wholesale Architecture"));
        assert!(output.contains("2 active"));
        assert!(output.contains("[#12345]"));
        assert!(output.contains("Bug"));
        assert!(output.contains("Login timeout on Azure AD redirect"));
        assert!(output.contains("Active"));
        assert!(output.contains("Leonardo Acosta"));
        assert!(output.contains("[#12301]"));
        assert!(output.contains("Task"));
        assert!(output.contains("New"));
    }

    #[test]
    fn test_build_wiql_default_active() {
        let wiql = build_wiql("MyProject", "@Me", "active");
        assert!(wiql.contains("MyProject"));
        assert!(wiql.contains("@Me"));
        assert!(wiql.contains("'Closed'"));
        assert!(wiql.contains("ORDER BY"));
    }

    #[test]
    fn test_build_wiql_new_state() {
        let wiql = build_wiql("MyProject", "@Me", "new");
        assert!(wiql.contains("[System.State] = 'New'"));
    }

    #[test]
    fn test_build_wiql_all_states() {
        let wiql = build_wiql("MyProject", "@Me", "all");
        // "all" has no state filter in the WHERE clause
        assert!(!wiql.contains("[System.State] <> 'Closed'"));
        assert!(!wiql.contains("[System.State] = 'New'"));
    }

    #[test]
    fn test_build_wiql_custom_assignee() {
        let wiql = build_wiql("MyProject", "John Doe", "active");
        assert!(wiql.contains("'John Doe'"));
        assert!(!wiql.contains("@Me"));
    }

    #[test]
    fn test_ado_tool_definitions_count() {
        let defs = ado_tool_definitions();
        assert_eq!(defs.len(), 4);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"ado_projects"));
        assert!(names.contains(&"ado_pipelines"));
        assert!(names.contains(&"ado_builds"));
        assert!(names.contains(&"query_ado_work_items"));
    }
}
