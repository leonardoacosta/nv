//! Upstash Redis info tools via REST API.
//!
//! Two read-only tools:
//! * `upstash_info()` — get Redis server info (memory, clients, keyspace, uptime).
//! * `upstash_keys(pattern)` — list keys matching a glob pattern via SCAN.
//!
//! Auth: `UPSTASH_REDIS_REST_URL` + `UPSTASH_REDIS_REST_TOKEN` env vars.

use std::time::Duration;

use anyhow::{bail, Result};
use serde::Deserialize;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_KEYS: usize = 100;

// ── Types ────────────────────────────────────────────────────────────

/// Parsed Redis INFO sections relevant for monitoring.
#[derive(Debug, Clone, Default)]
pub struct UpstashInfo {
    pub connected_clients: Option<String>,
    pub used_memory_human: Option<String>,
    pub keyspace_hits: Option<String>,
    pub keyspace_misses: Option<String>,
    pub uptime_in_seconds: Option<String>,
    pub total_keys: Option<String>,
}

/// REST API response envelope from Upstash.
#[derive(Debug, Deserialize)]
pub struct UpstashResponse {
    pub result: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
}

// ── Client ───────────────────────────────────────────────────────────

/// HTTP client for the Upstash Redis REST API.
#[derive(Debug)]
pub struct UpstashClient {
    http: reqwest::Client,
    rest_url: String,
    token: String,
}

impl UpstashClient {
    /// Create a new `UpstashClient` from environment variables.
    ///
    /// Returns an error if `UPSTASH_REDIS_REST_URL` or `UPSTASH_REDIS_REST_TOKEN` is not set.
    pub fn from_env() -> Result<Self> {
        let rest_url = std::env::var("UPSTASH_REDIS_REST_URL").map_err(|_| {
            anyhow::anyhow!(
                "Upstash not configured — UPSTASH_REDIS_REST_URL env var not set"
            )
        })?;
        let token = std::env::var("UPSTASH_REDIS_REST_TOKEN").map_err(|_| {
            anyhow::anyhow!(
                "Upstash not configured — UPSTASH_REDIS_REST_TOKEN env var not set"
            )
        })?;

        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build Upstash HTTP client");

        Ok(Self {
            http,
            rest_url,
            token,
        })
    }

    /// Create an `UpstashClient` with custom URL, token, and HTTP client (for testing).
    #[cfg(test)]
    pub fn with_http_client(http: reqwest::Client, rest_url: &str, token: &str) -> Self {
        Self {
            http,
            rest_url: rest_url.to_string(),
            token: token.to_string(),
        }
    }

    /// Execute a Redis command via the Upstash REST API.
    ///
    /// Sends a POST with a JSON array body `["COMMAND", "arg1", ...]`.
    async fn execute_command(&self, args: &[&str]) -> Result<serde_json::Value> {
        let resp = self
            .http
            .post(&self.rest_url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&args)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    anyhow::anyhow!(
                        "Upstash API request timed out after {}s",
                        REQUEST_TIMEOUT.as_secs()
                    )
                } else {
                    anyhow::anyhow!("Upstash API request failed: {e}")
                }
            })?;

        map_upstash_error(resp.status())?;

        let text = resp.text().await?;
        let envelope: UpstashResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("failed to parse Upstash response JSON: {e}"))?;

        if let Some(err) = envelope.error {
            bail!("Upstash error: {err}");
        }

        Ok(envelope.result)
    }

    /// Get Redis server INFO.
    pub async fn info(&self) -> Result<UpstashInfo> {
        let result = self.execute_command(&["INFO"]).await?;
        let info_str = result
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("INFO response was not a string"))?;
        Ok(parse_info(info_str))
    }

    /// List keys matching a glob pattern using SCAN (safer than KEYS).
    ///
    /// Returns up to `MAX_KEYS` matching key names.
    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        let result = self
            .execute_command(&["SCAN", "0", "MATCH", pattern, "COUNT", "100"])
            .await?;

        // SCAN returns [cursor, [key1, key2, ...]]
        let arr = result
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("SCAN response was not an array"))?;

        if arr.len() < 2 {
            bail!("SCAN response had fewer than 2 elements");
        }

        let keys_val = arr[1]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("SCAN keys element was not an array"))?;

        let keys: Vec<String> = keys_val
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .take(MAX_KEYS)
            .collect();

        Ok(keys)
    }
}

// ── Error Mapping ────────────────────────────────────────────────────

fn map_upstash_error(status: reqwest::StatusCode) -> Result<()> {
    match status.as_u16() {
        200..=299 => Ok(()),
        401 => bail!("Upstash token invalid — check UPSTASH_REDIS_REST_TOKEN"),
        403 => bail!("Upstash token lacks required permissions"),
        429 => bail!("Upstash rate limit exceeded — try again later"),
        status => bail!("Upstash API returned HTTP {status}"),
    }
}

// ── INFO Parser ──────────────────────────────────────────────────────

/// Parse the Redis INFO response string into structured fields.
fn parse_info(info_str: &str) -> UpstashInfo {
    let mut info = UpstashInfo::default();

    for line in info_str.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            match key {
                "connected_clients" => info.connected_clients = Some(value.to_string()),
                "used_memory_human" => info.used_memory_human = Some(value.to_string()),
                "keyspace_hits" => info.keyspace_hits = Some(value.to_string()),
                "keyspace_misses" => info.keyspace_misses = Some(value.to_string()),
                "uptime_in_seconds" => info.uptime_in_seconds = Some(value.to_string()),
                "db0" => {
                    // db0:keys=123,expires=45,avg_ttl=0
                    if let Some(keys_part) = value.split(',').find(|s| s.starts_with("keys="))
                    {
                        info.total_keys =
                            Some(keys_part.trim_start_matches("keys=").to_string());
                    }
                }
                _ => {}
            }
        }
    }

    info
}

// ── Formatters ───────────────────────────────────────────────────────

/// Format Redis INFO for Telegram output.
pub fn format_info(info: &UpstashInfo) -> String {
    // Build summary line components
    let keys_str = info.total_keys.as_deref().unwrap_or("?");
    let mem_str = info.used_memory_human.as_deref().unwrap_or("?");

    let uptime_str = info.uptime_in_seconds.as_deref().map(|u| {
        if let Ok(secs) = u.parse::<u64>() {
            let days = secs / 86400;
            let hours = (secs % 86400) / 3600;
            format!("{days}d {hours}h")
        } else {
            format!("{u}s")
        }
    });

    if info.used_memory_human.is_none()
        && info.connected_clients.is_none()
        && info.keyspace_hits.is_none()
        && info.total_keys.is_none()
        && info.uptime_in_seconds.is_none()
    {
        return "🔑 **Redis** — No info available.".to_string();
    }

    let mut lines = vec![format!("🔑 **Redis** — {keys_str} keys · {mem_str}")];

    if let Some(clients) = &info.connected_clients {
        lines.push(format!("   clients: {clients}"));
    }
    if let Some(hits) = &info.keyspace_hits {
        let misses = info.keyspace_misses.as_deref().unwrap_or("0");
        lines.push(format!("   keyspace: {hits} hits / {misses} misses"));
    }
    if let Some(uptime) = uptime_str {
        lines.push(format!("   uptime: {uptime}"));
    }

    lines.join("\n")
}

/// Format a key list for Telegram output.
pub fn format_keys(pattern: &str, keys: &[String]) -> String {
    if keys.is_empty() {
        return format!("No keys matching '{pattern}'.");
    }

    let mut lines = vec![format!(
        "🔑 **Redis keys** — {} matching `{pattern}`",
        keys.len()
    )];
    for key in keys {
        lines.push(format!("   {key}"));
    }
    if keys.len() >= MAX_KEYS {
        lines.push(format!("   (capped at {MAX_KEYS} results)"));
    }
    lines.join("\n")
}

// ── Public Tool Handlers ─────────────────────────────────────────────

/// Get Redis server info.
pub async fn upstash_info() -> Result<String> {
    let client = UpstashClient::from_env()?;
    let info = client.info().await?;
    Ok(format_info(&info))
}

/// List keys matching a glob pattern.
pub async fn upstash_keys(pattern: &str) -> Result<String> {
    if pattern.is_empty() {
        bail!("pattern cannot be empty");
    }
    let client = UpstashClient::from_env()?;
    let keys = client.keys(pattern).await?;
    Ok(format_keys(pattern, &keys))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all Upstash tools.
pub fn upstash_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "upstash_info".into(),
            description: "Get Upstash Redis server info. Returns memory usage, connected clients, keyspace hits/misses, key count, and uptime.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "upstash_keys".into(),
            description: "List Upstash Redis keys matching a glob pattern. Uses SCAN for safety. Returns up to 100 matching key names.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern to match keys (e.g. 'session:*', 'ratelimit:*', '*')"
                    }
                },
                "required": ["pattern"]
            }),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── INFO Parse Tests ────────────────────────────────────────

    #[test]
    fn parse_info_full() {
        let info_str = "\
# Server\r\n\
uptime_in_seconds:86400\r\n\
\r\n\
# Clients\r\n\
connected_clients:5\r\n\
\r\n\
# Memory\r\n\
used_memory_human:1.50M\r\n\
\r\n\
# Stats\r\n\
keyspace_hits:1000\r\n\
keyspace_misses:200\r\n\
\r\n\
# Keyspace\r\n\
db0:keys=42,expires=10,avg_ttl=0\r\n";

        let info = parse_info(info_str);
        assert_eq!(info.connected_clients.as_deref(), Some("5"));
        assert_eq!(info.used_memory_human.as_deref(), Some("1.50M"));
        assert_eq!(info.keyspace_hits.as_deref(), Some("1000"));
        assert_eq!(info.keyspace_misses.as_deref(), Some("200"));
        assert_eq!(info.uptime_in_seconds.as_deref(), Some("86400"));
        assert_eq!(info.total_keys.as_deref(), Some("42"));
    }

    #[test]
    fn parse_info_empty() {
        let info = parse_info("");
        assert!(info.connected_clients.is_none());
        assert!(info.used_memory_human.is_none());
    }

    #[test]
    fn parse_info_comments_only() {
        let info = parse_info("# Server\n# Clients\n");
        assert!(info.connected_clients.is_none());
    }

    // ── Format Tests ────────────────────────────────────────────

    #[test]
    fn format_info_with_data() {
        let info = UpstashInfo {
            connected_clients: Some("5".into()),
            used_memory_human: Some("1.50M".into()),
            keyspace_hits: Some("1000".into()),
            keyspace_misses: Some("200".into()),
            uptime_in_seconds: Some("86400".into()),
            total_keys: Some("42".into()),
        };
        let output = format_info(&info);
        assert!(output.contains("🔑"));
        assert!(output.contains("Redis"));
        assert!(output.contains("1.50M"));
        assert!(output.contains("clients: 5"));
        assert!(output.contains("1000 hits / 200 misses"));
        assert!(output.contains("42 keys"));
        assert!(output.contains("1d 0h"));
    }

    #[test]
    fn format_info_empty() {
        let info = UpstashInfo::default();
        let output = format_info(&info);
        assert!(output.contains("No info available."));
    }

    #[test]
    fn format_keys_with_data() {
        let keys = vec![
            "session:abc".to_string(),
            "session:def".to_string(),
        ];
        let output = format_keys("session:*", &keys);
        assert!(output.contains("🔑"));
        assert!(output.contains("2 matching"));
        assert!(output.contains("session:abc"));
        assert!(output.contains("session:def"));
    }

    #[test]
    fn format_keys_empty() {
        let output = format_keys("nothing:*", &[]);
        assert_eq!(output, "No keys matching 'nothing:*'.");
    }

    // ── SCAN Response Parse Test ────────────────────────────────

    #[test]
    fn parse_scan_response() {
        let json = r#"{"result": ["0", ["key1", "key2", "key3"]]}"#;
        let resp: UpstashResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_none());
        let arr = resp.result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let keys = arr[1].as_array().unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn parse_info_response() {
        let json = r##"{"result": "# Server\r\nuptime_in_seconds:100\r\n"}"##;
        let resp: UpstashResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_none());
        assert!(resp.result.as_str().is_some());
    }

    #[test]
    fn parse_error_response() {
        let json = r#"{"result": null, "error": "ERR wrong number of arguments"}"#;
        let resp: UpstashResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.error.as_deref(), Some("ERR wrong number of arguments"));
    }

    // ── Tool Definition Tests ───────────────────────────────────

    #[test]
    fn upstash_tool_definitions_returns_two_tools() {
        let tools = upstash_tool_definitions();
        assert_eq!(tools.len(), 2);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"upstash_info"));
        assert!(names.contains(&"upstash_keys"));
    }

    #[test]
    fn tool_definitions_have_correct_schema() {
        let tools = upstash_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
        }
    }

    #[test]
    fn upstash_keys_schema_requires_pattern() {
        let tools = upstash_tool_definitions();
        let uk = tools.iter().find(|t| t.name == "upstash_keys").unwrap();
        let required = uk.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("pattern")));
    }

    // ── Client env Tests ────────────────────────────────────────

    #[test]
    fn client_from_env_fails_without_url() {
        let saved_url = std::env::var("UPSTASH_REDIS_REST_URL").ok();
        let saved_token = std::env::var("UPSTASH_REDIS_REST_TOKEN").ok();
        std::env::remove_var("UPSTASH_REDIS_REST_URL");
        std::env::remove_var("UPSTASH_REDIS_REST_TOKEN");
        let result = UpstashClient::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("UPSTASH_REDIS_REST_URL"));
        if let Some(val) = saved_url {
            std::env::set_var("UPSTASH_REDIS_REST_URL", val);
        }
        if let Some(val) = saved_token {
            std::env::set_var("UPSTASH_REDIS_REST_TOKEN", val);
        }
    }

    #[test]
    fn client_from_env_fails_without_token() {
        let saved_url = std::env::var("UPSTASH_REDIS_REST_URL").ok();
        let saved_token = std::env::var("UPSTASH_REDIS_REST_TOKEN").ok();
        std::env::set_var("UPSTASH_REDIS_REST_URL", "https://example.upstash.io");
        std::env::remove_var("UPSTASH_REDIS_REST_TOKEN");
        let result = UpstashClient::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("UPSTASH_REDIS_REST_TOKEN"));
        // Restore
        if let Some(val) = saved_url {
            std::env::set_var("UPSTASH_REDIS_REST_URL", val);
        } else {
            std::env::remove_var("UPSTASH_REDIS_REST_URL");
        }
        if let Some(val) = saved_token {
            std::env::set_var("UPSTASH_REDIS_REST_TOKEN", val);
        }
    }
}

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for UpstashClient {
    fn name(&self) -> &str {
        "upstash"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;
        let (latency, result) = timed(|| async { self.execute_command(&["INFO"]).await }).await;
        match result {
            Ok(_) => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "INFO command succeeded".into(),
            },
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}
