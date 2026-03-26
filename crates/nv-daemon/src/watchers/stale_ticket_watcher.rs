//! Stale ticket watcher — checks for open beads issues without recent activity.
//!
//! Evaluates the `stale_ticket` alert rule by reading the local beads
//! `issues.jsonl` file. Any open issue whose `updated_at` is older than
//! `stale_days` fires as an obligation.
//!
//! Config JSON keys (optional, set in `alert_rules.rules[].config`):
//! - `stale_days`: days without activity before a ticket is considered stale (default: 7)
//! - `beads_path`: path to the `.beads/` directory (default: cwd + `.beads`)
//!
//! Note: This watcher reads the JSONL file directly — it does not invoke the
//! `bd` CLI — so it works even when the daemon is isolated.

use std::path::PathBuf;

use nv_core::types::ObligationOwner;
use serde::Deserialize;
use uuid::Uuid;

use crate::alert_rules::{AlertRule, RuleEvaluator};
use crate::obligation_store::NewObligation;

const DEFAULT_STALE_DAYS: i64 = 7;

/// A minimal view of a beads issue (only fields needed for staleness check).
#[derive(Debug, Deserialize)]
struct BeadsIssue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub updated_at: String,
    #[serde(default)]
    pub priority: i32,
}

/// Stale ticket watcher: evaluates `stale_ticket` alert rules.
pub struct StaleTicketWatcher;

impl RuleEvaluator for StaleTicketWatcher {
    async fn evaluate(&self, rule: &AlertRule) -> Option<NewObligation> {
        let config_val: Option<serde_json::Value> = rule
            .config
            .as_deref()
            .and_then(|cfg| serde_json::from_str(cfg).ok());

        let stale_days: i64 = config_val
            .as_ref()
            .and_then(|v| v.get("stale_days"))
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_STALE_DAYS);

        // Resolve beads path: configured override or auto-detect
        let beads_path: PathBuf = config_val
            .as_ref()
            .and_then(|v| v.get("beads_path"))
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Default: try CWD/.beads first, then HOME/.nv/.beads as fallback
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let candidate = cwd.join(".beads");
                if candidate.is_dir() {
                    candidate
                } else if let Ok(home) = std::env::var("HOME") {
                    PathBuf::from(home).join(".nv").join(".beads")
                } else {
                    candidate
                }
            });

        let issues_path = beads_path.join("issues.jsonl");

        let content = match std::fs::read_to_string(&issues_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(
                    rule = %rule.name,
                    path = %issues_path.display(),
                    error = %e,
                    "stale_ticket_watcher: could not read issues.jsonl"
                );
                return None;
            }
        };

        let cutoff = chrono::Utc::now() - chrono::Duration::days(stale_days);

        let mut stale_issues: Vec<BeadsIssue> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let issue: BeadsIssue = match serde_json::from_str(line) {
                Ok(i) => i,
                Err(e) => {
                    tracing::debug!(error = %e, "stale_ticket_watcher: skipping malformed JSONL line");
                    continue;
                }
            };

            // Only open issues
            if issue.status != "open" {
                continue;
            }

            // Parse updated_at (RFC3339)
            let updated_at = match chrono::DateTime::parse_from_rfc3339(&issue.updated_at) {
                Ok(dt) => dt.with_timezone(&chrono::Utc),
                Err(e) => {
                    tracing::debug!(
                        issue_id = %issue.id,
                        error = %e,
                        "stale_ticket_watcher: could not parse updated_at"
                    );
                    continue;
                }
            };

            if updated_at < cutoff {
                stale_issues.push(issue);
            }
        }

        if stale_issues.is_empty() {
            return None;
        }

        // Sort by priority ascending (0 = most urgent), then oldest first
        stale_issues.sort_by(|a, b| {
            a.priority.cmp(&b.priority).then_with(|| {
                a.updated_at.cmp(&b.updated_at)
            })
        });

        let description = if stale_issues.len() == 1 {
            let i = &stale_issues[0];
            format!(
                "Stale ticket ({} days+): {} — {}",
                stale_days, i.id, i.title
            )
        } else {
            let examples: Vec<String> = stale_issues
                .iter()
                .take(3)
                .map(|i| format!("{} ({})", i.id, i.title))
                .collect();
            format!(
                "{} stale tickets (>{} days): {}{}",
                stale_issues.len(),
                stale_days,
                examples.join(", "),
                if stale_issues.len() > 3 {
                    format!(" +{} more", stale_issues.len() - 3)
                } else {
                    String::new()
                }
            )
        };

        tracing::info!(
            rule = %rule.name,
            stale_count = stale_issues.len(),
            stale_days,
            "stale_ticket_watcher: firing obligation"
        );

        Some(NewObligation {
            id: Uuid::new_v4().to_string(),
            source_channel: "watcher:stale_ticket".to_string(),
            source_message: None,
            detected_action: description,
            project_code: None,
            priority: 3, // Minor — stale tickets are low urgency
            owner: ObligationOwner::Leo,
            owner_reason: Some(format!(
                "stale_ticket alert rule triggered ({stale_days}+ days without activity)"
            )),
            deadline: None,
        })
    }
}
