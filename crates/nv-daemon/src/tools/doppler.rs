//! Doppler secrets management tools — read-only, names only.
//!
//! Three tools exposing Doppler secrets metadata WITHOUT ever returning secret values:
//! * `doppler_secrets` — list secret names for a project+environment.
//! * `doppler_compare` — diff secret names between two environments.
//! * `doppler_activity` — recent audit log entries for a project.
//!
//! Authentication: `DOPPLER_API_TOKEN` environment variable.
//! Alias mapping: optional `[doppler.projects]` in `nv.toml`.
//!
//! SECURITY INVARIANT: This module NEVER returns secret values.
//! Only JSON object keys (name strings) are extracted from the Doppler API response.

use std::collections::HashSet;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

use crate::claude::ToolDefinition;
use nv_core::config::DopplerConfig;

// ── Constants ────────────────────────────────────────────────────────

const DOPPLER_API: &str = "https://api.doppler.com";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

// ── DopplerClient ────────────────────────────────────────────────────

/// HTTP client for the Doppler REST API v3.
///
/// Reads `DOPPLER_API_TOKEN` from the environment at construction time.
/// All requests use Bearer token authentication.
#[derive(Debug)]
pub struct DopplerClient {
    http: reqwest::Client,
    token: String,
}

impl DopplerClient {
    /// Create a client from `DOPPLER_API_TOKEN` environment variable.
    ///
    /// Returns `Err` if the env var is not set or empty.
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("DOPPLER_API_TOKEN")
            .map_err(|_| anyhow!("DOPPLER_API_TOKEN env var not set"))?;
        if token.is_empty() {
            bail!("DOPPLER_API_TOKEN env var is empty");
        }
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()?,
            token,
        })
    }

    /// Build a GET request with Bearer authorization.
    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
    }

    /// Map HTTP status codes to actionable error messages.
    fn map_status(status: reqwest::StatusCode, context: &str) -> anyhow::Error {
        match status.as_u16() {
            401 => anyhow!("Doppler token invalid or expired (401) — check DOPPLER_API_TOKEN"),
            403 => anyhow!("Doppler token lacks permission (403) — check token scopes"),
            404 => anyhow!("{context} not found (404)"),
            429 => anyhow!("Doppler rate limit hit (429) — wait a moment and retry"),
            code => anyhow!("Doppler API error ({code}) for {context}"),
        }
    }
}

// ── Project Alias Resolution ─────────────────────────────────────────

/// Resolve a project alias using the optional `DopplerConfig`.
///
/// If the input matches an alias in `config.projects`, returns the full project name.
/// Otherwise, returns the input unchanged (raw project name pass-through).
///
/// Returns secret names only — never values.
pub fn resolve_project(project: &str, config: Option<&DopplerConfig>) -> String {
    if let Some(cfg) = config {
        if let Some(full_name) = cfg.projects.get(project) {
            return full_name.clone();
        }
    }
    project.to_string()
}

// ── Doppler API response types ───────────────────────────────────────

/// Generic Doppler API envelope with a `secrets` map.
/// We only keep the keys (secret names) — values are intentionally ignored.
#[derive(Deserialize)]
struct SecretsResponse {
    secrets: serde_json::Map<String, serde_json::Value>,
}

/// A single Doppler activity log entry.
#[derive(Deserialize)]
struct LogEntry {
    #[serde(rename = "createdAt", alias = "created_at")]
    created_at: Option<String>,
    #[serde(rename = "user", default)]
    user: Option<LogUser>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct LogUser {
    name: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
struct LogsResponse {
    logs: Vec<LogEntry>,
}

// ── Tool: doppler_secrets ────────────────────────────────────────────

/// List secret names only for a Doppler project and environment.
///
/// Returns secret names only — never values.
/// The API response contains full secret objects; we extract only the JSON object keys.
pub async fn doppler_secrets(
    client: &DopplerClient,
    project: &str,
    environment: &str,
    doppler_config: Option<&DopplerConfig>,
) -> Result<String> {
    if project.is_empty() {
        bail!("project cannot be empty");
    }
    if environment.is_empty() {
        bail!("environment cannot be empty");
    }

    let resolved_project = resolve_project(project, doppler_config);

    let url = format!("{DOPPLER_API}/v3/configs/config/secrets");
    let resp = client
        .get(&url)
        .query(&[
            ("project", resolved_project.as_str()),
            ("config", environment),
        ])
        .send()
        .await
        .map_err(|e| anyhow!("Doppler request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(DopplerClient::map_status(
            resp.status(),
            &format!("project '{resolved_project}' env '{environment}'"),
        ));
    }

    // Extract only the JSON object keys — never deserialize values.
    let body: SecretsResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse Doppler secrets response: {e}"))?;

    // Returns secret names only — never values.
    let mut names: Vec<String> = body.secrets.keys().cloned().collect();
    names.sort();
    let count = names.len();

    let mut lines = vec![format!(
        "{resolved_project}/{environment} — {count} secret(s):"
    )];
    for name in &names {
        lines.push(format!("  {name}"));
    }

    Ok(lines.join("\n"))
}

// ── Tool: doppler_compare ────────────────────────────────────────────

/// Compare secret names between two environments of the same project.
///
/// Returns secret names only — never values.
/// Shows which secrets exist only in env_a, only in env_b, and the common count.
pub async fn doppler_compare(
    client: &DopplerClient,
    project: &str,
    env_a: &str,
    env_b: &str,
    doppler_config: Option<&DopplerConfig>,
) -> Result<String> {
    if project.is_empty() {
        bail!("project cannot be empty");
    }
    if env_a.is_empty() || env_b.is_empty() {
        bail!("both env_a and env_b must be non-empty");
    }

    let resolved_project = resolve_project(project, doppler_config);

    // Fetch both environments' secret name sets in sequence
    let names_a = fetch_secret_names(client, &resolved_project, env_a).await?;
    let names_b = fetch_secret_names(client, &resolved_project, env_b).await?;

    let set_a: HashSet<&str> = names_a.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = names_b.iter().map(|s| s.as_str()).collect();

    let only_in_a: Vec<&str> = {
        let mut v: Vec<&str> = set_a.difference(&set_b).copied().collect();
        v.sort();
        v
    };
    let only_in_b: Vec<&str> = {
        let mut v: Vec<&str> = set_b.difference(&set_a).copied().collect();
        v.sort();
        v
    };
    let common_count = set_a.intersection(&set_b).count();

    if only_in_a.is_empty() && only_in_b.is_empty() {
        return Ok(format!(
            "{resolved_project}: {env_a} and {env_b} are fully aligned \
            ({common_count} secrets in common)."
        ));
    }

    let mut lines = vec![format!(
        "Secret diff for {resolved_project}: {env_a} vs {env_b}\n"
    )];

    if only_in_a.is_empty() {
        lines.push(format!("Only in {env_a}: (none)"));
    } else {
        lines.push(format!("Only in {env_a} ({}):", only_in_a.len()));
        for name in &only_in_a {
            lines.push(format!("  {name}"));
        }
    }

    lines.push(String::new());

    if only_in_b.is_empty() {
        lines.push(format!("Only in {env_b}: (none)"));
    } else {
        lines.push(format!("Only in {env_b} ({}):", only_in_b.len()));
        for name in &only_in_b {
            lines.push(format!("  {name}"));
        }
    }

    lines.push(String::new());
    lines.push(format!("Common: {common_count} secret(s)"));

    Ok(lines.join("\n"))
}

/// Fetch secret names only for a project/environment.
///
/// Returns secret names only — never values.
async fn fetch_secret_names(
    client: &DopplerClient,
    project: &str,
    environment: &str,
) -> Result<Vec<String>> {
    let url = format!("{DOPPLER_API}/v3/configs/config/secrets");
    let resp = client
        .get(&url)
        .query(&[("project", project), ("config", environment)])
        .send()
        .await
        .map_err(|e| anyhow!("Doppler request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(DopplerClient::map_status(
            resp.status(),
            &format!("project '{project}' env '{environment}'"),
        ));
    }

    // Extract only the JSON object keys — never deserialize values.
    let body: SecretsResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse Doppler secrets response: {e}"))?;

    // Returns secret names only — never values.
    let mut names: Vec<String> = body.secrets.keys().cloned().collect();
    names.sort();
    Ok(names)
}

// ── Tool: doppler_activity ───────────────────────────────────────────

/// Fetch recent activity log entries for a Doppler project.
///
/// Returns formatted list of log entries with timestamp, user, and action text.
/// Count is clamped to 1..=25, default 10.
pub async fn doppler_activity(
    client: &DopplerClient,
    project: &str,
    count: Option<u64>,
    doppler_config: Option<&DopplerConfig>,
) -> Result<String> {
    if project.is_empty() {
        bail!("project cannot be empty");
    }

    let resolved_project = resolve_project(project, doppler_config);
    let count = count.unwrap_or(10).clamp(1, 25) as usize;

    let url = format!("{DOPPLER_API}/v3/logs");
    let resp = client
        .get(&url)
        .query(&[("project", resolved_project.as_str())])
        .send()
        .await
        .map_err(|e| anyhow!("Doppler request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(DopplerClient::map_status(
            resp.status(),
            &format!("activity for '{resolved_project}'"),
        ));
    }

    let body: LogsResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse Doppler activity response: {e}"))?;

    if body.logs.is_empty() {
        return Ok(format!("No activity found for {resolved_project}."));
    }

    let mut lines = vec![format!(
        "Recent activity for {resolved_project} ({} entr{}):",
        count.min(body.logs.len()),
        if count == 1 { "y" } else { "ies" }
    )];

    for entry in body.logs.into_iter().take(count) {
        let timestamp = entry
            .created_at
            .as_deref()
            .unwrap_or("?");

        let actor = entry
            .user
            .as_ref()
            .and_then(|u| u.name.as_deref().or(u.email.as_deref()))
            .unwrap_or("unknown");

        let text = entry.text.as_deref().unwrap_or("(no description)");

        lines.push(format!("  [{timestamp}] {actor}: {text}"));
    }

    Ok(lines.join("\n"))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all Doppler tools.
pub fn doppler_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "doppler_secrets".into(),
            description: "List secret NAMES only for a Doppler project and environment. Never returns secret values — names only. Use project aliases (e.g. 'oo') or full Doppler project names.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project alias (e.g. 'oo') or full Doppler project name (e.g. 'otaku-odyssey')"
                    },
                    "environment": {
                        "type": "string",
                        "description": "Doppler config/environment name (e.g. 'dev', 'stg', 'prd', 'dev_e2e')"
                    }
                },
                "required": ["project", "environment"]
            }),
        },
        ToolDefinition {
            name: "doppler_compare".into(),
            description: "Compare secret names between two environments of the same Doppler project. Shows which secrets exist only in one environment vs the other, and the count of common secrets. Never returns secret values.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project alias or full Doppler project name"
                    },
                    "env_a": {
                        "type": "string",
                        "description": "First environment to compare (e.g. 'dev')"
                    },
                    "env_b": {
                        "type": "string",
                        "description": "Second environment to compare (e.g. 'prd')"
                    }
                },
                "required": ["project", "env_a", "env_b"]
            }),
        },
        ToolDefinition {
            name: "doppler_activity".into(),
            description: "Fetch recent activity log entries for a Doppler project. Shows who changed what and when. Useful for auditing secret changes or troubleshooting environment drift.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project alias or full Doppler project name"
                    },
                    "count": {
                        "type": "integer",
                        "description": "Number of activity entries to return (default: 10, max: 25)"
                    }
                },
                "required": ["project"]
            }),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::config::DopplerConfig;
    use std::collections::HashMap;

    fn make_config(aliases: &[(&str, &str)]) -> DopplerConfig {
        DopplerConfig {
            projects: aliases
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn doppler_tool_definitions_returns_three_tools() {
        let tools = doppler_tool_definitions();
        assert_eq!(tools.len(), 3);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"doppler_secrets"));
        assert!(names.contains(&"doppler_compare"));
        assert!(names.contains(&"doppler_activity"));
    }

    #[test]
    fn tool_definitions_have_schemas() {
        for tool in doppler_tool_definitions() {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
        }
    }

    #[test]
    fn doppler_secrets_requires_project_and_environment() {
        let tools = doppler_tool_definitions();
        let t = tools.iter().find(|t| t.name == "doppler_secrets").unwrap();
        let required = t.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("project")));
        assert!(required.iter().any(|v| v.as_str() == Some("environment")));
    }

    #[test]
    fn doppler_compare_requires_project_env_a_env_b() {
        let tools = doppler_tool_definitions();
        let t = tools.iter().find(|t| t.name == "doppler_compare").unwrap();
        let required = t.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("project")));
        assert!(required.iter().any(|v| v.as_str() == Some("env_a")));
        assert!(required.iter().any(|v| v.as_str() == Some("env_b")));
    }

    #[test]
    fn resolve_project_with_alias() {
        let cfg = make_config(&[("oo", "otaku-odyssey"), ("tc", "tribal-cities")]);
        assert_eq!(resolve_project("oo", Some(&cfg)), "otaku-odyssey");
        assert_eq!(resolve_project("tc", Some(&cfg)), "tribal-cities");
    }

    #[test]
    fn resolve_project_passthrough_when_no_alias() {
        let cfg = make_config(&[("oo", "otaku-odyssey")]);
        assert_eq!(resolve_project("unknown-project", Some(&cfg)), "unknown-project");
    }

    #[test]
    fn resolve_project_without_config() {
        assert_eq!(resolve_project("myproject", None), "myproject");
    }

    #[test]
    fn client_from_env_fails_without_token() {
        let saved = std::env::var("DOPPLER_API_TOKEN").ok();
        unsafe { std::env::remove_var("DOPPLER_API_TOKEN"); }
        let result = DopplerClient::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("DOPPLER_API_TOKEN"));
        if let Some(v) = saved {
            unsafe { std::env::set_var("DOPPLER_API_TOKEN", v); }
        }
    }
}
