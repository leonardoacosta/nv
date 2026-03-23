//! Home Assistant tools via REST API (localhost:8123).
//!
//! Three tools:
//! * `ha_states()` — list all entity states grouped by domain.
//! * `ha_entity(id)` — get detailed state for a specific entity.
//! * `ha_service_call(domain, service, data)` — call a HA service (PendingAction).
//!
//! Auth: Bearer token via `HA_TOKEN` env var.
//! Base URL defaults to `http://localhost:8123`, overridable via `HA_URL`.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const DEFAULT_HA_URL: &str = "http://localhost:8123";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RECENT_ENTITIES: usize = 20;

// ── Types ────────────────────────────────────────────────────────────

/// A Home Assistant entity state from the REST API.
#[derive(Debug, Clone, Deserialize)]
pub struct HAEntity {
    pub entity_id: String,
    pub state: String,
    pub attributes: serde_json::Value,
    pub last_changed: Option<String>,
    pub last_updated: Option<String>,
}

// ── Client ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct HAClient {
    http: reqwest::Client,
    base_url: String,
}

impl HAClient {
    /// Create a new `HAClient` from environment variables.
    ///
    /// Returns an error if `HA_TOKEN` is not set.
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("HA_TOKEN")
            .map_err(|_| anyhow!("Home Assistant not configured — HA_TOKEN env var not set"))?;

        let base_url = std::env::var("HA_URL").unwrap_or_else(|_| DEFAULT_HA_URL.to_string());

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}")
                .parse()
                .expect("valid auth header"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().expect("valid content type"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()?;

        Ok(Self { http, base_url })
    }

    /// GET /api/states — list all entity states.
    pub async fn states(&self) -> Result<Vec<HAEntity>> {
        let url = format!("{}/api/states", self.base_url);
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HA API error ({status}): {body}");
        }

        let entities: Vec<HAEntity> = resp.json().await?;
        Ok(entities)
    }

    /// GET /api/states/<entity_id> — get a single entity's state.
    pub async fn entity(&self, id: &str) -> Result<HAEntity> {
        let url = format!("{}/api/states/{}", self.base_url, id);
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HA API error ({status}): {body}");
        }

        let entity: HAEntity = resp.json().await?;
        Ok(entity)
    }

    /// POST /api/services/<domain>/<service> — call a HA service.
    #[allow(dead_code)]
    pub async fn service_call(
        &self,
        domain: &str,
        service: &str,
        data: &serde_json::Value,
    ) -> Result<String> {
        let url = format!("{}/api/services/{}/{}", self.base_url, domain, service);
        let resp = self.http.post(&url).json(data).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HA service call failed ({status}): {body}");
        }

        let body = resp.text().await.unwrap_or_default();
        Ok(format!(
            "Service {domain}.{service} called successfully.\n{body}"
        ))
    }
}

// ── Tool Definitions ─────────────────────────────────────────────────

pub fn ha_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "ha_states".into(),
            description: "List all Home Assistant entity states, grouped by domain \
                (light, sensor, switch, climate, etc.). Shows counts per domain and \
                the 20 most recently changed entities."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "ha_entity".into(),
            description: "Get detailed state for a specific Home Assistant entity. \
                Returns state, all attributes (brightness, temperature, etc.), \
                last_changed, and last_updated."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Full entity ID (e.g., 'light.office', 'sensor.living_room_temperature')"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "ha_service_call".into(),
            description: "Call a Home Assistant service (e.g., turn on/off lights, \
                set temperature). Requires user confirmation before execution. \
                Use entity IDs from ha_states or ha_entity."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "description": "Service domain (e.g., 'light', 'switch', 'climate')"
                    },
                    "service": {
                        "type": "string",
                        "description": "Service name (e.g., 'turn_on', 'turn_off', 'set_temperature')"
                    },
                    "data": {
                        "type": "object",
                        "description": "Service data payload (e.g., {\"entity_id\": \"light.office\", \"brightness\": 128})"
                    }
                },
                "required": ["domain", "service", "data"]
            }),
        },
    ]
}

// ── Formatting ───────────────────────────────────────────────────────

/// Format a list of entities grouped by domain with the top N recently changed.
pub fn format_states(entities: &[HAEntity]) -> String {
    if entities.is_empty() {
        return "No items found.".to_string();
    }

    // Group by domain
    let mut by_domain: HashMap<String, Vec<&HAEntity>> = HashMap::new();
    for e in entities {
        let domain = e
            .entity_id
            .split('.')
            .next()
            .unwrap_or("unknown")
            .to_string();
        by_domain.entry(domain).or_default().push(e);
    }

    let mut domains: Vec<_> = by_domain.iter().collect();
    domains.sort_by_key(|(name, _)| (*name).clone());

    // Header with domain summary
    let domain_summary: Vec<String> = domains
        .iter()
        .map(|(name, ents)| format!("{name}:{}", ents.len()))
        .collect();
    let mut lines = vec![format!(
        "🏠 **Home Assistant** — {} entities ({})",
        entities.len(),
        domain_summary.join(", ")
    )];

    // Top N recently changed
    let mut sorted: Vec<&HAEntity> = entities.iter().collect();
    sorted.sort_by(|a, b| {
        let a_time = a.last_changed.as_deref().unwrap_or("");
        let b_time = b.last_changed.as_deref().unwrap_or("");
        b_time.cmp(a_time)
    });

    for e in sorted.iter().take(MAX_RECENT_ENTITIES) {
        let rel = e
            .last_changed
            .as_deref()
            .map(super::relative_time)
            .unwrap_or_default();
        let when = if rel.is_empty() {
            e.last_changed.as_deref().unwrap_or("unknown").to_string()
        } else {
            rel
        };
        lines.push(format!("   {} — {} ({})", e.entity_id, e.state, when));
    }

    lines.join("\n")
}

/// Format a single entity's full state and attributes.
pub fn format_entity(entity: &HAEntity) -> String {
    let rel = entity
        .last_changed
        .as_deref()
        .map(super::relative_time)
        .unwrap_or_default();
    let when = if rel.is_empty() {
        entity.last_changed.as_deref().unwrap_or("unknown").to_string()
    } else {
        rel
    };

    let mut lines = vec![format!(
        "🏠 **{}** — {}\n   changed: {}",
        entity.entity_id, entity.state, when
    )];

    // Format attributes
    if let Some(attrs) = entity.attributes.as_object() {
        if !attrs.is_empty() {
            let mut keys: Vec<_> = attrs.keys().collect();
            keys.sort();
            for key in keys {
                let val = &attrs[key];
                let display = match val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                lines.push(format!("   {key}: {display}"));
            }
        }
    }

    lines.join("\n")
}

/// Build a human-readable description for a pending HA service call.
pub fn describe_service_call(domain: &str, service: &str, data: &serde_json::Value) -> String {
    let entity_hint = data
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("(unspecified)");
    format!(
        "Home Assistant: {domain}.{service} on {entity_hint}\nData: {}",
        serde_json::to_string_pretty(data).unwrap_or_default()
    )
}

// ── Public Entry Points ──────────────────────────────────────────────

/// Execute ha_states: fetch all entities and return formatted summary.
pub async fn ha_states() -> Result<String> {
    let client = HAClient::from_env()?;
    let entities = client.states().await?;
    tracing::info!(count = entities.len(), "ha_states completed");
    Ok(format_states(&entities))
}

/// Execute ha_entity: fetch a single entity's state.
pub async fn ha_entity(id: &str) -> Result<String> {
    let client = HAClient::from_env()?;
    let entity = client.entity(id).await?;
    tracing::info!(entity_id = id, "ha_entity completed");
    Ok(format_entity(&entity))
}

/// Execute ha_service_call after confirmation.
#[allow(dead_code)]
pub async fn ha_service_call_execute(
    domain: &str,
    service: &str,
    data: &serde_json::Value,
) -> Result<String> {
    let client = HAClient::from_env()?;
    let result = client.service_call(domain, service, data).await?;
    tracing::info!(domain, service, "ha_service_call executed");
    Ok(result)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_states_empty() {
        assert_eq!(format_states(&[]), "No items found.");
    }

    #[test]
    fn test_format_states_groups_by_domain() {
        let entities = vec![
            HAEntity {
                entity_id: "light.office".into(),
                state: "on".into(),
                attributes: serde_json::json!({}),
                last_changed: Some("2026-03-22T10:00:00Z".into()),
                last_updated: None,
            },
            HAEntity {
                entity_id: "light.bedroom".into(),
                state: "off".into(),
                attributes: serde_json::json!({}),
                last_changed: Some("2026-03-22T09:00:00Z".into()),
                last_updated: None,
            },
            HAEntity {
                entity_id: "sensor.temperature".into(),
                state: "22.5".into(),
                attributes: serde_json::json!({"unit_of_measurement": "°C"}),
                last_changed: Some("2026-03-22T11:00:00Z".into()),
                last_updated: None,
            },
        ];

        let output = format_states(&entities);
        assert!(output.contains("🏠"));
        assert!(output.contains("3 entities"));
        assert!(output.contains("light:2"));
        assert!(output.contains("sensor:1"));
        // Most recent first
        assert!(output.contains("sensor.temperature"));
    }

    #[test]
    fn test_format_entity_full() {
        let entity = HAEntity {
            entity_id: "light.office".into(),
            state: "on".into(),
            attributes: serde_json::json!({
                "brightness": 200,
                "friendly_name": "Office Light"
            }),
            last_changed: Some("2026-03-22T10:00:00Z".into()),
            last_updated: Some("2026-03-22T10:05:00Z".into()),
        };

        let output = format_entity(&entity);
        assert!(output.contains("🏠"));
        assert!(output.contains("light.office"));
        assert!(output.contains("on"));
        assert!(output.contains("brightness: 200"));
        assert!(output.contains("friendly_name: Office Light"));
    }

    #[test]
    fn test_describe_service_call() {
        let data = serde_json::json!({"entity_id": "light.office"});
        let desc = describe_service_call("light", "turn_off", &data);
        assert!(desc.contains("light.turn_off"));
        assert!(desc.contains("light.office"));
    }

    #[test]
    fn test_ha_client_from_env_missing_token() {
        std::env::remove_var("HA_TOKEN");
        let result = HAClient::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not configured"));
    }
}

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for HAClient {
    fn name(&self) -> &str {
        "ha"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;
        let url = format!("{}/api/", self.base_url);
        let (latency, result) = timed(|| async { self.http.get(&url).send().await }).await;
        match result {
            Ok(resp) if resp.status().is_success() => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: format!("API reachable ({})", self.base_url),
            },
            Ok(resp) if resp.status().as_u16() == 401 => crate::tools::CheckResult::Unhealthy {
                error: "token invalid (401) — check HA_TOKEN".into(),
            },
            Ok(resp) => crate::tools::CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: format!("unreachable ({}): {e}", self.base_url),
            },
        }
    }

    async fn check_write(&self) -> Option<crate::tools::CheckResult> {
        use crate::tools::check::timed;
        // POST /api/services/light/turn_on with empty body — expect 200 or 400
        let url = format!("{}/api/services/light/turn_on", self.base_url);
        let (latency, result) = timed(|| async {
            self.http.post(&url).json(&serde_json::json!({})).send().await
        })
        .await;
        let result = match result {
            // HA returns 200 with an entity state array on success, or 400 on bad input
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 400 => {
                crate::tools::CheckResult::Healthy {
                    latency_ms: latency,
                    detail: "services endpoint writable".into(),
                }
            }
            Ok(resp) if resp.status().as_u16() == 401 => crate::tools::CheckResult::Unhealthy {
                error: "write probe: token invalid (401)".into(),
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
