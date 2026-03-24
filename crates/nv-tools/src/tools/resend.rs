//! Resend email delivery tools via REST API (api.resend.com).
//!
//! Two read-only tools:
//! * `resend_emails(status)` — list recent emails, optionally filtered by delivery status.
//! * `resend_bounces()` — list emails that bounced.
//!
//! Auth: Bearer token via `RESEND_API_KEY` env var.

use std::time::Duration;

use anyhow::{bail, Result};
use serde::Deserialize;

use nv_core::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const RESEND_BASE_URL: &str = "https://api.resend.com";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_EMAILS: usize = 20;

// ── Types ────────────────────────────────────────────────────────────

/// A single email from the Resend API list response.
#[derive(Debug, Clone, Deserialize)]
pub struct ResendEmail {
    #[allow(dead_code)]
    pub id: String,
    pub to: Vec<String>,
    pub subject: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    pub created_at: String,
}

/// Envelope returned by `GET /emails`.
#[derive(Debug, Deserialize)]
pub struct EmailsResponse {
    pub data: Vec<ResendEmail>,
}

// ── Client ───────────────────────────────────────────────────────────

/// HTTP client for the Resend REST API.
#[derive(Debug)]
pub struct ResendClient {
    pub http: reqwest::Client,
}

impl ResendClient {
    /// Create a new `ResendClient` from the `RESEND_API_KEY` environment variable.
    ///
    /// Returns an error if the env var is not set.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("RESEND_API_KEY")
            .map_err(|_| anyhow::anyhow!("Resend not configured — RESEND_API_KEY env var not set"))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {api_key}")
                .parse()
                .expect("valid auth header"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build Resend HTTP client");

        Ok(Self { http })
    }

    /// List recent emails, optionally filtered by delivery status.
    pub async fn list_emails(&self, status: Option<&str>) -> Result<Vec<ResendEmail>> {
        let url = format!("{RESEND_BASE_URL}/emails");

        let resp = self.http.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                anyhow::anyhow!(
                    "Resend API request timed out after {}s",
                    REQUEST_TIMEOUT.as_secs()
                )
            } else {
                anyhow::anyhow!("Resend API request failed: {e}")
            }
        })?;

        map_resend_error(resp.status())?;

        let text = resp.text().await?;
        let envelope: EmailsResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("failed to parse Resend emails JSON: {e}"))?;

        let mut emails = envelope.data;

        // Filter by status if provided
        if let Some(filter_status) = status {
            emails.retain(|e| {
                e.status
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case(filter_status))
                    .unwrap_or(false)
            });
        }

        // Cap at MAX_EMAILS
        emails.truncate(MAX_EMAILS);

        Ok(emails)
    }

    /// List emails that bounced (convenience wrapper).
    pub async fn list_bounces(&self) -> Result<Vec<ResendEmail>> {
        self.list_emails(Some("bounced")).await
    }
}

// ── Error Mapping ────────────────────────────────────────────────────

fn map_resend_error(status: reqwest::StatusCode) -> Result<()> {
    match status.as_u16() {
        200..=299 => Ok(()),
        401 => bail!("Resend API key expired or invalid — check RESEND_API_KEY"),
        403 => bail!("Resend API key lacks required permissions"),
        429 => bail!("Resend API rate limit exceeded — try again later"),
        status => bail!("Resend API returned HTTP {status}"),
    }
}

// ── Formatters ───────────────────────────────────────────────────────

/// Format a list of emails for Telegram output.
pub fn format_emails(emails: &[ResendEmail]) -> String {
    if emails.is_empty() {
        return "No emails found.".to_string();
    }

    let mut lines = Vec::with_capacity(emails.len());
    for email in emails {
        let to = email.to.join(", ");
        let subject = email.subject.as_deref().unwrap_or("(no subject)");
        let status = email.status.as_deref().unwrap_or("unknown");
        let icon = status_icon(status);
        let ts = short_timestamp(&email.created_at);
        lines.push(format!(
            "📧 {icon} **{subject}** [{status}] — {to}\n   {ts}"
        ));
    }
    lines.join("\n")
}

/// Format bounced emails for Telegram output.
pub fn format_bounces(emails: &[ResendEmail]) -> String {
    if emails.is_empty() {
        return "No bounces found.".to_string();
    }

    let mut lines = Vec::with_capacity(emails.len());
    for email in emails {
        let to = email.to.join(", ");
        let subject = email.subject.as_deref().unwrap_or("(no subject)");
        let ts = short_timestamp(&email.created_at);
        lines.push(format!(
            "📧 ❌ **{subject}** [bounced] — {to}\n   {ts}"
        ));
    }
    lines.join("\n")
}

fn status_icon(status: &str) -> &'static str {
    match status {
        "delivered" => "\u{2705}",   // green check
        "bounced" => "\u{274c}",     // red X
        "complained" => "\u{26a0}",  // warning
        "sent" => "\u{1f4e8}",      // envelope
        _ => "\u{2753}",            // question mark
    }
}

/// Shorten an ISO timestamp to date only (YYYY-MM-DD).
fn short_timestamp(ts: &str) -> &str {
    if ts.len() >= 10 {
        &ts[..10]
    } else {
        ts
    }
}

// ── Public Tool Handlers ─────────────────────────────────────────────

/// List recent emails, optionally filtered by status.
pub async fn resend_emails(status: Option<&str>) -> Result<String> {
    let client = ResendClient::from_env()?;
    let emails = client.list_emails(status).await?;
    Ok(format_emails(&emails))
}

/// List emails that bounced.
pub async fn resend_bounces() -> Result<String> {
    let client = ResendClient::from_env()?;
    let emails = client.list_bounces().await?;
    Ok(format_bounces(&emails))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all Resend tools.
pub fn resend_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "resend_emails".into(),
            description: "List recent emails from Resend. Optionally filter by delivery status (delivered, bounced, complained). Returns to address, subject, status, and timestamp.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "Optional delivery status filter: 'delivered', 'bounced', 'complained', or omit for all"
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "resend_bounces".into(),
            description: "List emails that bounced from Resend. Returns to address, subject, and timestamp for all bounced emails.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parse Tests ──────────────────────────────────────────────

    #[test]
    fn parse_emails_response() {
        let json = r#"{
            "data": [
                {
                    "id": "em_123",
                    "to": ["user@example.com"],
                    "subject": "Welcome!",
                    "status": "delivered",
                    "created_at": "2026-03-22T10:00:00Z"
                },
                {
                    "id": "em_456",
                    "to": ["bad@example.com"],
                    "subject": "Reset password",
                    "status": "bounced",
                    "created_at": "2026-03-22T11:00:00Z"
                }
            ]
        }"#;
        let resp: EmailsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].id, "em_123");
        assert_eq!(resp.data[0].to, vec!["user@example.com"]);
        assert_eq!(resp.data[0].subject.as_deref(), Some("Welcome!"));
        assert_eq!(resp.data[0].status.as_deref(), Some("delivered"));
        assert_eq!(resp.data[1].status.as_deref(), Some("bounced"));
    }

    #[test]
    fn parse_emails_empty() {
        let json = r#"{"data": []}"#;
        let resp: EmailsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    #[test]
    fn parse_emails_missing_optional_fields() {
        let json = r#"{
            "data": [{
                "id": "em_789",
                "to": ["test@example.com"],
                "subject": null,
                "created_at": "2026-03-22T12:00:00Z"
            }]
        }"#;
        let resp: EmailsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert!(resp.data[0].subject.is_none());
        assert!(resp.data[0].status.is_none());
    }

    // ── Formatter Tests ─────────────────────────────────────────

    #[test]
    fn format_emails_empty() {
        let output = format_emails(&[]);
        assert_eq!(output, "No emails found.");
    }

    #[test]
    fn format_emails_with_data() {
        let emails = vec![
            ResendEmail {
                id: "em_1".into(),
                to: vec!["user@example.com".into()],
                subject: Some("Hello".into()),
                status: Some("delivered".into()),
                created_at: "2026-03-22T10:00:00Z".into(),
            },
            ResendEmail {
                id: "em_2".into(),
                to: vec!["bad@example.com".into()],
                subject: Some("Test".into()),
                status: Some("bounced".into()),
                created_at: "2026-03-22T11:00:00Z".into(),
            },
        ];
        let output = format_emails(&emails);
        assert!(output.contains("📧"));
        assert!(output.contains("user@example.com"));
        assert!(output.contains("**Hello**"));
        assert!(output.contains("[delivered]"));
        assert!(output.contains("\u{2705}")); // delivered icon
        assert!(output.contains("\u{274c}")); // bounced icon
    }

    #[test]
    fn format_bounces_empty() {
        let output = format_bounces(&[]);
        assert_eq!(output, "No bounces found.");
    }

    #[test]
    fn format_bounces_with_data() {
        let emails = vec![ResendEmail {
            id: "em_3".into(),
            to: vec!["bounce@example.com".into()],
            subject: Some("Oops".into()),
            status: Some("bounced".into()),
            created_at: "2026-03-22T12:00:00Z".into(),
        }];
        let output = format_bounces(&emails);
        assert!(output.contains("📧"));
        assert!(output.contains("bounce@example.com"));
        assert!(output.contains("**Oops**"));
        assert!(output.contains("[bounced]"));
        assert!(output.contains("\u{274c}"));
    }

    // ── Status Icon Tests ───────────────────────────────────────

    #[test]
    fn status_icons() {
        assert_eq!(status_icon("delivered"), "\u{2705}");
        assert_eq!(status_icon("bounced"), "\u{274c}");
        assert_eq!(status_icon("complained"), "\u{26a0}");
        assert_eq!(status_icon("sent"), "\u{1f4e8}");
        assert_eq!(status_icon("unknown"), "\u{2753}");
    }

    // ── Short Timestamp Tests ───────────────────────────────────

    #[test]
    fn short_timestamp_full_iso() {
        assert_eq!(short_timestamp("2026-03-22T15:30:00Z"), "2026-03-22");
    }

    #[test]
    fn short_timestamp_already_short() {
        assert_eq!(short_timestamp("2026"), "2026");
    }

    // ── Tool Definition Tests ───────────────────────────────────

    #[test]
    fn resend_tool_definitions_returns_two_tools() {
        let tools = resend_tool_definitions();
        assert_eq!(tools.len(), 2);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"resend_emails"));
        assert!(names.contains(&"resend_bounces"));
    }

    #[test]
    fn tool_definitions_have_correct_schema() {
        let tools = resend_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
        }
    }

    // ── Client env Tests ────────────────────────────────────────

    #[test]
    fn client_from_env_fails_without_key() {
        let saved = std::env::var("RESEND_API_KEY").ok();
        std::env::remove_var("RESEND_API_KEY");
        let result = ResendClient::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("RESEND_API_KEY"));
        if let Some(val) = saved {
            std::env::set_var("RESEND_API_KEY", val);
        }
    }
}
