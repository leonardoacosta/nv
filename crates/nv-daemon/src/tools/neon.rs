//! Neon PostgreSQL read-only query tool and Neon REST API management tools.
//!
//! # Direct SQL (`neon_query`)
//! Executes read-only SQL against per-project Neon databases.
//! Connection strings resolved via `POSTGRES_URL_{PROJECT_CODE}` env vars
//! (e.g., `POSTGRES_URL_OO`).
//!
//! Safety guards:
//! * SQL keyword blocklist (INSERT, UPDATE, DELETE, DROP, ALTER, TRUNCATE, CREATE)
//! * Read-only transaction mode
//! * LIMIT 50 appended when missing
//! * Cell truncation at 200 chars
//!
//! # Neon REST API (`NeonApiClient`)
//! Read-only access to the Neon platform via the Neon API v2.
//! Auth via `NEON_API_KEY` env var (Bearer token).
//! Three tools: `neon_projects`, `neon_branches`, `neon_compute`.

use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use tokio_postgres::types::Type;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ROWS: usize = 50;
const MAX_CELL_LEN: usize = 200;

/// Base URL for the Neon REST API v2.
const NEON_API_BASE: &str = "https://console.neon.tech/api/v2";

/// HTTP timeout for Neon REST API requests.
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Blocked SQL keywords (case-insensitive). Checked before execution.
const BLOCKED_KEYWORDS: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "TRUNCATE", "CREATE",
];

// ── Types ────────────────────────────────────────────────────────────

/// Result of a Neon query: column headers + rows of string values.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

// ── Tool Definitions ─────────────────────────────────────────────────

pub fn neon_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "neon_query".into(),
            description: "Execute a read-only SQL query against a project's Neon PostgreSQL database. \
                Returns formatted results as a text table. Only SELECT queries allowed; \
                INSERT/UPDATE/DELETE/DROP/ALTER/TRUNCATE/CREATE are rejected. \
                Results limited to 50 rows."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl', 'mv', 'ss')"
                    },
                    "sql": {
                        "type": "string",
                        "description": "SQL SELECT query to execute"
                    }
                },
                "required": ["project", "sql"]
            }),
        },
        ToolDefinition {
            name: "neon_projects".into(),
            description: "List all Neon projects in the account. Returns project name, ID, region, \
                and creation date as an aligned table. Uses NEON_API_KEY env var for authentication."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "neon_branches".into(),
            description: "List all branches for a Neon project. Returns branch name, ID, parent branch ID, \
                creation date, and current state. The project_id is the Neon project ID (e.g. 'aged-bird-123456'), \
                not the Nova project code."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Neon project ID (e.g. 'aged-bird-123456'), not the Nova project code"
                    }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "neon_compute".into(),
            description: "List compute endpoints for a Neon project. Returns endpoint ID, type \
                (read_write/read_only), status (active/idle/suspended), autoscaling size range, \
                and last active timestamp. Optionally filter by branch_id."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Neon project ID (e.g. 'aged-bird-123456')"
                    },
                    "branch_id": {
                        "type": "string",
                        "description": "Optional branch ID to filter endpoints (e.g. 'br-small-pond-123456')"
                    }
                },
                "required": ["project_id"]
            }),
        },
    ]
}

// ── SQL Validation ───────────────────────────────────────────────────

/// Validate that the SQL does not contain mutation keywords.
///
/// Performs a case-insensitive word-boundary check for each blocked keyword.
/// Returns `Ok(())` if the SQL passes validation, or an error describing
/// the rejected keyword.
pub fn validate_sql(sql: &str) -> Result<()> {
    let upper = sql.to_uppercase();
    for keyword in BLOCKED_KEYWORDS {
        // Check for the keyword as a standalone word (preceded by start-of-string
        // or non-alphanumeric, followed by end-of-string or non-alphanumeric).
        if let Some(pos) = upper.find(keyword) {
            let before_ok = pos == 0
                || !upper.as_bytes()[pos - 1].is_ascii_alphanumeric()
                    && upper.as_bytes()[pos - 1] != b'_';
            let after_pos = pos + keyword.len();
            let after_ok = after_pos >= upper.len()
                || !upper.as_bytes()[after_pos].is_ascii_alphanumeric()
                    && upper.as_bytes()[after_pos] != b'_';
            if before_ok && after_ok {
                bail!(
                    "SQL rejected: contains blocked keyword '{keyword}'. Only read-only queries allowed."
                );
            }
        }
    }
    Ok(())
}

/// Append `LIMIT 50` if the query does not already contain a LIMIT clause.
pub fn ensure_limit(sql: &str) -> String {
    let upper = sql.to_uppercase();
    if upper.contains("LIMIT") {
        sql.to_string()
    } else {
        let trimmed = sql.trim_end().trim_end_matches(';');
        format!("{trimmed} LIMIT {MAX_ROWS}")
    }
}

// ── Connection ───────────────────────────────────────────────────────

/// Resolve the connection URL for a project code from env vars.
///
/// Looks up `POSTGRES_URL_{CODE}` (uppercase), e.g. `POSTGRES_URL_OO`.
fn resolve_connection_url(project: &str) -> Result<String> {
    let env_key = format!("POSTGRES_URL_{}", project.to_uppercase());
    std::env::var(&env_key).map_err(|_| {
        anyhow::anyhow!(
            "No connection string found for project '{project}'. \
             Set env var {env_key} with the Neon PostgreSQL connection URL."
        )
    })
}

/// Connect to a project's Neon database with TLS via rustls.
async fn connect(project: &str) -> Result<tokio_postgres::Client> {
    let url = resolve_connection_url(project)?;

    // Build rustls TLS connector with webpki root certificates
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);

    let (client, connection) = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio_postgres::connect(&url, tls),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Connection to Neon timed out after {CONNECT_TIMEOUT:?}"))??;

    // Spawn the connection task — it drives the TCP I/O
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!(error = %e, "Neon connection error");
        }
    });

    Ok(client)
}

// ── Query Execution ──────────────────────────────────────────────────

/// Execute a read-only query and return structured results.
async fn execute_readonly(client: &tokio_postgres::Client, sql: &str) -> Result<QueryResult> {
    // Set read-only transaction mode
    client
        .batch_execute("BEGIN TRANSACTION READ ONLY")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to begin read-only transaction: {e}"))?;

    let result = tokio::time::timeout(QUERY_TIMEOUT, client.query(sql, &[]))
        .await
        .map_err(|_| anyhow::anyhow!("Query timed out after {QUERY_TIMEOUT:?}"))?
        .map_err(|e| anyhow::anyhow!("Query error: {e}"))?;

    // Commit (no-op for read-only, but clean)
    let _ = client.batch_execute("COMMIT").await;

    // Extract column names
    let columns: Vec<String> = if let Some(first_row) = result.first() {
        first_row
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect()
    } else {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
        });
    };

    // Extract row values as strings
    let rows: Vec<Vec<String>> = result
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, col)| cell_to_string(row, i, col.type_()))
                .collect()
        })
        .collect();

    Ok(QueryResult { columns, rows })
}

/// Convert a cell value to a display string, truncating at MAX_CELL_LEN.
fn cell_to_string(row: &tokio_postgres::Row, idx: usize, pg_type: &Type) -> String {
    let raw = match *pg_type {
        Type::BOOL => row.get::<_, Option<bool>>(idx).map(|v| v.to_string()),
        Type::INT2 => row.get::<_, Option<i16>>(idx).map(|v| v.to_string()),
        Type::INT4 => row.get::<_, Option<i32>>(idx).map(|v| v.to_string()),
        Type::INT8 => row.get::<_, Option<i64>>(idx).map(|v| v.to_string()),
        Type::FLOAT4 => row.get::<_, Option<f32>>(idx).map(|v| v.to_string()),
        Type::FLOAT8 => row.get::<_, Option<f64>>(idx).map(|v| v.to_string()),
        _ => row.get::<_, Option<String>>(idx),
    };

    let val = raw.unwrap_or_else(|| "NULL".to_string());
    truncate_cell(&val)
}

/// Truncate a cell value to MAX_CELL_LEN characters.
fn truncate_cell(s: &str) -> String {
    if s.len() > MAX_CELL_LEN {
        format!("{}...", &s[..MAX_CELL_LEN - 3])
    } else {
        s.to_string()
    }
}

// ── Result Formatting ────────────────────────────────────────────────

/// Format query results as an aligned text table for Telegram delivery.
///
/// Single-row results are formatted as key-value pairs.
/// Multi-row results are formatted as an aligned column table.
pub fn format_results(result: &QueryResult) -> String {
    if result.rows.is_empty() {
        return "(no rows returned)".to_string();
    }

    // Single row → key: value format
    if result.rows.len() == 1 {
        let row = &result.rows[0];
        return result
            .columns
            .iter()
            .zip(row.iter())
            .map(|(col, val)| format!("{col}: {val}"))
            .collect::<Vec<_>>()
            .join("\n");
    }

    // Multi-row → numbered list with key: value per row
    let mut lines = Vec::with_capacity(result.rows.len() * (result.columns.len() + 1));

    for (row_idx, row) in result.rows.iter().enumerate() {
        lines.push(format!("**Row {}:**", row_idx + 1));
        for (col, val) in result.columns.iter().zip(row.iter()) {
            lines.push(format!("  {col}: {val}"));
        }
    }

    lines.join("\n")
}

// ── Public Entry Point ───────────────────────────────────────────────

/// Execute the neon_query tool: validate SQL, connect, query, format.
pub async fn neon_query(project: &str, sql: &str) -> Result<String> {
    // Validate SQL safety
    validate_sql(sql)?;

    // Ensure LIMIT
    let safe_sql = ensure_limit(sql);

    tracing::info!(
        project,
        sql = %safe_sql,
        "executing neon_query"
    );

    // Connect and execute
    let client = connect(project).await?;
    let result = execute_readonly(&client, &safe_sql).await?;

    let row_count = result.rows.len();
    let formatted = format_results(&result);

    tracing::info!(project, rows = row_count, "neon_query completed");

    Ok(formatted)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_sql ─────────────────────────────────────────────

    #[test]
    fn test_validate_sql_allows_select() {
        assert!(validate_sql("SELECT count(*) FROM users").is_ok());
        assert!(validate_sql("select id, name from orders where status = 'active'").is_ok());
        assert!(validate_sql("SELECT * FROM events LIMIT 10").is_ok());
    }

    #[test]
    fn test_validate_sql_blocks_insert() {
        assert!(validate_sql("INSERT INTO users (name) VALUES ('test')").is_err());
        assert!(validate_sql("insert into users (name) values ('test')").is_err());
    }

    #[test]
    fn test_validate_sql_blocks_update() {
        assert!(validate_sql("UPDATE users SET name = 'test' WHERE id = 1").is_err());
    }

    #[test]
    fn test_validate_sql_blocks_delete() {
        assert!(validate_sql("DELETE FROM users WHERE id = 1").is_err());
    }

    #[test]
    fn test_validate_sql_blocks_drop() {
        assert!(validate_sql("DROP TABLE users").is_err());
    }

    #[test]
    fn test_validate_sql_blocks_alter() {
        assert!(validate_sql("ALTER TABLE users ADD COLUMN age int").is_err());
    }

    #[test]
    fn test_validate_sql_blocks_truncate() {
        assert!(validate_sql("TRUNCATE users").is_err());
    }

    #[test]
    fn test_validate_sql_blocks_create() {
        assert!(validate_sql("CREATE TABLE test (id int)").is_err());
    }

    #[test]
    fn test_validate_sql_allows_keyword_in_string_context() {
        // "updated_at" contains "update" but is not a standalone keyword
        assert!(validate_sql("SELECT updated_at FROM orders").is_ok());
        // "created_at" contains "create" but is not a standalone keyword
        assert!(validate_sql("SELECT created_at FROM users").is_ok());
        // "deleted" contains "delete" but is not a standalone keyword
        assert!(validate_sql("SELECT * FROM users WHERE deleted = false").is_ok());
    }

    // ── ensure_limit ─────────────────────────────────────────────

    #[test]
    fn test_ensure_limit_appends_when_missing() {
        let result = ensure_limit("SELECT * FROM users");
        assert!(result.contains("LIMIT 50"));
    }

    #[test]
    fn test_ensure_limit_preserves_existing() {
        let sql = "SELECT * FROM users LIMIT 10";
        let result = ensure_limit(sql);
        assert_eq!(result, sql);
    }

    #[test]
    fn test_ensure_limit_strips_trailing_semicolon() {
        let result = ensure_limit("SELECT * FROM users;");
        assert_eq!(result, "SELECT * FROM users LIMIT 50");
    }

    // ── format_results ───────────────────────────────────────────

    #[test]
    fn test_format_empty_results() {
        let result = QueryResult {
            columns: vec!["id".into()],
            rows: vec![],
        };
        assert_eq!(format_results(&result), "(no rows returned)");
    }

    #[test]
    fn test_format_single_row_as_kv() {
        let result = QueryResult {
            columns: vec!["count".into(), "status".into()],
            rows: vec![vec!["42".into(), "active".into()]],
        };
        let output = format_results(&result);
        assert!(output.contains("count: 42"));
        assert!(output.contains("status: active"));
    }

    #[test]
    fn test_format_multi_row_as_list() {
        let result = QueryResult {
            columns: vec!["id".into(), "name".into()],
            rows: vec![
                vec!["1".into(), "Alice".into()],
                vec!["2".into(), "Bob".into()],
            ],
        };
        let output = format_results(&result);
        // Should contain row labels and key: value pairs
        assert!(output.contains("Row 1"));
        assert!(output.contains("id: 1"));
        assert!(output.contains("name: Alice"));
        assert!(output.contains("Row 2"));
        assert!(output.contains("name: Bob"));
    }

    // ── truncate_cell ────────────────────────────────────────────

    #[test]
    fn test_truncate_short_cell() {
        assert_eq!(truncate_cell("hello"), "hello");
    }

    #[test]
    fn test_truncate_long_cell() {
        let long = "a".repeat(300);
        let result = truncate_cell(&long);
        assert_eq!(result.len(), MAX_CELL_LEN);
        assert!(result.ends_with("..."));
    }

    // ── resolve_connection_url ───────────────────────────────────

    #[test]
    fn test_resolve_missing_env_var() {
        // Ensure the env var is not set
        std::env::remove_var("POSTGRES_URL_ZZZTEST");
        let result = resolve_connection_url("zzztest");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("POSTGRES_URL_ZZZTEST"));
    }

    // ── NeonApiClient ────────────────────────────────────────────

    #[test]
    fn test_neon_api_client_from_env_missing_key() {
        // Ensure the env var is not set
        std::env::remove_var("NEON_API_KEY");
        let result = NeonApiClient::from_env();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("NEON_API_KEY"));
    }

    #[test]
    fn test_project_summary_deserializes() {
        let json = serde_json::json!({
            "id": "aged-bird-123456",
            "name": "otaku-odyssey",
            "region_id": "aws-us-east-2",
            "created_at": "2024-01-15T12:00:00Z"
        });
        let p: ProjectSummary = serde_json::from_value(json).unwrap();
        assert_eq!(p.id, "aged-bird-123456");
        assert_eq!(p.name, "otaku-odyssey");
        assert_eq!(p.region_id, "aws-us-east-2");
    }

    #[test]
    fn test_project_summary_deserializes_with_missing_optional_fields() {
        // Only required fields — optional fields should default
        let json = serde_json::json!({
            "id": "silent-fog-999",
            "name": "test-project"
        });
        let p: ProjectSummary = serde_json::from_value(json).unwrap();
        assert_eq!(p.id, "silent-fog-999");
        assert!(p.region_id.is_empty());
        assert!(p.created_at.is_empty());
    }

    #[test]
    fn test_branch_summary_deserializes() {
        let json = serde_json::json!({
            "id": "br-small-pond-123456",
            "name": "main",
            "parent_id": "",
            "created_at": "2024-01-15T12:00:00Z",
            "current_state": "ready"
        });
        let b: BranchSummary = serde_json::from_value(json).unwrap();
        assert_eq!(b.id, "br-small-pond-123456");
        assert_eq!(b.name, "main");
        assert_eq!(b.current_state, "ready");
    }

    #[test]
    fn test_endpoint_summary_deserializes() {
        let json = serde_json::json!({
            "id": "ep-cool-rain-123456",
            "type": "read_write",
            "current_state": "idle",
            "branch_id": "br-small-pond-123456",
            "autoscaling_limit_min_cu": 0.25,
            "autoscaling_limit_max_cu": 0.25,
            "last_active": "2024-01-15T18:30:00Z"
        });
        let e: EndpointSummary = serde_json::from_value(json).unwrap();
        assert_eq!(e.id, "ep-cool-rain-123456");
        assert_eq!(e.endpoint_type, "read_write");
        assert_eq!(e.current_state, "idle");
    }

    #[test]
    fn test_format_projects_empty() {
        assert_eq!(format_projects(&[]), "No projects found.");
    }

    #[test]
    fn test_format_projects_list() {
        let projects = vec![
            ProjectSummary {
                id: "aged-bird-123456".into(),
                name: "otaku-odyssey".into(),
                region_id: "aws-us-east-2".into(),
                created_at: "2024-01-15T12:00:00Z".into(),
            },
            ProjectSummary {
                id: "silent-fog-999".into(),
                name: "tribal-cities".into(),
                region_id: "aws-us-east-2".into(),
                created_at: "2024-02-20T08:00:00Z".into(),
            },
        ];
        let output = format_projects(&projects);
        assert!(output.contains("Neon projects (2)"));
        assert!(output.contains("otaku-odyssey"));
        assert!(output.contains("aged-bird-123456"));
        assert!(output.contains("aws-us-east-2"));
        assert!(output.contains("tribal-cities"));
    }

    #[test]
    fn test_format_branches_empty() {
        assert_eq!(format_branches(&[]), "No branches found.");
    }

    #[test]
    fn test_format_endpoints_empty() {
        assert_eq!(format_endpoints(&[]), "No compute endpoints found.");
    }

    #[test]
    fn test_truncate_timestamp_full() {
        assert_eq!(truncate_timestamp("2024-01-15T12:00:00Z"), "2024-01-15");
    }

    #[test]
    fn test_truncate_timestamp_empty() {
        assert_eq!(truncate_timestamp(""), "—");
    }
}

// ── Neon API Types ───────────────────────────────────────────────────

/// Summary of a single Neon project returned by `GET /projects`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub region_id: String,
    #[serde(default)]
    pub created_at: String,
}

/// Envelope returned by `GET /projects`.
#[derive(Debug, Deserialize)]
struct ProjectsResponse {
    projects: Vec<ProjectSummary>,
}

/// Summary of a single Neon branch returned by `GET /projects/{id}/branches`.
#[derive(Debug, Clone, Deserialize)]
pub struct BranchSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent_id: String,
    #[serde(default)]
    #[allow(dead_code)] // part of Neon API response; reserved for future branch age checks
    pub created_at: String,
    #[serde(default)]
    pub current_state: String,
}

/// Envelope returned by `GET /projects/{id}/branches`.
#[derive(Debug, Deserialize)]
struct BranchesResponse {
    branches: Vec<BranchSummary>,
}

/// Summary of a single compute endpoint returned by `GET /projects/{id}/endpoints`.
#[derive(Debug, Clone, Deserialize)]
pub struct EndpointSummary {
    pub id: String,
    #[serde(rename = "type", default)]
    pub endpoint_type: String,
    #[serde(default)]
    pub current_state: String,
    #[serde(default)]
    pub branch_id: String,
    #[serde(default)]
    pub autoscaling_limit_min_cu: f64,
    #[serde(default)]
    pub autoscaling_limit_max_cu: f64,
    #[serde(default)]
    pub last_active: String,
}

/// Envelope returned by `GET /projects/{id}/endpoints`.
#[derive(Debug, Deserialize)]
struct EndpointsResponse {
    endpoints: Vec<EndpointSummary>,
}

// ── NeonApiClient ────────────────────────────────────────────────────

/// HTTP client for the Neon REST API v2.
///
/// Authenticates with `NEON_API_KEY` env var via Bearer token.
/// All operations are read-only.
#[derive(Debug)]
pub struct NeonApiClient {
    http: reqwest::Client,
    api_key: String,
}

impl NeonApiClient {
    /// Create a new client from the `NEON_API_KEY` environment variable.
    ///
    /// Returns `Err` if the env var is not set.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("NEON_API_KEY").map_err(|_| {
            anyhow!("NEON_API_KEY env var not set — required for Neon management tools")
        })?;
        if api_key.is_empty() {
            bail!("NEON_API_KEY env var is empty");
        }
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(API_REQUEST_TIMEOUT)
                .build()?,
            api_key,
        })
    }

    /// Build a GET request with Bearer auth header.
    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
    }

    /// Map common HTTP status codes to actionable error messages.
    fn map_status(status: reqwest::StatusCode, context: &str) -> anyhow::Error {
        match status.as_u16() {
            401 => anyhow!("Neon API key invalid (401) — check NEON_API_KEY env var"),
            403 => anyhow!("Neon API key lacks permissions (403) — check key scopes"),
            404 => anyhow!("{context} not found (404)"),
            429 => anyhow!("Neon API rate limited (429) — wait a few moments and retry"),
            code => anyhow!("Neon API error ({code}) for {context}"),
        }
    }

    // ── API Methods ──────────────────────────────────────────────────

    /// List all Neon projects in the account.
    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let url = format!("{NEON_API_BASE}/projects");
        let resp = self.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), "projects list"));
        }
        let envelope: ProjectsResponse = resp.json().await?;
        Ok(envelope.projects)
    }

    /// List all branches for a Neon project.
    pub async fn list_branches(&self, project_id: &str) -> Result<Vec<BranchSummary>> {
        let url = format!("{NEON_API_BASE}/projects/{project_id}/branches");
        let resp = self.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(Self::map_status(
                resp.status(),
                &format!("branches for project '{project_id}'"),
            ));
        }
        let envelope: BranchesResponse = resp.json().await?;
        Ok(envelope.branches)
    }

    /// List compute endpoints for a Neon project, optionally filtered by branch.
    pub async fn list_endpoints(
        &self,
        project_id: &str,
        branch_id: Option<&str>,
    ) -> Result<Vec<EndpointSummary>> {
        let url = format!("{NEON_API_BASE}/projects/{project_id}/endpoints");
        let resp = self.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(Self::map_status(
                resp.status(),
                &format!("endpoints for project '{project_id}'"),
            ));
        }
        let envelope: EndpointsResponse = resp.json().await?;
        let endpoints = match branch_id {
            Some(bid) => envelope
                .endpoints
                .into_iter()
                .filter(|e| e.branch_id == bid)
                .collect(),
            None => envelope.endpoints,
        };
        Ok(endpoints)
    }
}

// ── Neon API Formatting ──────────────────────────────────────────────

/// Format a list of Neon projects as a mobile-friendly list.
pub fn format_projects(projects: &[ProjectSummary]) -> String {
    if projects.is_empty() {
        return "No projects found.".to_string();
    }

    let mut lines = vec![format!("Neon projects ({}):", projects.len())];
    for p in projects {
        let created = crate::tools::relative_time(&p.created_at);
        let created = if created.is_empty() { truncate_timestamp(&p.created_at) } else { created };
        lines.push(format!("\u{1f5c3}\u{fe0f} **{}**", p.name));
        lines.push(format!("   ID: {} | Region: {} | Created: {}", p.id, p.region_id, created));
    }
    lines.join("\n")
}

/// Format a list of Neon branches as a mobile-friendly list.
pub fn format_branches(branches: &[BranchSummary]) -> String {
    if branches.is_empty() {
        return "No branches found.".to_string();
    }

    let mut lines = vec![format!("Neon branches ({}):", branches.len())];
    for b in branches {
        let parent = if b.parent_id.is_empty() { "(root)".to_string() } else { b.parent_id.clone() };
        lines.push(format!("\u{1f5c3}\u{fe0f} **{}** ({})", b.name, b.current_state));
        lines.push(format!("   ID: {} | Parent: {}", b.id, parent));
    }
    lines.join("\n")
}

/// Format a list of Neon compute endpoints as a mobile-friendly list.
pub fn format_endpoints(endpoints: &[EndpointSummary]) -> String {
    if endpoints.is_empty() {
        return "No compute endpoints found.".to_string();
    }

    let mut lines = vec![format!("Neon endpoints ({}):", endpoints.len())];
    for e in endpoints {
        let size = if e.autoscaling_limit_max_cu > 0.0 {
            format!("{}-{} CU", e.autoscaling_limit_min_cu, e.autoscaling_limit_max_cu)
        } else {
            "n/a".to_string()
        };
        let last_active = crate::tools::relative_time(&e.last_active);
        let last_active = if last_active.is_empty() { truncate_timestamp(&e.last_active) } else { last_active };
        lines.push(format!("\u{1f5c3}\u{fe0f} **{}** ({}) \u{2014} {}", e.id, e.endpoint_type, e.current_state));
        lines.push(format!("   Size: {size} | Last active: {last_active}"));
    }
    lines.join("\n")
}

/// Truncate an ISO 8601 timestamp to just the date portion for display.
fn truncate_timestamp(ts: &str) -> String {
    if ts.is_empty() {
        return "—".to_string();
    }
    // "2024-01-15T12:00:00Z" -> "2024-01-15"
    ts.get(..10).unwrap_or(ts).to_string()
}

// ── Neon API Entry Points ────────────────────────────────────────────

/// Execute the neon_projects tool: list all Neon projects in the account.
pub async fn neon_projects() -> Result<String> {
    tracing::info!("executing neon_projects");
    let client = NeonApiClient::from_env()?;
    let projects = client.list_projects().await?;
    tracing::info!(count = projects.len(), "neon_projects completed");
    Ok(format_projects(&projects))
}

/// Execute the neon_branches tool: list branches for a Neon project.
pub async fn neon_branches(project_id: &str) -> Result<String> {
    tracing::info!(project_id, "executing neon_branches");
    let client = NeonApiClient::from_env()?;
    let branches = client.list_branches(project_id).await?;
    tracing::info!(project_id, count = branches.len(), "neon_branches completed");
    Ok(format_branches(&branches))
}

/// Execute the neon_compute tool: list compute endpoints for a Neon project.
pub async fn neon_compute(project_id: &str, branch_id: Option<&str>) -> Result<String> {
    tracing::info!(project_id, branch_id, "executing neon_compute");
    let client = NeonApiClient::from_env()?;
    let endpoints = client.list_endpoints(project_id, branch_id).await?;
    tracing::info!(project_id, count = endpoints.len(), "neon_compute completed");
    Ok(format_endpoints(&endpoints))
}

// ── NeonClient wrapper ───────────────────────────────────────────────

/// Thin wrapper around the project-scoped Neon connection functions,
/// used for `Checkable` health checks.
///
/// Holds the project code for which connectivity is checked.
#[allow(dead_code)]
pub struct NeonClient {
    /// Project code (e.g. `"oo"`, `"tc"`).
    pub project: String,
}

#[allow(dead_code)]
impl NeonClient {
    /// Create a `NeonClient` for the given project code.
    pub fn new(project: impl Into<String>) -> Self {
        Self {
            project: project.into(),
        }
    }
}

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for NeonClient {
    fn name(&self) -> &str {
        "neon"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;

        // Resolve env var presence first — fast path for missing creds
        let env_key = format!("POSTGRES_URL_{}", self.project.to_uppercase());
        if std::env::var(&env_key).is_err() {
            return crate::tools::CheckResult::Missing { env_var: env_key };
        }

        let project = self.project.clone();
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async { connect(&project).await }).await;

        match result {
            Ok(client) => {
                // Run SELECT 1 to verify query capability
                match client.query_one("SELECT 1", &[]).await {
                    Ok(_) => crate::tools::CheckResult::Healthy {
                        latency_ms: latency,
                        detail: format!("SELECT 1 ok ({})", self.project.to_uppercase()),
                    },
                    Err(e) => crate::tools::CheckResult::Unhealthy {
                        error: format!("query failed: {e}"),
                    },
                }
            }
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: format!("connection failed: {e}"),
            },
        }
    }
}
