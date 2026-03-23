//! Sentry watcher — checks for error spikes and creates obligations.
//!
//! Evaluates the `sentry_spike` alert rule by querying the Sentry API for
//! unresolved issues. If any issue has an event count above the configured
//! threshold, it fires.
//!
//! Config JSON keys (optional, set in `alert_rules.rules[].config`):
//! - `project`: Sentry project slug to check (required — logs warning if absent)
//! - `threshold`: minimum event count to trigger (default: 10)

use nv_core::types::ObligationOwner;
use uuid::Uuid;

use crate::alert_rules::{AlertRule, RuleEvaluator};
use crate::obligation_store::NewObligation;
use crate::tools::sentry::SentryClient;

const DEFAULT_THRESHOLD: u64 = 10;

/// Sentry watcher: evaluates `sentry_spike` alert rules.
pub struct SentryWatcher;

impl RuleEvaluator for SentryWatcher {
    async fn evaluate(&self, rule: &AlertRule) -> Option<NewObligation> {
        let client = match SentryClient::from_env() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    rule = %rule.name,
                    error = %e,
                    "sentry_watcher: Sentry not configured, skipping"
                );
                return None;
            }
        };

        let config_val: Option<serde_json::Value> = rule
            .config
            .as_deref()
            .and_then(|cfg| serde_json::from_str(cfg).ok());

        let project = config_val
            .as_ref()
            .and_then(|v| v.get("project"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let project = match project {
            Some(p) => p,
            None => {
                tracing::warn!(
                    rule = %rule.name,
                    "sentry_watcher: no 'project' in rule config, skipping"
                );
                return None;
            }
        };

        let threshold: u64 = config_val
            .as_ref()
            .and_then(|v| v.get("threshold"))
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_THRESHOLD);

        let issues = match client.list_issues(&project).await {
            Ok(issues) => issues,
            Err(e) => {
                tracing::warn!(
                    rule = %rule.name,
                    project = %project,
                    error = %e,
                    "sentry_watcher: failed to list issues"
                );
                return None;
            }
        };

        // Find issues with count above threshold
        let spiked: Vec<String> = issues
            .iter()
            .filter(|issue| {
                issue
                    .count
                    .parse::<u64>()
                    .map(|c| c >= threshold)
                    .unwrap_or(false)
            })
            .map(|issue| {
                format!(
                    "[{}] {} (count: {})",
                    issue.level, issue.title, issue.count
                )
            })
            .collect();

        if spiked.is_empty() {
            return None;
        }

        let description = if spiked.len() == 1 {
            format!("Sentry spike in {project}: {}", spiked[0])
        } else {
            format!(
                "{} Sentry issues spiked in {project} (threshold: {}): {}",
                spiked.len(),
                threshold,
                spiked.join("; ")
            )
        };

        tracing::info!(
            rule = %rule.name,
            project = %project,
            spiked = spiked.len(),
            "sentry_watcher: firing obligation"
        );

        Some(NewObligation {
            id: Uuid::new_v4().to_string(),
            source_channel: "watcher:sentry".to_string(),
            source_message: None,
            detected_action: description,
            project_code: Some(project),
            priority: 1, // High — error spikes need investigation
            owner: ObligationOwner::Nova,
            owner_reason: Some("sentry_spike alert rule triggered".to_string()),
        })
    }
}
