//! Neon PostgreSQL read-only query tool.
//!
//! Executes read-only SQL against per-project Neon databases.
//! Connection strings resolved via `POSTGRES_URL_{PROJECT_CODE}` env vars
//! (e.g., `POSTGRES_URL_OO`).
//!
//! Safety guards:
//! * SQL keyword blocklist (INSERT, UPDATE, DELETE, DROP, ALTER, TRUNCATE, CREATE)
//! * Read-only transaction mode
//! * LIMIT 50 appended when missing
//! * Cell truncation at 200 chars

use std::time::Duration;

use anyhow::{bail, Result};
use tokio_postgres::types::Type;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ROWS: usize = 50;
const MAX_CELL_LEN: usize = 200;

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
    vec![ToolDefinition {
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
    }]
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

    // Multi-row → aligned table
    let num_cols = result.columns.len();
    let mut widths = vec![0usize; num_cols];

    // Measure column header widths
    for (i, col) in result.columns.iter().enumerate() {
        widths[i] = col.len();
    }

    // Measure data widths
    for row in &result.rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let mut lines = Vec::with_capacity(result.rows.len() + 2);

    // Header
    let header: String = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, col)| format!("{:<width$}", col, width = widths[i]))
        .collect::<Vec<_>>()
        .join(" | ");
    lines.push(header);

    // Separator
    let sep: String = widths
        .iter()
        .map(|w| "-".repeat(*w))
        .collect::<Vec<_>>()
        .join("-+-");
    lines.push(sep);

    // Data rows
    for row in &result.rows {
        let line: String = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let w = if i < num_cols { widths[i] } else { cell.len() };
                format!("{:<width$}", cell, width = w)
            })
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(line);
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
    fn test_format_multi_row_as_table() {
        let result = QueryResult {
            columns: vec!["id".into(), "name".into()],
            rows: vec![
                vec!["1".into(), "Alice".into()],
                vec!["2".into(), "Bob".into()],
            ],
        };
        let output = format_results(&result);
        // Should contain header, separator, and data rows
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 4); // header + separator + 2 data rows
        assert!(lines[0].contains("id"));
        assert!(lines[0].contains("name"));
        assert!(lines[1].contains("---"));
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
        let (latency, result) = timed(|| async { connect(&project).await }).await;

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
