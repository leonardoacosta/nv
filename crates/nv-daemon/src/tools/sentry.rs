//! Sentry error tracking tools via REST API (sentry.io).
//!
//! Two read-only tools:
//! * `sentry_issues(project)` — list unresolved issues with count, title, level.
//! * `sentry_issue(id)` — get issue details with stack trace (top 5 frames).
//!
//! Auth: Bearer token via `SENTRY_AUTH_TOKEN` env var.
//! Organization: `SENTRY_ORG` env var.

use std::time::Duration;

use anyhow::{bail, Result};
use serde::Deserialize;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const SENTRY_BASE_URL: &str = "https://sentry.io/api/0";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_STACK_FRAMES: usize = 5;

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentryIssueSummary {
    pub id: String,
    pub title: String,
    pub culprit: Option<String>,
    pub count: String,
    #[allow(dead_code)]
    pub first_seen: String,
    #[allow(dead_code)]
    pub last_seen: String,
    pub level: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentryIssueDetail {
    pub id: String,
    pub title: String,
    pub culprit: Option<String>,
    pub count: String,
    pub first_seen: String,
    pub last_seen: String,
    pub level: String,
    pub status: String,
}

/// Represents a single stack frame from a Sentry event.
#[derive(Debug, Clone, Deserialize)]
pub struct StackFrame {
    pub filename: Option<String>,
    pub function: Option<String>,
    #[serde(rename = "lineNo")]
    pub line_no: Option<u64>,
    #[serde(rename = "colNo")]
    #[allow(dead_code)]
    pub col_no: Option<u64>,
    pub module: Option<String>,
}

/// A Sentry event's exception entry (used to extract stack traces).
#[derive(Debug, Clone, Deserialize)]
pub struct ExceptionEntry {
    #[serde(rename = "type")]
    pub exception_type: Option<String>,
    pub value: Option<String>,
    pub stacktrace: Option<Stacktrace>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Stacktrace {
    pub frames: Vec<StackFrame>,
}

/// Top-level wrapper for the latest event response.
#[derive(Debug, Clone, Deserialize)]
pub struct SentryEvent {
    pub entries: Option<Vec<EventEntry>>,
}

/// An entry inside a Sentry event (we only care about "exception" type).
#[derive(Debug, Clone, Deserialize)]
pub struct EventEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub data: Option<EventEntryData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventEntryData {
    pub values: Option<Vec<ExceptionEntry>>,
}

// ── Client ───────────────────────────────────────────────────────────

pub struct SentryClient {
    http: reqwest::Client,
    org: String,
}

impl SentryClient {
    /// Create a new `SentryClient` from environment variables.
    ///
    /// Returns an error if `SENTRY_AUTH_TOKEN` or `SENTRY_ORG` is not set.
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("SENTRY_AUTH_TOKEN")
            .map_err(|_| anyhow::anyhow!("SENTRY_AUTH_TOKEN env var not set"))?;
        let org = std::env::var("SENTRY_ORG")
            .map_err(|_| anyhow::anyhow!("SENTRY_ORG env var not set"))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}")
                .parse()
                .expect("valid auth header"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build Sentry HTTP client");

        Ok(Self { http, org })
    }

    /// Create a `SentryClient` with a custom HTTP client (for testing with mock servers).
    #[cfg(test)]
    pub fn with_http_client(http: reqwest::Client, org: &str) -> Self {
        Self {
            http,
            org: org.to_string(),
        }
    }

    /// List unresolved issues for a Sentry project.
    pub async fn list_issues(&self, project: &str) -> Result<Vec<SentryIssueSummary>> {
        let url = format!(
            "{}/projects/{}/{}/issues/?query=is:unresolved&sort=date&limit=10",
            SENTRY_BASE_URL, self.org, project
        );

        let resp = self.http.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                anyhow::anyhow!("Sentry API request timed out after {}s", REQUEST_TIMEOUT.as_secs())
            } else {
                anyhow::anyhow!("Sentry API request failed: {e}")
            }
        })?;

        map_sentry_error(resp.status(), project)?;

        let text = resp.text().await?;
        let issues: Vec<SentryIssueSummary> = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("failed to parse Sentry issues JSON: {e}"))?;
        Ok(issues)
    }

    /// Get detailed information about a specific issue.
    pub async fn get_issue(&self, issue_id: &str) -> Result<SentryIssueDetail> {
        let url = format!("{}/issues/{}/", SENTRY_BASE_URL, issue_id);

        let resp = self.http.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                anyhow::anyhow!("Sentry API request timed out after {}s", REQUEST_TIMEOUT.as_secs())
            } else {
                anyhow::anyhow!("Sentry API request failed: {e}")
            }
        })?;

        map_sentry_error(resp.status(), issue_id)?;

        let text = resp.text().await?;
        let detail: SentryIssueDetail = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("failed to parse Sentry issue detail JSON: {e}"))?;
        Ok(detail)
    }

    /// Get the latest event for an issue (to extract stack trace).
    pub async fn get_latest_event(&self, issue_id: &str) -> Result<Option<SentryEvent>> {
        let url = format!("{}/issues/{}/events/latest/", SENTRY_BASE_URL, issue_id);

        let resp = self.http.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                anyhow::anyhow!("Sentry API request timed out after {}s", REQUEST_TIMEOUT.as_secs())
            } else {
                anyhow::anyhow!("Sentry API request failed: {e}")
            }
        })?;

        // 404 on latest event is not fatal — the issue might have no events yet
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        map_sentry_error(resp.status(), issue_id)?;

        let text = resp.text().await?;
        let event: SentryEvent = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("failed to parse Sentry event JSON: {e}"))?;
        Ok(Some(event))
    }
}

// ── Error Mapping ────────────────────────────────────────────────────

fn map_sentry_error(status: reqwest::StatusCode, context: &str) -> Result<()> {
    match status.as_u16() {
        200..=299 => Ok(()),
        401 => bail!("Sentry token expired or invalid — check SENTRY_AUTH_TOKEN"),
        403 => bail!("Sentry token lacks access to '{context}' — check token scopes"),
        404 => bail!("Sentry resource not found: '{context}'"),
        429 => bail!("Sentry API rate limit exceeded — try again later"),
        status => bail!("Sentry API returned HTTP {status} for '{context}'"),
    }
}

// ── Validation ───────────────────────────────────────────────────────

/// Validate a Sentry project slug (alphanumeric, hyphens, underscores).
fn validate_project_slug(slug: &str) -> Result<()> {
    if slug.is_empty() {
        bail!("project slug cannot be empty");
    }
    let valid = slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        bail!("invalid project slug: '{slug}' — only alphanumeric, hyphens, and underscores allowed");
    }
    Ok(())
}

/// Validate a Sentry issue ID (must be numeric).
fn validate_issue_id(id: &str) -> Result<()> {
    if id.is_empty() {
        bail!("issue ID cannot be empty");
    }
    if !id.chars().all(|c| c.is_ascii_digit()) {
        bail!("invalid issue ID: '{id}' — must be numeric");
    }
    Ok(())
}

// ── Stack Trace Formatting ───────────────────────────────────────────

/// Format stack trace frames into a condensed file:line list for Telegram.
///
/// Skips vendor/node_modules frames and limits to top N frames.
pub fn format_stack_trace(event: &SentryEvent) -> Option<String> {
    let entries = event.entries.as_ref()?;

    let exception_entry = entries
        .iter()
        .find(|e| e.entry_type == "exception")?;

    let data = exception_entry.data.as_ref()?;
    let values = data.values.as_ref()?;

    let mut all_frames: Vec<String> = Vec::new();

    for exc in values {
        if let Some(st) = &exc.stacktrace {
            // Sentry frames are bottom-up; reverse for top-down display
            let frames: Vec<&StackFrame> = st
                .frames
                .iter()
                .rev()
                .filter(|f| !is_vendor_frame(f))
                .take(MAX_STACK_FRAMES)
                .collect();

            if frames.is_empty() {
                continue;
            }

            // Add exception header if available
            if let Some(exc_type) = &exc.exception_type {
                let msg = exc.value.as_deref().unwrap_or("");
                all_frames.push(format!("{exc_type}: {msg}"));
            }

            for frame in &frames {
                let file = frame.filename.as_deref().unwrap_or("<unknown>");
                let func = frame.function.as_deref().unwrap_or("<anonymous>");
                let line = frame
                    .line_no
                    .map(|l| format!(":{l}"))
                    .unwrap_or_default();
                all_frames.push(format!("  {file}{line} in {func}"));
            }
        }
    }

    if all_frames.is_empty() {
        None
    } else {
        Some(all_frames.join("\n"))
    }
}

/// Check if a frame is from a vendor/node_modules path.
fn is_vendor_frame(frame: &StackFrame) -> bool {
    let filename = frame.filename.as_deref().unwrap_or("");
    let module = frame.module.as_deref().unwrap_or("");
    filename.contains("node_modules")
        || filename.contains("/vendor/")
        || module.contains("node_modules")
}

// ── Telegram Formatters ──────────────────────────────────────────────

impl SentryIssueSummary {
    pub fn format_for_telegram(&self) -> String {
        let icon = level_icon(&self.level);
        let culprit = self
            .culprit
            .as_deref()
            .unwrap_or("unknown");
        let last = super::relative_time(&self.last_seen);
        let when = if last.is_empty() { short_timestamp(&self.last_seen).to_string() } else { last };
        format!(
            "🐛 {icon} **#{}** {} — {} events\n   {} · {when}",
            self.id, self.title, self.count, culprit
        )
    }
}

impl SentryIssueDetail {
    pub fn format_for_telegram(&self, stack_trace: Option<&str>) -> String {
        let icon = level_icon(&self.level);
        let culprit = self
            .culprit
            .as_deref()
            .unwrap_or("unknown");
        let first = super::relative_time(&self.first_seen);
        let last = super::relative_time(&self.last_seen);
        let first_str = if first.is_empty() { short_timestamp(&self.first_seen).to_string() } else { first };
        let last_str = if last.is_empty() { short_timestamp(&self.last_seen).to_string() } else { last };
        let mut out = format!(
            "🐛 {icon} **#{}** {} [{}]\n   {culprit}\n   {} events · first: {first_str} · last: {last_str}",
            self.id,
            self.title,
            self.status,
            self.count,
        );
        if let Some(trace) = stack_trace {
            out.push_str("\n\nStack trace:\n");
            out.push_str(trace);
        }
        out
    }
}

fn level_icon(level: &str) -> &'static str {
    match level {
        "error" | "fatal" => "\u{274c}",  // red X
        "warning" => "\u{26a0}",          // warning
        "info" => "\u{2139}",             // info
        _ => "\u{2753}",                  // question mark
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

/// List unresolved Sentry issues for a project.
///
/// Uses a pre-initialized client (from the service registry) when provided,
/// otherwise constructs one from environment variables on demand.
pub async fn sentry_issues(client: &SentryClient, project: &str) -> Result<String> {
    validate_project_slug(project)?;
    let issues = client.list_issues(project).await?;

    if issues.is_empty() {
        return Ok(format!("No unresolved issues on {project}."));
    }

    let mut lines = vec![format!("{} unresolved issue(s) on {project}:", issues.len())];
    for issue in &issues {
        lines.push(issue.format_for_telegram());
    }
    Ok(lines.join("\n"))
}

/// Get details and stack trace for a specific Sentry issue.
///
/// Uses a pre-initialized client (from the service registry) when provided,
/// otherwise constructs one from environment variables on demand.
pub async fn sentry_issue(client: &SentryClient, issue_id: &str) -> Result<String> {
    validate_issue_id(issue_id)?;
    let detail = client.get_issue(issue_id).await?;

    // Fetch latest event for stack trace
    let stack_trace = match client.get_latest_event(issue_id).await {
        Ok(Some(event)) => format_stack_trace(&event),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(issue_id, error = %e, "failed to fetch latest event for stack trace");
            None
        }
    };

    Ok(detail.format_for_telegram(stack_trace.as_deref()))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all Sentry tools.
pub fn sentry_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "sentry_issues".into(),
            description: "List unresolved Sentry issues for a project. Returns issue ID, title, level, event count, and culprit file/function.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Sentry project slug (e.g. 'otaku-odyssey', 'tribal-cities')"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "sentry_issue".into(),
            description: "Get details for a specific Sentry issue by ID. Returns title, status, event count, first/last seen, and stack trace (top 5 frames).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Sentry issue ID (numeric)"
                    }
                },
                "required": ["id"]
            }),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Validation Tests ─────────────────────────────────────────

    #[test]
    fn validate_project_slug_valid() {
        assert!(validate_project_slug("otaku-odyssey").is_ok());
        assert!(validate_project_slug("tribal_cities").is_ok());
        assert!(validate_project_slug("my-project-123").is_ok());
    }

    #[test]
    fn validate_project_slug_invalid() {
        assert!(validate_project_slug("").is_err());
        assert!(validate_project_slug("has spaces").is_err());
        assert!(validate_project_slug("has/slash").is_err());
        assert!(validate_project_slug("has@symbol").is_err());
    }

    #[test]
    fn validate_issue_id_valid() {
        assert!(validate_issue_id("12345").is_ok());
        assert!(validate_issue_id("1").is_ok());
        assert!(validate_issue_id("999999999").is_ok());
    }

    #[test]
    fn validate_issue_id_invalid() {
        assert!(validate_issue_id("").is_err());
        assert!(validate_issue_id("abc").is_err());
        assert!(validate_issue_id("123abc").is_err());
        assert!(validate_issue_id("12.34").is_err());
    }

    // ── Parse Tests ──────────────────────────────────────────────

    #[test]
    fn parse_issues_list_empty() {
        let json = "[]";
        let issues: Vec<SentryIssueSummary> = serde_json::from_str(json).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn parse_issues_list_single() {
        let json = r#"[{
            "id": "12345",
            "title": "TypeError: Cannot read property 'x' of undefined",
            "culprit": "auth.ts in handleLogin",
            "count": "42",
            "firstSeen": "2026-03-20T10:00:00Z",
            "lastSeen": "2026-03-22T15:30:00Z",
            "level": "error"
        }]"#;
        let issues: Vec<SentryIssueSummary> = serde_json::from_str(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "12345");
        assert_eq!(
            issues[0].title,
            "TypeError: Cannot read property 'x' of undefined"
        );
        assert_eq!(issues[0].culprit.as_deref(), Some("auth.ts in handleLogin"));
        assert_eq!(issues[0].count, "42");
        assert_eq!(issues[0].level, "error");
    }

    #[test]
    fn parse_issue_detail() {
        let json = r#"{
            "id": "12345",
            "title": "TypeError: Cannot read property 'x' of undefined",
            "culprit": "auth.ts in handleLogin",
            "count": "42",
            "firstSeen": "2026-03-20T10:00:00Z",
            "lastSeen": "2026-03-22T15:30:00Z",
            "level": "error",
            "status": "unresolved"
        }"#;
        let detail: SentryIssueDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail.id, "12345");
        assert_eq!(detail.status, "unresolved");
        assert_eq!(detail.count, "42");
    }

    #[test]
    fn parse_issues_malformed_json() {
        let result: Result<Vec<SentryIssueSummary>, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    // ── Stack Trace Formatting Tests ─────────────────────────────

    #[test]
    fn format_stack_trace_with_frames() {
        let event = SentryEvent {
            entries: Some(vec![EventEntry {
                entry_type: "exception".into(),
                data: Some(EventEntryData {
                    values: Some(vec![ExceptionEntry {
                        exception_type: Some("TypeError".into()),
                        value: Some("Cannot read 'x'".into()),
                        stacktrace: Some(Stacktrace {
                            frames: vec![
                                StackFrame {
                                    filename: Some("node_modules/express/lib/router.js".into()),
                                    function: Some("handle".into()),
                                    line_no: Some(100),
                                    col_no: None,
                                    module: None,
                                },
                                StackFrame {
                                    filename: Some("src/auth.ts".into()),
                                    function: Some("handleLogin".into()),
                                    line_no: Some(42),
                                    col_no: Some(12),
                                    module: None,
                                },
                                StackFrame {
                                    filename: Some("src/utils.ts".into()),
                                    function: Some("validate".into()),
                                    line_no: Some(15),
                                    col_no: None,
                                    module: None,
                                },
                            ],
                        }),
                    }]),
                }),
            }]),
        };

        let result = format_stack_trace(&event).unwrap();
        assert!(result.contains("TypeError: Cannot read 'x'"));
        // Should skip node_modules frame
        assert!(!result.contains("node_modules"));
        // Should contain application frames (reversed — top-down)
        assert!(result.contains("src/utils.ts:15 in validate"));
        assert!(result.contains("src/auth.ts:42 in handleLogin"));
    }

    #[test]
    fn format_stack_trace_empty_entries() {
        let event = SentryEvent {
            entries: Some(vec![]),
        };
        assert!(format_stack_trace(&event).is_none());
    }

    #[test]
    fn format_stack_trace_no_entries() {
        let event = SentryEvent { entries: None };
        assert!(format_stack_trace(&event).is_none());
    }

    #[test]
    fn format_stack_trace_only_vendor_frames() {
        let event = SentryEvent {
            entries: Some(vec![EventEntry {
                entry_type: "exception".into(),
                data: Some(EventEntryData {
                    values: Some(vec![ExceptionEntry {
                        exception_type: Some("Error".into()),
                        value: Some("vendor error".into()),
                        stacktrace: Some(Stacktrace {
                            frames: vec![StackFrame {
                                filename: Some("node_modules/lib/index.js".into()),
                                function: Some("run".into()),
                                line_no: Some(1),
                                col_no: None,
                                module: None,
                            }],
                        }),
                    }]),
                }),
            }]),
        };
        // Only vendor frames — should return None (no useful frames to show)
        assert!(format_stack_trace(&event).is_none());
    }

    // ── Telegram Formatter Tests ─────────────────────────────────

    #[test]
    fn format_issue_summary_for_telegram() {
        let issue = SentryIssueSummary {
            id: "12345".into(),
            title: "TypeError in auth.ts".into(),
            culprit: Some("handleLogin".into()),
            count: "42".into(),
            first_seen: "2026-03-20T10:00:00Z".into(),
            last_seen: "2026-03-22T15:30:00Z".into(),
            level: "error".into(),
        };
        let formatted = issue.format_for_telegram();
        assert!(formatted.contains("🐛"));
        assert!(formatted.contains("#12345"));
        assert!(formatted.contains("TypeError in auth.ts"));
        assert!(formatted.contains("42 events"));
        assert!(formatted.contains("handleLogin"));
        assert!(formatted.contains("\u{274c}")); // error icon
    }

    #[test]
    fn format_issue_detail_for_telegram_with_trace() {
        let detail = SentryIssueDetail {
            id: "12345".into(),
            title: "TypeError".into(),
            culprit: Some("auth.ts".into()),
            count: "10".into(),
            first_seen: "2026-03-20T10:00:00Z".into(),
            last_seen: "2026-03-22T15:30:00Z".into(),
            level: "error".into(),
            status: "unresolved".into(),
        };
        let trace = "TypeError: Cannot read 'x'\n  src/auth.ts:42 in handleLogin";
        let formatted = detail.format_for_telegram(Some(trace));
        assert!(formatted.contains("🐛"));
        assert!(formatted.contains("#12345"));
        assert!(formatted.contains("TypeError"));
        assert!(formatted.contains("[unresolved]"));
        assert!(formatted.contains("Stack trace:"));
        assert!(formatted.contains("src/auth.ts:42"));
    }

    #[test]
    fn format_issue_detail_for_telegram_no_trace() {
        let detail = SentryIssueDetail {
            id: "99".into(),
            title: "Warning".into(),
            culprit: None,
            count: "1".into(),
            first_seen: "2026-03-22T00:00:00Z".into(),
            last_seen: "2026-03-22T00:00:00Z".into(),
            level: "warning".into(),
            status: "unresolved".into(),
        };
        let formatted = detail.format_for_telegram(None);
        assert!(formatted.contains("🐛"));
        assert!(formatted.contains("\u{26a0}")); // warning icon
        assert!(formatted.contains("unknown")); // no culprit
        assert!(!formatted.contains("Stack trace:"));
    }

    // ── Level Icon Tests ─────────────────────────────────────────

    #[test]
    fn level_icons() {
        assert_eq!(level_icon("error"), "\u{274c}");
        assert_eq!(level_icon("fatal"), "\u{274c}");
        assert_eq!(level_icon("warning"), "\u{26a0}");
        assert_eq!(level_icon("info"), "\u{2139}");
        assert_eq!(level_icon("debug"), "\u{2753}");
    }

    // ── Short Timestamp Tests ────────────────────────────────────

    #[test]
    fn short_timestamp_full_iso() {
        assert_eq!(short_timestamp("2026-03-22T15:30:00Z"), "2026-03-22");
    }

    #[test]
    fn short_timestamp_already_short() {
        assert_eq!(short_timestamp("2026"), "2026");
    }

    // ── Tool Definition Tests ────────────────────────────────────

    #[test]
    fn sentry_tool_definitions_returns_two_tools() {
        let tools = sentry_tool_definitions();
        assert_eq!(tools.len(), 2);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"sentry_issues"));
        assert!(names.contains(&"sentry_issue"));
    }

    #[test]
    fn tool_definitions_have_correct_schema() {
        let tools = sentry_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
        }
    }

    // ── Vendor Frame Detection Tests ─────────────────────────────

    #[test]
    fn is_vendor_frame_node_modules() {
        let frame = StackFrame {
            filename: Some("node_modules/express/lib/router.js".into()),
            function: Some("handle".into()),
            line_no: Some(100),
            col_no: None,
            module: None,
        };
        assert!(is_vendor_frame(&frame));
    }

    #[test]
    fn is_vendor_frame_vendor_path() {
        let frame = StackFrame {
            filename: Some("/vendor/lib/something.py".into()),
            function: None,
            line_no: None,
            col_no: None,
            module: None,
        };
        assert!(is_vendor_frame(&frame));
    }

    #[test]
    fn is_not_vendor_frame_app_code() {
        let frame = StackFrame {
            filename: Some("src/auth.ts".into()),
            function: Some("handleLogin".into()),
            line_no: Some(42),
            col_no: None,
            module: None,
        };
        assert!(!is_vendor_frame(&frame));
    }
}

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for SentryClient {
    fn name(&self) -> &str {
        "sentry"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;
        let url = format!("{SENTRY_BASE_URL}/organizations/{}/", self.org);
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async { self.http.get(&url).send().await }).await;
        match result {
            Ok(resp) if resp.status().is_success() => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: format!("org: {}", self.org),
            },
            Ok(resp) if resp.status().as_u16() == 401 => crate::tools::CheckResult::Unhealthy {
                error: "token invalid or expired (401) — check SENTRY_AUTH_TOKEN".into(),
            },
            Ok(resp) if resp.status().as_u16() == 403 => crate::tools::CheckResult::Unhealthy {
                error: format!("access denied (403) to org '{}' — check token scopes", self.org),
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
