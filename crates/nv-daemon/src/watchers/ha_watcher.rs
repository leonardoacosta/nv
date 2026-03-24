//! Home Assistant anomaly watcher — checks for unexpected entity states.
//!
//! Evaluates the `ha_anomaly` alert rule by querying the Home Assistant REST
//! API for specific watched entities. If any entity has a state outside its
//! expected range or has a specific anomalous value, it fires.
//!
//! Config JSON keys (optional, set in `alert_rules.rules[].config`):
//! - `entities`: array of entity IDs to watch (required — logs warning if absent)
//! - `anomaly_states`: array of state strings considered anomalous (default: ["unavailable", "unknown"])
//!
//! Example config:
//! ```json
//! {
//!   "entities": ["sensor.living_room_temperature", "binary_sensor.front_door"],
//!   "anomaly_states": ["unavailable", "unknown", "on"]
//! }
//! ```

use nv_core::types::ObligationOwner;
use uuid::Uuid;

use crate::alert_rules::{AlertRule, RuleEvaluator};
use crate::obligation_store::NewObligation;
use crate::tools::ha::HAClient;

/// Default states considered anomalous when no `anomaly_states` config is set.
const DEFAULT_ANOMALY_STATES: &[&str] = &["unavailable", "unknown"];

/// Home Assistant watcher: evaluates `ha_anomaly` alert rules.
pub struct HaWatcher;

impl RuleEvaluator for HaWatcher {
    async fn evaluate(&self, rule: &AlertRule) -> Option<NewObligation> {
        let client = match HAClient::from_env() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    rule = %rule.name,
                    error = %e,
                    "ha_watcher: Home Assistant not configured, skipping"
                );
                return None;
            }
        };

        let config_val: Option<serde_json::Value> = rule
            .config
            .as_deref()
            .and_then(|cfg| serde_json::from_str(cfg).ok());

        // Parse entity list
        let entities: Vec<String> = config_val
            .as_ref()
            .and_then(|v| v.get("entities"))
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
            .unwrap_or_default();

        if entities.is_empty() {
            tracing::warn!(
                rule = %rule.name,
                "ha_watcher: no 'entities' in rule config, skipping"
            );
            return None;
        }

        // Parse anomaly states
        let anomaly_states: Vec<String> = config_val
            .as_ref()
            .and_then(|v| v.get("anomaly_states"))
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
            .unwrap_or_else(|| {
                DEFAULT_ANOMALY_STATES
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            });

        // Fetch all entity states in a single bulk request instead of N+1 serial calls.
        let all_states = match client.states().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    rule = %rule.name,
                    error = %e,
                    "ha_watcher: failed to fetch bulk entity states"
                );
                return None;
            }
        };

        // Index by entity_id for O(1) lookup.
        let state_map: std::collections::HashMap<&str, &crate::tools::ha::HAEntity> =
            all_states.iter().map(|e| (e.entity_id.as_str(), e)).collect();

        let mut anomalies: Vec<String> = Vec::new();

        for entity_id in &entities {
            match state_map.get(entity_id.as_str()) {
                Some(entity) => {
                    let state_lower = entity.state.to_lowercase();
                    let is_anomalous = anomaly_states
                        .iter()
                        .any(|s| s.to_lowercase() == state_lower);

                    if is_anomalous {
                        let last_changed = entity
                            .last_changed
                            .as_deref()
                            .unwrap_or("unknown time");
                        anomalies.push(format!(
                            "{entity_id} = '{}' (since {last_changed})",
                            entity.state
                        ));
                    }
                }
                None => {
                    tracing::warn!(
                        rule = %rule.name,
                        entity = %entity_id,
                        "ha_watcher: configured entity not found in bulk states response"
                    );
                }
            }
        }

        if anomalies.is_empty() {
            return None;
        }

        let description = if anomalies.len() == 1 {
            format!("HA anomaly: {}", anomalies[0])
        } else {
            format!(
                "{} HA anomalies: {}",
                anomalies.len(),
                anomalies.join("; ")
            )
        };

        tracing::info!(
            rule = %rule.name,
            anomalies = anomalies.len(),
            "ha_watcher: firing obligation"
        );

        Some(NewObligation {
            id: Uuid::new_v4().to_string(),
            source_channel: "watcher:ha".to_string(),
            source_message: None,
            detected_action: description,
            project_code: None,
            priority: 2, // Important — HA anomalies need attention but aren't critical
            owner: ObligationOwner::Nova,
            owner_reason: Some("ha_anomaly alert rule triggered".to_string()),
        })
    }
}
