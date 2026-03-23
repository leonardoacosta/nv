//! Deploy watcher — checks Vercel for failed deployments and creates obligations.
//!
//! Evaluates the `deploy_failure` alert rule by querying the Vercel API for
//! recent deployments across all configured project names. If any deployment
//! in the last check window has state `ERROR` or `FAILED`, it fires.
//!
//! Config JSON keys (optional, set in `alert_rules.rules[].config`):
//! - `projects`: array of Vercel project names to watch (default: checks all)
//! - `window_minutes`: how far back to look for failed deployments (default: 10)

use nv_core::types::ObligationOwner;
use uuid::Uuid;

use crate::alert_rules::{AlertRule, RuleEvaluator};
use crate::obligation_store::NewObligation;
use crate::tools::vercel::VercelClient;

/// Deploy watcher: evaluates `deploy_failure` alert rules.
pub struct DeployWatcher;

impl RuleEvaluator for DeployWatcher {
    async fn evaluate(&self, rule: &AlertRule) -> Option<NewObligation> {
        let client = match VercelClient::from_env() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    rule = %rule.name,
                    error = %e,
                    "deploy_watcher: Vercel not configured, skipping"
                );
                return None;
            }
        };

        // Parse optional project list from rule config
        let projects: Vec<String> = rule
            .config
            .as_deref()
            .and_then(|cfg| serde_json::from_str::<serde_json::Value>(cfg).ok())
            .and_then(|v| v.get("projects").cloned())
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default();

        let window_minutes: i64 = rule
            .config
            .as_deref()
            .and_then(|cfg| serde_json::from_str::<serde_json::Value>(cfg).ok())
            .and_then(|v| v.get("window_minutes").and_then(|w| w.as_i64()))
            .unwrap_or(10);

        let cutoff_ms = (chrono::Utc::now() - chrono::Duration::minutes(window_minutes))
            .timestamp_millis() as u64;

        // If no projects configured, we can't enumerate them from the API alone.
        // Attempt a known fallback or skip with a debug log.
        if projects.is_empty() {
            tracing::debug!(
                rule = %rule.name,
                "deploy_watcher: no projects configured in rule config, skipping"
            );
            return None;
        }

        let mut failed_summaries: Vec<String> = Vec::new();

        for project in &projects {
            match client.list_deployments(project).await {
                Ok(deployments) => {
                    for dep in &deployments {
                        // Only look at deployments within the window
                        let in_window = dep
                            .created_at
                            .map(|ts| ts >= cutoff_ms)
                            .unwrap_or(false);

                        if !in_window {
                            continue;
                        }

                        let state = dep.state.as_deref().unwrap_or("");
                        if state.eq_ignore_ascii_case("ERROR") || state.eq_ignore_ascii_case("FAILED") {
                            let branch = dep
                                .meta
                                .as_ref()
                                .and_then(|m| m.github_commit_ref.as_deref())
                                .unwrap_or("unknown branch");
                            let msg = dep
                                .meta
                                .as_ref()
                                .and_then(|m| m.github_commit_message.as_deref())
                                .unwrap_or("");
                            failed_summaries.push(format!(
                                "{project} ({branch}): {state} — {msg}"
                            ));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        rule = %rule.name,
                        project = %project,
                        error = %e,
                        "deploy_watcher: failed to list deployments"
                    );
                }
            }
        }

        if failed_summaries.is_empty() {
            return None;
        }

        let description = if failed_summaries.len() == 1 {
            format!("Deploy failure: {}", failed_summaries[0])
        } else {
            format!(
                "{} deploy failures: {}",
                failed_summaries.len(),
                failed_summaries.join("; ")
            )
        };

        tracing::info!(
            rule = %rule.name,
            failures = failed_summaries.len(),
            "deploy_watcher: firing obligation"
        );

        Some(NewObligation {
            id: Uuid::new_v4().to_string(),
            source_channel: "watcher:deploy".to_string(),
            source_message: None,
            detected_action: description,
            project_code: projects.first().cloned(),
            priority: 1, // Critical — deploy failures block users
            owner: ObligationOwner::Nova,
            owner_reason: Some("deploy_failure alert rule triggered".to_string()),
        })
    }
}
