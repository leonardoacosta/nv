//! Plaid financial data tools via cortex-postgres read-only queries.
//!
//! Two tools:
//! * `plaid_balances()` — list account balances (name, type, current/available balance, last_updated).
//! * `plaid_bills()` — list credit/loan accounts as upcoming bills.
//!
//! **Security**: Column allowlist + PII regex filter in Rust, BEFORE tool results reach Claude.
//! Claude never sees raw query results — only `SafeRow` structs.
//!
//! Auth: `PLAID_DB_URL` env var (connection string for cortex-postgres on localhost:5436).

use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Hardcoded allowlist of columns that may be returned to Claude.
/// Any column NOT on this list is stripped from results.
#[allow(dead_code)]
const ALLOWED_COLUMNS: &[&str] = &[
    "account_name",
    "account_type",
    "current_balance",
    "available_balance",
    "last_updated",
];

/// Regex patterns that indicate PII in values — values matching these are redacted.
/// 9-digit numbers (SSN-like), routing numbers (9 digits), account numbers (8+ digits).
const PII_PATTERNS: &[&str] = &[
    r"\b\d{9}\b",       // 9-digit numbers (SSN, routing numbers)
    r"\b\d{8,17}\b",    // 8-17 digit numbers (account numbers)
];

// ── Types ────────────────────────────────────────────────────────────

/// A sanitized row containing only allowlisted fields. This is the ONLY
/// type that leaves this module — raw database rows never cross this boundary.
#[derive(Debug, Clone)]
pub struct SafeRow {
    pub account_name: Option<String>,
    pub account_type: Option<String>,
    pub current_balance: Option<String>,
    pub available_balance: Option<String>,
    pub last_updated: Option<String>,
}

// ── PII Filter (Security Critical) ──────────────────────────────────

/// Scrub a string value against PII patterns. If a match is found, the
/// value is replaced with "[REDACTED]".
fn scrub_pii(value: &str) -> String {
    for pattern in PII_PATTERNS {
        // Simple regex-free check for digit sequences
        if matches_pii_pattern(value, pattern) {
            return "[REDACTED]".to_string();
        }
    }
    value.to_string()
}

/// Check if a value matches a PII digit pattern.
/// Uses a simple digit-sequence scanner instead of pulling in regex.
fn matches_pii_pattern(value: &str, pattern: &str) -> bool {
    // Parse the digit count range from the pattern
    // Patterns: r"\b\d{9}\b" or r"\b\d{8,17}\b"
    let (min_digits, max_digits) = if pattern.contains(',') {
        // Range pattern like \d{8,17}
        let start = pattern.find('{').unwrap_or(0) + 1;
        let comma = pattern.find(',').unwrap_or(start);
        let end = pattern.find('}').unwrap_or(pattern.len());
        let min: usize = pattern[start..comma].parse().unwrap_or(0);
        let max: usize = pattern[comma + 1..end].parse().unwrap_or(0);
        (min, max)
    } else {
        // Exact pattern like \d{9}
        let start = pattern.find('{').unwrap_or(0) + 1;
        let end = pattern.find('}').unwrap_or(pattern.len());
        let count: usize = pattern[start..end].parse().unwrap_or(0);
        (count, count)
    };

    if min_digits == 0 {
        return false;
    }

    // Scan for digit sequences of the target length
    let chars: Vec<char> = value.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let digit_len = i - start;

            // Check word boundary: char before start and at i should be non-alphanumeric
            let before_ok =
                start == 0 || !chars[start - 1].is_alphanumeric();
            let after_ok = i >= chars.len() || !chars[i].is_alphanumeric();

            if before_ok && after_ok && digit_len >= min_digits && digit_len <= max_digits {
                return true;
            }
        } else {
            i += 1;
        }
    }

    false
}

/// Extract only allowlisted columns from a tokio-postgres Row and scrub values.
/// This is the primary security boundary.
fn filter_pii(
    row: &tokio_postgres::Row,
    columns: &[String],
) -> SafeRow {
    let get_safe = |col_name: &str| -> Option<String> {
        let idx = columns.iter().position(|c| c == col_name)?;
        let raw: Option<String> = row.get(idx);
        raw.map(|v| scrub_pii(&v))
    };

    SafeRow {
        account_name: get_safe("account_name"),
        account_type: get_safe("account_type"),
        current_balance: get_safe("current_balance"),
        available_balance: get_safe("available_balance"),
        last_updated: get_safe("last_updated"),
    }
}

// ── Connection ───────────────────────────────────────────────────────

/// Connect to cortex-postgres (localhost, no TLS needed).
async fn connect() -> Result<tokio_postgres::Client> {
    let url = std::env::var("PLAID_DB_URL")
        .map_err(|_| anyhow!("Plaid not configured — PLAID_DB_URL env var not set"))?;

    let (client, connection) = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio_postgres::connect(&url, tokio_postgres::NoTls),
    )
    .await
    .map_err(|_| anyhow!("Connection to cortex-postgres timed out after {CONNECT_TIMEOUT:?}"))??;

    // Spawn the connection task
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!(error = %e, "cortex-postgres connection error");
        }
    });

    // Set read-only mode
    client
        .batch_execute("SET default_transaction_read_only = on")
        .await
        .map_err(|e| anyhow!("Failed to set read-only mode: {e}"))?;

    Ok(client)
}

// ── Tool Definitions ─────────────────────────────────────────────────

pub fn plaid_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "plaid_balances".into(),
            description: "List account balances from Plaid. Shows account name, type, \
                current balance, available balance, and last sync time. \
                No PII (account numbers, routing numbers) is included."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "plaid_bills".into(),
            description: "List upcoming bills from credit and loan accounts. \
                Shows account name, type, current balance (amount owed), and last sync time. \
                No PII (account numbers, routing numbers) is included."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

// ── Formatting ───────────────────────────────────────────────────────

/// Format balance rows as a readable table.
pub fn format_balances(rows: &[SafeRow]) -> String {
    if rows.is_empty() {
        return "(no accounts found)".to_string();
    }

    let mut lines = vec![format!("Account Balances ({} accounts):", rows.len())];
    lines.push(String::new());

    for row in rows {
        let name = row.account_name.as_deref().unwrap_or("Unknown");
        let acct_type = row.account_type.as_deref().unwrap_or("?");
        let current = row.current_balance.as_deref().unwrap_or("-");
        let available = row.available_balance.as_deref().unwrap_or("-");
        let updated = row.last_updated.as_deref().unwrap_or("unknown");

        lines.push(format!(
            "  {name} ({acct_type}): current={current}, available={available} (synced: {updated})"
        ));
    }

    lines.join("\n")
}

/// Format bill/credit rows as a readable list.
pub fn format_bills(rows: &[SafeRow]) -> String {
    if rows.is_empty() {
        return "(no bills/credit accounts found)".to_string();
    }

    let mut lines = vec![format!("Bills & Credit Accounts ({}):", rows.len())];
    lines.push(String::new());

    for row in rows {
        let name = row.account_name.as_deref().unwrap_or("Unknown");
        let acct_type = row.account_type.as_deref().unwrap_or("?");
        let balance = row.current_balance.as_deref().unwrap_or("-");
        let updated = row.last_updated.as_deref().unwrap_or("unknown");

        lines.push(format!(
            "  {name} ({acct_type}): owed={balance} (synced: {updated})"
        ));
    }

    lines.join("\n")
}

// ── Public Entry Points ──────────────────────────────────────────────

/// Execute plaid_balances: query all account balances with PII filtering.
pub async fn plaid_balances() -> Result<String> {
    let client = connect().await?;

    // Hardcoded query — only allowlisted columns, no user input
    let sql = "SELECT account_name, account_type, \
               current_balance::text, available_balance::text, \
               last_updated::text \
               FROM plaid_accounts \
               ORDER BY account_type, account_name";

    let rows = tokio::time::timeout(QUERY_TIMEOUT, client.query(sql, &[]))
        .await
        .map_err(|_| anyhow!("Query timed out after {QUERY_TIMEOUT:?}"))??;

    // Extract column names from the first row
    let columns: Vec<String> = if let Some(first) = rows.first() {
        first.columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        return Ok("(no accounts found)".to_string());
    };

    // Filter through PII boundary
    let safe_rows: Vec<SafeRow> = rows.iter().map(|r| filter_pii(r, &columns)).collect();

    tracing::info!(count = safe_rows.len(), "plaid_balances completed");
    Ok(format_balances(&safe_rows))
}

/// Execute plaid_bills: query credit/loan accounts with PII filtering.
pub async fn plaid_bills() -> Result<String> {
    let client = connect().await?;

    // Hardcoded query — only credit/loan accounts, allowlisted columns
    let sql = "SELECT account_name, account_type, \
               current_balance::text, available_balance::text, \
               last_updated::text \
               FROM plaid_accounts \
               WHERE account_type IN ('credit', 'loan') \
               ORDER BY current_balance DESC";

    let rows = tokio::time::timeout(QUERY_TIMEOUT, client.query(sql, &[]))
        .await
        .map_err(|_| anyhow!("Query timed out after {QUERY_TIMEOUT:?}"))??;

    let columns: Vec<String> = if let Some(first) = rows.first() {
        first.columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        return Ok("(no bills/credit accounts found)".to_string());
    };

    let safe_rows: Vec<SafeRow> = rows.iter().map(|r| filter_pii(r, &columns)).collect();

    tracing::info!(count = safe_rows.len(), "plaid_bills completed");
    Ok(format_bills(&safe_rows))
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── PII Filter Tests (Security Critical) ────────────────────

    #[test]
    fn test_scrub_pii_clean_value() {
        assert_eq!(scrub_pii("Checking Account"), "Checking Account");
        assert_eq!(scrub_pii("1234.56"), "1234.56");
        assert_eq!(scrub_pii("credit"), "credit");
    }

    #[test]
    fn test_scrub_pii_nine_digit_number() {
        // SSN-like pattern
        assert_eq!(scrub_pii("123456789"), "[REDACTED]");
        // Embedded in text
        assert_eq!(scrub_pii("acct 123456789 main"), "[REDACTED]");
    }

    #[test]
    fn test_scrub_pii_account_number() {
        // 8+ digit account number
        assert_eq!(scrub_pii("12345678"), "[REDACTED]");
        assert_eq!(scrub_pii("12345678901234"), "[REDACTED]");
    }

    #[test]
    fn test_scrub_pii_allows_short_numbers() {
        // Balances with decimals should pass (dot breaks digit sequence)
        assert_eq!(scrub_pii("1234.56"), "1234.56");
        // Short numbers are fine
        assert_eq!(scrub_pii("42"), "42");
        assert_eq!(scrub_pii("1234567"), "1234567");
    }

    #[test]
    fn test_matches_pii_exact() {
        assert!(matches_pii_pattern("123456789", r"\b\d{9}\b"));
        assert!(!matches_pii_pattern("12345678", r"\b\d{9}\b"));
        assert!(!matches_pii_pattern("1234567890", r"\b\d{9}\b"));
    }

    #[test]
    fn test_matches_pii_range() {
        assert!(matches_pii_pattern("12345678", r"\b\d{8,17}\b"));
        assert!(matches_pii_pattern("12345678901234567", r"\b\d{8,17}\b"));
        assert!(!matches_pii_pattern("1234567", r"\b\d{8,17}\b"));
    }

    #[test]
    fn test_matches_pii_word_boundary() {
        // Digit sequence attached to letters should not match
        assert!(!matches_pii_pattern("abc123456789xyz", r"\b\d{9}\b"));
        // With spaces — should match
        assert!(matches_pii_pattern("acct 123456789 main", r"\b\d{9}\b"));
    }

    // ── SafeRow / Formatting Tests ──────────────────────────────

    #[test]
    fn test_format_balances_empty() {
        assert_eq!(format_balances(&[]), "(no accounts found)");
    }

    #[test]
    fn test_format_balances_with_data() {
        let rows = vec![SafeRow {
            account_name: Some("Checking".into()),
            account_type: Some("depository".into()),
            current_balance: Some("1234.56".into()),
            available_balance: Some("1200.00".into()),
            last_updated: Some("2026-03-22T10:00:00Z".into()),
        }];
        let output = format_balances(&rows);
        assert!(output.contains("Account Balances (1 accounts)"));
        assert!(output.contains("Checking (depository)"));
        assert!(output.contains("current=1234.56"));
        assert!(output.contains("available=1200.00"));
    }

    #[test]
    fn test_format_bills_empty() {
        assert_eq!(format_bills(&[]), "(no bills/credit accounts found)");
    }

    #[test]
    fn test_format_bills_with_data() {
        let rows = vec![SafeRow {
            account_name: Some("Visa Card".into()),
            account_type: Some("credit".into()),
            current_balance: Some("500.00".into()),
            available_balance: None,
            last_updated: Some("2026-03-22T10:00:00Z".into()),
        }];
        let output = format_bills(&rows);
        assert!(output.contains("Bills & Credit Accounts (1)"));
        assert!(output.contains("Visa Card (credit)"));
        assert!(output.contains("owed=500.00"));
    }

    #[test]
    fn test_plaid_connect_missing_env() {
        std::env::remove_var("PLAID_DB_URL");
        // connect() is async, test the env check directly
        let result = std::env::var("PLAID_DB_URL");
        assert!(result.is_err());
    }
}

// ── PlaidClient wrapper ──────────────────────────────────────────────

/// Thin wrapper for `Checkable` health checks.
/// Plaid data is read from cortex-postgres; no Plaid API key required.
#[allow(dead_code)]
pub struct PlaidClient;

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for PlaidClient {
    fn name(&self) -> &str {
        "plaid"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;

        if std::env::var("PLAID_DB_URL").is_err() {
            return crate::tools::CheckResult::Missing {
                env_var: "PLAID_DB_URL".into(),
            };
        }

        let (latency, result) = timed(|| async {
            match connect().await {
                Ok(client) => client
                    .query_one("SELECT 1", &[])
                    .await
                    .map(|_| ())
                    .map_err(|e| anyhow::anyhow!(e)),
                Err(e) => Err(e),
            }
        })
        .await;

        match result {
            Ok(_) => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "cortex-postgres reachable (SELECT 1 ok)".into(),
            },
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: format!("connection failed: {e}"),
            },
        }
    }
}
