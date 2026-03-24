//! Stripe payment data tools via REST API (api.stripe.com).
//!
//! Two read-only tools:
//! * `stripe_customers(query)` — search customers by email, name, or metadata.
//! * `stripe_invoices(status)` — list invoices filtered by status.
//!
//! Auth: Bearer token via `STRIPE_SECRET_KEY` env var.
//! API version pinned to `2024-12-18.acacia`.

use std::time::Duration;

use anyhow::{bail, Result};
use serde::Deserialize;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const STRIPE_BASE_URL: &str = "https://api.stripe.com/v1";
const STRIPE_API_VERSION: &str = "2024-12-18.acacia";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Customer {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub created: i64,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomerSearchResponse {
    pub data: Vec<Customer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub customer_email: Option<String>,
    pub amount_due: i64,
    pub currency: Option<String>,
    pub status: Option<String>,
    pub due_date: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvoiceListResponse {
    pub data: Vec<Invoice>,
}

// ── Client ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct StripeClient {
    http: reqwest::Client,
}

impl StripeClient {
    /// Create a new `StripeClient` from the `STRIPE_SECRET_KEY` environment variable.
    ///
    /// Returns an error if the env var is not set.
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("STRIPE_SECRET_KEY")
            .map_err(|_| anyhow::anyhow!("STRIPE_SECRET_KEY env var not set"))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}")
                .parse()
                .expect("valid auth header"),
        );
        headers.insert(
            "Stripe-Version",
            STRIPE_API_VERSION
                .parse()
                .expect("valid stripe version header"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build Stripe HTTP client");

        Ok(Self { http })
    }

    /// Create a `StripeClient` with a custom HTTP client (for testing with mock servers).
    #[cfg(test)]
    pub fn with_http_client(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// Search customers via the Stripe Search API.
    pub async fn search_customers(&self, query: &str) -> Result<Vec<Customer>> {
        let url = format!("{STRIPE_BASE_URL}/customers/search");

        let resp = self
            .http
            .get(&url)
            .query(&[("query", query), ("limit", "10")])
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    anyhow::anyhow!(
                        "Stripe API request timed out after {}s",
                        REQUEST_TIMEOUT.as_secs()
                    )
                } else {
                    anyhow::anyhow!("Stripe API request failed: {e}")
                }
            })?;

        map_stripe_error(resp.status())?;

        let text = resp.text().await?;
        let parsed: CustomerSearchResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("failed to parse Stripe customers JSON: {e}"))?;
        Ok(parsed.data)
    }

    /// List invoices filtered by status.
    pub async fn list_invoices(&self, status: &str) -> Result<Vec<Invoice>> {
        let url = format!("{STRIPE_BASE_URL}/invoices");

        let resp = self
            .http
            .get(&url)
            .query(&[("status", status), ("limit", "20")])
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    anyhow::anyhow!(
                        "Stripe API request timed out after {}s",
                        REQUEST_TIMEOUT.as_secs()
                    )
                } else {
                    anyhow::anyhow!("Stripe API request failed: {e}")
                }
            })?;

        map_stripe_error(resp.status())?;

        let text = resp.text().await?;
        let parsed: InvoiceListResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("failed to parse Stripe invoices JSON: {e}"))?;
        Ok(parsed.data)
    }
}

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for StripeClient {
    fn name(&self) -> &str {
        "stripe"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http
                .get(format!("{STRIPE_BASE_URL}/balance"))
                .send()
                .await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "balance endpoint reachable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => crate::tools::CheckResult::Unhealthy {
                error: "invalid API key (401) — check STRIPE_SECRET_KEY".into(),
            },
            Ok(resp) => crate::tools::CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }

    async fn check_write(&self) -> Option<crate::tools::CheckResult> {
        use crate::tools::check::timed;
        // POST /v1/invoices with no body — expect 400 (missing required fields), not 2xx
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http
                .post(format!("{STRIPE_BASE_URL}/invoices"))
                .send()
                .await
        })
        .await;
        let result = match result {
            // 400 means the endpoint is reachable and auth is valid — write permissions confirmed
            Ok(resp) if resp.status().as_u16() == 400 => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "invoices endpoint writable (400 as expected)".into(),
            },
            Ok(resp) if resp.status().is_success() => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "invoices endpoint writable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => crate::tools::CheckResult::Unhealthy {
                error: "write probe: invalid API key (401)".into(),
            },
            Ok(resp) => crate::tools::CheckResult::Unhealthy {
                error: format!("write probe: HTTP {}", resp.status()),
            },
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: format!("write probe: {e}"),
            },
        };
        Some(result)
    }
}

// ── Error Mapping ────────────────────────────────────────────────────

fn map_stripe_error(status: reqwest::StatusCode) -> Result<()> {
    match status.as_u16() {
        200..=299 => Ok(()),
        401 => bail!("Stripe key invalid — check STRIPE_SECRET_KEY"),
        403 => bail!("Stripe key lacks permission — use a key with read-only scope"),
        429 => bail!("Stripe API rate limited — try again later"),
        status => bail!("Stripe API returned HTTP {status}"),
    }
}

// ── Validation ───────────────────────────────────────────────────────

/// Valid Stripe invoice statuses.
const VALID_INVOICE_STATUSES: &[&str] = &["draft", "open", "paid", "uncollectible", "void"];

fn validate_invoice_status(status: &str) -> Result<()> {
    if VALID_INVOICE_STATUSES.contains(&status) {
        Ok(())
    } else {
        bail!(
            "invalid invoice status: '{status}' — must be one of: {}",
            VALID_INVOICE_STATUSES.join(", ")
        )
    }
}

// ── Currency Formatting ─────────────────────────────────────────────

/// Convert cents to a display string (e.g., 4500 USD -> "$45.00").
pub fn format_currency(amount_cents: i64, currency: &str) -> String {
    let major = amount_cents.abs() / 100;
    let minor = amount_cents.abs() % 100;
    let sign = if amount_cents < 0 { "-" } else { "" };

    let symbol = match currency.to_lowercase().as_str() {
        "usd" => "$",
        "eur" => "\u{20ac}",
        "gbp" => "\u{00a3}",
        "jpy" => "\u{00a5}",
        "cad" | "aud" | "nzd" | "sgd" | "hkd" => "$",
        _ => "",
    };

    if symbol.is_empty() {
        format!("{sign}{major}.{minor:02} {}", currency.to_uppercase())
    } else {
        format!("{sign}{symbol}{major}.{minor:02}")
    }
}

// ── Telegram Formatters ──────────────────────────────────────────────

impl Customer {
    pub fn format_for_telegram(&self) -> String {
        let email = self.email.as_deref().unwrap_or("no email");
        let name = self.name.as_deref().unwrap_or("unnamed");
        let currency = self
            .currency
            .as_deref()
            .unwrap_or("usd")
            .to_uppercase();
        let created = format_unix_date(self.created);
        format!(
            "💰 **{name}** — {email}\n   {} · {currency} · since {created}",
            self.id
        )
    }
}

impl Invoice {
    pub fn format_for_telegram(&self) -> String {
        let email = self.customer_email.as_deref().unwrap_or("unknown");
        let currency = self.currency.as_deref().unwrap_or("usd");
        let amount = format_currency(self.amount_due, currency);
        let status = self.status.as_deref().unwrap_or("unknown");
        let icon = invoice_status_icon(status);
        let due = self
            .due_date
            .map(format_unix_date)
            .unwrap_or_else(|| "no due date".into());
        let desc = self
            .description
            .as_deref()
            .unwrap_or("");

        let desc_part = if desc.is_empty() {
            String::new()
        } else {
            format!(" — {desc}")
        };

        format!(
            "💰 {icon} **{amount}** [{status}]{desc_part}\n   {email} · due {due} · {}",
            self.id
        )
    }
}

fn invoice_status_icon(status: &str) -> &'static str {
    match status {
        "paid" => "\u{2705}",          // green check
        "open" => "\u{1f4e8}",         // envelope
        "draft" => "\u{270f}",         // pencil
        "uncollectible" => "\u{274c}", // red X
        "void" => "\u{26d4}",          // no entry
        _ => "\u{2753}",               // question mark
    }
}

/// Format a Unix timestamp (seconds) as YYYY-MM-DD.
fn format_unix_date(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".into())
}

// ── Public Tool Handlers ─────────────────────────────────────────────

/// Search Stripe customers by query string.
pub async fn stripe_customers(query: &str) -> Result<String> {
    if query.is_empty() {
        bail!("search query cannot be empty");
    }
    let client = StripeClient::from_env()?;
    let customers = client.search_customers(query).await?;

    if customers.is_empty() {
        return Ok(format!("No customers found for query: {query}"));
    }

    let mut lines = vec![format!("{} customer(s) found:", customers.len())];
    for c in &customers {
        lines.push(c.format_for_telegram());
    }
    Ok(lines.join("\n"))
}

/// List Stripe invoices by status.
pub async fn stripe_invoices(status: &str) -> Result<String> {
    let status = if status.is_empty() { "open" } else { status };
    validate_invoice_status(status)?;

    let client = StripeClient::from_env()?;
    let invoices = client.list_invoices(status).await?;

    if invoices.is_empty() {
        return Ok(format!("No {status} invoices."));
    }

    let total_cents: i64 = invoices.iter().map(|i| i.amount_due).sum();
    // Use currency from first invoice for the total
    let total_currency = invoices
        .first()
        .and_then(|i| i.currency.as_deref())
        .unwrap_or("usd");
    let total = format_currency(total_cents, total_currency);

    let mut lines = vec![format!(
        "{} {status} invoice(s) — total: {total}",
        invoices.len()
    )];
    for inv in &invoices {
        lines.push(inv.format_for_telegram());
    }
    Ok(lines.join("\n"))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all Stripe tools.
pub fn stripe_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "stripe_customers".into(),
            description: "Search Stripe customers by email, name, or metadata. Uses the Stripe Search API query syntax (e.g., email:\"john@example.com\", name:\"John\").".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Stripe Search API query (e.g., email:\"john@example.com\", name:\"John\", metadata[\"key\"]:\"value\")"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "stripe_invoices".into(),
            description: "List Stripe invoices filtered by status. Returns invoice amounts (formatted), customer email, due dates, and status. Shows total at bottom. Valid statuses: draft, open, paid, uncollectible, void.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "Invoice status filter: draft, open, paid, uncollectible, void (default: open)"
                    }
                },
                "required": ["status"]
            }),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Currency Formatting Tests ────────────────────────────────

    #[test]
    fn format_currency_usd() {
        assert_eq!(format_currency(4500, "usd"), "$45.00");
    }

    #[test]
    fn format_currency_zero() {
        assert_eq!(format_currency(0, "usd"), "$0.00");
    }

    #[test]
    fn format_currency_cents_only() {
        assert_eq!(format_currency(99, "usd"), "$0.99");
    }

    #[test]
    fn format_currency_large_amount() {
        assert_eq!(format_currency(1_000_000, "usd"), "$10000.00");
    }

    #[test]
    fn format_currency_negative() {
        assert_eq!(format_currency(-2500, "usd"), "-$25.00");
    }

    #[test]
    fn format_currency_eur() {
        assert_eq!(format_currency(1999, "eur"), "\u{20ac}19.99");
    }

    #[test]
    fn format_currency_gbp() {
        assert_eq!(format_currency(5000, "gbp"), "\u{00a3}50.00");
    }

    #[test]
    fn format_currency_jpy() {
        assert_eq!(format_currency(15000, "jpy"), "\u{00a5}150.00");
    }

    #[test]
    fn format_currency_unknown() {
        assert_eq!(format_currency(1234, "brl"), "12.34 BRL");
    }

    // ── Validation Tests ─────────────────────────────────────────

    #[test]
    fn validate_invoice_status_valid() {
        for status in VALID_INVOICE_STATUSES {
            assert!(validate_invoice_status(status).is_ok());
        }
    }

    #[test]
    fn validate_invoice_status_invalid() {
        assert!(validate_invoice_status("").is_err());
        assert!(validate_invoice_status("pending").is_err());
        assert!(validate_invoice_status("OPEN").is_err());
    }

    // ── Parse Tests ──────────────────────────────────────────────

    #[test]
    fn parse_customer_search_response() {
        let json = r#"{
            "data": [{
                "id": "cus_abc123",
                "email": "john@example.com",
                "name": "John Doe",
                "created": 1700000000,
                "currency": "usd"
            }]
        }"#;
        let resp: CustomerSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "cus_abc123");
        assert_eq!(resp.data[0].email.as_deref(), Some("john@example.com"));
        assert_eq!(resp.data[0].name.as_deref(), Some("John Doe"));
    }

    #[test]
    fn parse_customer_search_empty() {
        let json = r#"{"data": []}"#;
        let resp: CustomerSearchResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    #[test]
    fn parse_invoice_list_response() {
        let json = r#"{
            "data": [{
                "id": "in_abc123",
                "customer_email": "john@example.com",
                "amount_due": 4500,
                "currency": "usd",
                "status": "open",
                "due_date": 1711100000,
                "description": "March subscription"
            }]
        }"#;
        let resp: InvoiceListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "in_abc123");
        assert_eq!(resp.data[0].amount_due, 4500);
        assert_eq!(resp.data[0].status.as_deref(), Some("open"));
    }

    #[test]
    fn parse_invoice_list_empty() {
        let json = r#"{"data": []}"#;
        let resp: InvoiceListResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    #[test]
    fn parse_customer_malformed_json() {
        let result: Result<CustomerSearchResponse, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invoice_malformed_json() {
        let result: Result<InvoiceListResponse, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    // ── Telegram Formatter Tests ─────────────────────────────────

    #[test]
    fn format_customer_for_telegram() {
        let c = Customer {
            id: "cus_abc123".into(),
            email: Some("john@example.com".into()),
            name: Some("John Doe".into()),
            created: 1700000000,
            currency: Some("usd".into()),
        };
        let formatted = c.format_for_telegram();
        assert!(formatted.contains("💰"));
        assert!(formatted.contains("John Doe"));
        assert!(formatted.contains("john@example.com"));
        assert!(formatted.contains("cus_abc123"));
        assert!(formatted.contains("USD"));
    }

    #[test]
    fn format_customer_missing_fields() {
        let c = Customer {
            id: "cus_xyz".into(),
            email: None,
            name: None,
            created: 0,
            currency: None,
        };
        let formatted = c.format_for_telegram();
        assert!(formatted.contains("unnamed"));
        assert!(formatted.contains("no email"));
    }

    #[test]
    fn format_invoice_for_telegram() {
        let inv = Invoice {
            id: "in_abc123".into(),
            customer_email: Some("john@example.com".into()),
            amount_due: 4500,
            currency: Some("usd".into()),
            status: Some("open".into()),
            due_date: Some(1711100000),
            description: Some("March subscription".into()),
        };
        let formatted = inv.format_for_telegram();
        assert!(formatted.contains("💰"));
        assert!(formatted.contains("$45.00"));
        assert!(formatted.contains("[open]"));
        assert!(formatted.contains("john@example.com"));
        assert!(formatted.contains("in_abc123"));
        assert!(formatted.contains("March subscription"));
        assert!(formatted.contains("\u{1f4e8}")); // envelope icon
    }

    #[test]
    fn format_invoice_paid() {
        let inv = Invoice {
            id: "in_paid".into(),
            customer_email: Some("test@example.com".into()),
            amount_due: 1000,
            currency: Some("usd".into()),
            status: Some("paid".into()),
            due_date: None,
            description: None,
        };
        let formatted = inv.format_for_telegram();
        assert!(formatted.contains("💰"));
        assert!(formatted.contains("\u{2705}")); // green check for paid
        assert!(formatted.contains("$10.00"));
        assert!(formatted.contains("no due date"));
    }

    // ── Invoice Status Icon Tests ────────────────────────────────

    #[test]
    fn invoice_status_icons() {
        assert_eq!(invoice_status_icon("paid"), "\u{2705}");
        assert_eq!(invoice_status_icon("open"), "\u{1f4e8}");
        assert_eq!(invoice_status_icon("draft"), "\u{270f}");
        assert_eq!(invoice_status_icon("uncollectible"), "\u{274c}");
        assert_eq!(invoice_status_icon("void"), "\u{26d4}");
        assert_eq!(invoice_status_icon("unknown"), "\u{2753}");
    }

    // ── Tool Definition Tests ────────────────────────────────────

    #[test]
    fn stripe_tool_definitions_returns_two_tools() {
        let tools = stripe_tool_definitions();
        assert_eq!(tools.len(), 2);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"stripe_customers"));
        assert!(names.contains(&"stripe_invoices"));
    }

    #[test]
    fn tool_definitions_have_correct_schema() {
        let tools = stripe_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn stripe_customers_schema_requires_query() {
        let tools = stripe_tool_definitions();
        let sc = tools
            .iter()
            .find(|t| t.name == "stripe_customers")
            .unwrap();
        let required = sc.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("query")));
    }

    #[test]
    fn stripe_invoices_schema_requires_status() {
        let tools = stripe_tool_definitions();
        let si = tools
            .iter()
            .find(|t| t.name == "stripe_invoices")
            .unwrap();
        let required = si.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("status")));
    }

    // ── Format Unix Date Tests ───────────────────────────────────

    #[test]
    fn format_unix_date_valid() {
        assert_eq!(format_unix_date(1700000000), "2023-11-14");
    }

    #[test]
    fn format_unix_date_zero() {
        assert_eq!(format_unix_date(0), "1970-01-01");
    }

    // ── Client Env Var Tests ─────────────────────────────────────

    #[test]
    fn client_from_env_fails_without_key() {
        let saved = std::env::var("STRIPE_SECRET_KEY").ok();
        std::env::remove_var("STRIPE_SECRET_KEY");
        let result = StripeClient::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("STRIPE_SECRET_KEY env var not set"));
        if let Some(val) = saved {
            std::env::set_var("STRIPE_SECRET_KEY", val);
        }
    }
}
