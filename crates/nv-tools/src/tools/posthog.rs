//! PostHog product analytics tools via REST API.
//!
//! Two read-only tools:
//! * `posthog_trends(project, event)` — daily event counts for the last 7 days.
//! * `posthog_flags(project)` — active feature flags with rollout percentages.
//!
//! Auth: `POSTHOG_API_KEY` env var (Personal API key).
//! Host: `POSTHOG_HOST` env var (default `app.posthog.com`).
//! Project mapping: `POSTHOG_PROJECT_ID` env var (default project ID)
//!   or per-project via `POSTHOG_PROJECT_<CODE>` (e.g. `POSTHOG_PROJECT_OO`).

use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::Deserialize;

/// Timeout for all PostHog API calls.
const POSTHOG_TIMEOUT: Duration = Duration::from_secs(15);

// ── Response Types ──────────────────────────────────────────────────

/// A single series result from the trends endpoint.
#[derive(Debug, Deserialize)]
struct TrendSeries {
    /// Human-readable label (event name or breakdown value).
    label: Option<String>,
    /// Day labels (e.g. "2026-03-15").
    days: Option<Vec<String>>,
    /// Count per day.
    data: Option<Vec<f64>>,
    /// Aggregate count across the period.
    count: Option<f64>,
}

/// Top-level trends response.
#[derive(Debug, Deserialize)]
struct TrendsResponse {
    result: Option<Vec<TrendSeries>>,
}

/// A single feature flag from the flags endpoint.
#[derive(Debug, Deserialize)]
struct FeatureFlag {
    key: String,
    name: Option<String>,
    active: bool,
    rollout_percentage: Option<f64>,
    #[allow(dead_code)]
    filters: Option<serde_json::Value>,
}

/// Paginated feature flags response.
#[derive(Debug, Deserialize)]
struct FeatureFlagsResponse {
    results: Option<Vec<FeatureFlag>>,
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Read the PostHog Personal API key from the environment.
pub fn api_key_pub() -> Result<String> {
    std::env::var("POSTHOG_API_KEY")
        .map_err(|_| anyhow!("POSTHOG_API_KEY env var not set"))
}

fn api_key() -> Result<String> {
    api_key_pub()
}

/// Read the PostHog host (default: `app.posthog.com`).
/// Strips any protocol prefix (https://, http://) if present.
pub fn host_pub() -> String {
    let raw = std::env::var("POSTHOG_HOST").unwrap_or_else(|_| "app.posthog.com".into());
    raw.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

fn host() -> String {
    let raw = std::env::var("POSTHOG_HOST").unwrap_or_else(|_| "app.posthog.com".into());
    raw.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

/// Resolve a project code (e.g. "oo", "tc") to a PostHog project ID.
///
/// Lookup order:
/// 1. `POSTHOG_PROJECT_<CODE>` env var (uppercased code)
/// 2. `POSTHOG_PROJECT_ID` env var (default fallback)
fn resolve_project_id(code: &str) -> Result<String> {
    let upper = code.to_uppercase();
    let per_project_var = format!("POSTHOG_PROJECT_{upper}");

    if let Ok(id) = std::env::var(&per_project_var) {
        return Ok(id);
    }
    if let Ok(id) = std::env::var("POSTHOG_PROJECT_ID") {
        return Ok(id);
    }
    Err(anyhow!(
        "Unknown project code '{code}'. Set {per_project_var} or POSTHOG_PROJECT_ID env var."
    ))
}

/// Build a reqwest client with the PostHog auth header — public for health checks.
pub fn build_client_pub(key: &str) -> Result<reqwest::Client> {
    build_client(key)
}

/// Build a reqwest client with the PostHog auth header.
fn build_client(key: &str) -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|e| anyhow!("invalid API key format: {e}"))?,
    );
    reqwest::Client::builder()
        .timeout(POSTHOG_TIMEOUT)
        .default_headers(headers)
        .build()
        .map_err(|e| anyhow!("failed to build HTTP client: {e}"))
}

/// Map HTTP status codes to human-readable errors.
fn map_status(status: reqwest::StatusCode, context: &str) -> anyhow::Error {
    match status.as_u16() {
        401 => anyhow!("PostHog API key invalid or expired (401) — check POSTHOG_API_KEY"),
        403 => anyhow!("PostHog access denied (403) — key may lack permissions for {context}"),
        404 => anyhow!("PostHog project not found (404) — check project ID for {context}"),
        429 => anyhow!("PostHog rate limited (429) — try again later"),
        code => anyhow!("PostHog API error ({code}) for {context}"),
    }
}

// ── posthog_trends ──────────────────────────────────────────────────

/// Query event trends for a project over the last 7 days.
///
/// Returns a Telegram-formatted daily breakdown with totals.
pub async fn query_trends(project: &str, event: &str) -> Result<String> {
    if event.is_empty() {
        return Err(anyhow!("event name is required"));
    }

    let key = api_key()?;
    let project_id = resolve_project_id(project)?;
    let base = host();

    let url = format!("https://{base}/api/projects/{project_id}/insights/trend/");

    let body = serde_json::json!({
        "events": [{"id": event, "type": "events"}],
        "date_from": "-7d",
        "interval": "day"
    });

    let client = build_client(&key)?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("PostHog trends request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(map_status(status, &format!("trends/{project}")));
    }

    let data: TrendsResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse PostHog trends response: {e}"))?;

    format_trends(project, event, &data)
}

/// Format trends response for Telegram delivery.
fn format_trends(project: &str, event: &str, data: &TrendsResponse) -> Result<String> {
    let results = data
        .result
        .as_ref()
        .ok_or_else(|| anyhow!("PostHog trends response missing 'result' field"))?;

    if results.is_empty() {
        return Ok(format!("No data for event '{event}' in project {project}."));
    }

    let series = &results[0];
    let days = series.days.as_deref().unwrap_or(&[]);
    let values = series.data.as_deref().unwrap_or(&[]);
    let total = series.count.unwrap_or(0.0) as i64;
    let label = series
        .label
        .as_deref()
        .unwrap_or(event);

    // Trend direction
    let trend = if values.len() >= 2 {
        let last = values[values.len() - 1];
        let prev = values[values.len() - 2];
        if last > prev { "↑" } else if last < prev { "↓" } else { "→" }
    } else {
        "→"
    };

    let mut out = format!("📊 **{label}** ({project}) — 7d · {total} total {trend}\n");

    for (day, val) in days.iter().zip(values.iter()) {
        // Show just the date part (YYYY-MM-DD -> MM-DD)
        let short_day = if day.len() >= 10 { &day[5..10] } else { day };
        let count = *val as i64;
        out.push_str(&format!("   {short_day}: {count}\n"));
    }

    // Remove trailing newline
    if out.ends_with('\n') {
        out.pop();
    }

    Ok(out)
}

// ── posthog_flags ───────────────────────────────────────────────────

/// List active feature flags for a project.
///
/// Returns a Telegram-formatted condensed list with rollout percentages.
pub async fn list_flags(project: &str) -> Result<String> {
    let key = api_key()?;
    let project_id = resolve_project_id(project)?;
    let base = host();

    let url = format!(
        "https://{base}/api/projects/{project_id}/feature_flags/?limit=50"
    );

    let client = build_client(&key)?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("PostHog flags request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(map_status(status, &format!("flags/{project}")));
    }

    let data: FeatureFlagsResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse PostHog flags response: {e}"))?;

    format_flags(project, &data)
}

/// Format feature flags for Telegram delivery.
fn format_flags(project: &str, data: &FeatureFlagsResponse) -> Result<String> {
    let flags = data
        .results
        .as_ref()
        .ok_or_else(|| anyhow!("PostHog flags response missing 'results' field"))?;

    let active: Vec<&FeatureFlag> = flags.iter().filter(|f| f.active).collect();

    if active.is_empty() {
        return Ok(format!("No active feature flags in project {project}."));
    }

    let mut out = format!("📊 **Feature flags** ({project}) — {} active\n", active.len());

    for flag in &active {
        let name = flag.name.as_deref().unwrap_or("");
        let rollout = flag
            .rollout_percentage
            .map(|p| format!("{p:.0}%"))
            .unwrap_or_else(|| "100%".into());

        if name.is_empty() || name == flag.key {
            out.push_str(&format!("   • {} [{}]\n", flag.key, rollout));
        } else {
            out.push_str(&format!("   • {} — {} [{}]\n", flag.key, name, rollout));
        }
    }

    // Trim trailing newline
    if out.ends_with('\n') {
        out.pop();
    }

    Ok(out)
}

// ── PosthogClient wrapper ────────────────────────────────────────────

/// Thin wrapper for `Checkable` health checks.
pub struct PosthogClient;

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_trends_empty_result() {
        let data = TrendsResponse {
            result: Some(vec![]),
        };
        let out = format_trends("oo", "$pageview", &data).unwrap();
        assert!(out.contains("No data"));
    }

    #[test]
    fn format_trends_with_data() {
        let data = TrendsResponse {
            result: Some(vec![TrendSeries {
                label: Some("$pageview".into()),
                days: Some(vec![
                    "2026-03-15".into(),
                    "2026-03-16".into(),
                    "2026-03-17".into(),
                ]),
                data: Some(vec![10.0, 15.0, 12.0]),
                count: Some(37.0),
            }]),
        };
        let out = format_trends("oo", "$pageview", &data).unwrap();
        assert!(out.contains("📊"));
        assert!(out.contains("$pageview"));
        assert!(out.contains("(oo)"));
        assert!(out.contains("03-15: 10"));
        assert!(out.contains("03-16: 15"));
        assert!(out.contains("03-17: 12"));
        assert!(out.contains("37 total"));
        // last (12) < prev (15) → downtrend
        assert!(out.contains("↓"));
    }

    #[test]
    fn format_trends_uptrend() {
        let data = TrendsResponse {
            result: Some(vec![TrendSeries {
                label: Some("signup".into()),
                days: Some(vec!["2026-03-20".into(), "2026-03-21".into()]),
                data: Some(vec![5.0, 8.0]),
                count: Some(13.0),
            }]),
        };
        let out = format_trends("tc", "signup", &data).unwrap();
        assert!(out.contains("↑"));
    }

    #[test]
    fn format_flags_no_active() {
        let data = FeatureFlagsResponse {
            results: Some(vec![FeatureFlag {
                key: "old-flag".into(),
                name: None,
                active: false,
                rollout_percentage: None,
                filters: None,
            }]),
        };
        let out = format_flags("oo", &data).unwrap();
        assert!(out.contains("No active feature flags"));
    }

    #[test]
    fn format_flags_with_active() {
        let data = FeatureFlagsResponse {
            results: Some(vec![
                FeatureFlag {
                    key: "new-checkout".into(),
                    name: Some("New Checkout Flow".into()),
                    active: true,
                    rollout_percentage: Some(50.0),
                    filters: None,
                },
                FeatureFlag {
                    key: "beta-ui".into(),
                    name: None,
                    active: true,
                    rollout_percentage: None,
                    filters: None,
                },
                FeatureFlag {
                    key: "old-thing".into(),
                    name: Some("Deprecated".into()),
                    active: false,
                    rollout_percentage: Some(0.0),
                    filters: None,
                },
            ]),
        };
        let out = format_flags("tc", &data).unwrap();
        assert!(out.contains("📊"));
        assert!(out.contains("2 active"));
        assert!(out.contains("new-checkout"));
        assert!(out.contains("New Checkout Flow"));
        assert!(out.contains("[50%]"));
        assert!(out.contains("beta-ui"));
        assert!(out.contains("[100%]"));
        // Inactive flag should not appear
        assert!(!out.contains("old-thing"));
    }

    #[test]
    fn resolve_project_id_missing() {
        // Clear any existing env vars for this test
        std::env::remove_var("POSTHOG_PROJECT_XX");
        std::env::remove_var("POSTHOG_PROJECT_ID");
        let result = resolve_project_id("xx");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("POSTHOG_PROJECT_XX"));
    }

    #[test]
    fn resolve_project_id_per_project() {
        std::env::set_var("POSTHOG_PROJECT_ZZ", "12345");
        let result = resolve_project_id("zz").unwrap();
        assert_eq!(result, "12345");
        std::env::remove_var("POSTHOG_PROJECT_ZZ");
    }

    #[test]
    fn resolve_project_id_default_fallback() {
        std::env::remove_var("POSTHOG_PROJECT_YY");
        std::env::set_var("POSTHOG_PROJECT_ID", "99999");
        let result = resolve_project_id("yy").unwrap();
        assert_eq!(result, "99999");
        std::env::remove_var("POSTHOG_PROJECT_ID");
    }

    #[test]
    fn query_trends_rejects_empty_event() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        std::env::set_var("POSTHOG_API_KEY", "test");
        std::env::set_var("POSTHOG_PROJECT_ID", "1");
        let result = rt.block_on(query_trends("oo", ""));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("event name is required"));
        std::env::remove_var("POSTHOG_API_KEY");
        std::env::remove_var("POSTHOG_PROJECT_ID");
    }

    #[test]
    fn api_key_missing() {
        std::env::remove_var("POSTHOG_API_KEY");
        let result = api_key();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("POSTHOG_API_KEY"));
    }

    #[test]
    fn host_default() {
        std::env::remove_var("POSTHOG_HOST");
        assert_eq!(host(), "app.posthog.com");
    }

    #[test]
    fn host_custom() {
        std::env::set_var("POSTHOG_HOST", "eu.posthog.com");
        assert_eq!(host(), "eu.posthog.com");
        std::env::remove_var("POSTHOG_HOST");
    }
}

// ── Tool Definitions ─────────────────────────────────────────────────────────

/// Return MCP tool definitions for the two PostHog analytics tools.
pub fn posthog_tool_definitions() -> Vec<nv_core::ToolDefinition> {
    vec![
        nv_core::ToolDefinition {
            name: "posthog_trends".into(),
            description: "Get event trend data from PostHog for a project over the last 7 days. \
                Returns daily counts with totals and trend direction."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    },
                    "event": {
                        "type": "string",
                        "description": "PostHog event name (e.g. '$pageview', 'signup', 'purchase')"
                    }
                },
                "required": ["project", "event"]
            }),
        },
        nv_core::ToolDefinition {
            name: "posthog_flags".into(),
            description: "List active feature flags from PostHog for a project. \
                Returns flag keys, names, and rollout percentages."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    }
                },
                "required": ["project"]
            }),
        },
    ]
}
