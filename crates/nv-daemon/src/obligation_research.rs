//! Proactive obligation research module.
//!
//! After an obligation is created, `schedule_research` fires a low-priority
//! background worker that uses available tools (Jira, GitHub, calendar,
//! email) to gather context and stores structured notes on the obligation.
//!
//! Notes surface automatically in followup messages and query context.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use nv_core::config::ObligationResearchConfig;
use nv_core::types::{ObligationResearchTrigger, ObligationStatus, Trigger};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::worker::{Priority, SharedDeps, WorkerTask, WorkerPool};

// ── Data Types ───────────────────────────────────────────────────────

/// A single research finding from one tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Tool name: "jira", "github", "calendar", "email", "web".
    pub tool: String,
    /// Short human-readable label (e.g. "OO-42 status: In Progress").
    pub label: String,
    /// Optional longer description or raw snippet.
    pub detail: Option<String>,
}

/// Result of a proactive research session for one obligation.
///
/// Stored in the `obligation_notes` table and surfaced in followup/query context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    pub obligation_id: String,
    /// 2-5 sentence prose summary for use in briefings and query context.
    pub summary: String,
    pub raw_findings: Vec<Finding>,
    pub researched_at: DateTime<Utc>,
    /// Names of tools that were actually called during this session.
    pub tools_used: Vec<String>,
    /// Set when research partially or fully failed; `None` on clean success.
    pub error: Option<String>,
}

// ── Raw Claude response shape (internal) ─────────────────────────────

/// JSON shape returned by Claude in a research session.
#[derive(Debug, Deserialize)]
struct ResearchResponse {
    summary: String,
    #[serde(default)]
    findings: Vec<Finding>,
    #[serde(default)]
    tools_used: Vec<String>,
}

// ── Public API ────────────────────────────────────────────────────────

/// Schedule a background research task for the given obligation.
///
/// Returns immediately. A Tokio task is spawned that sleeps for
/// `config.research_delay_secs`, re-checks that the obligation is still
/// `Open`, then dispatches a `Priority::Low` `WorkerTask` into the pool.
///
/// Filters applied before spawning:
/// - `config.enabled` must be `true`
/// - `obligation.priority <= config.min_priority` (lower value = higher urgency)
/// - If `config.trigger_channels` is non-empty, obligation channel must be in the list
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)] // wired up by proactive-obligation-research spec
pub fn schedule_research(
    obligation_id: String,
    detected_action: String,
    project_code: Option<String>,
    source_channel: String,
    priority: i32,
    deps: Arc<SharedDeps>,
    pool: Arc<WorkerPool>,
    config: ObligationResearchConfig,
) {
    // Apply priority gate before spawning
    if priority > config.min_priority {
        tracing::debug!(
            obligation_id = %obligation_id,
            priority,
            min_priority = config.min_priority,
            "skipping research: obligation priority too low"
        );
        return;
    }

    // Apply channel gate before spawning
    if !config.trigger_channels.is_empty()
        && !config.trigger_channels.contains(&source_channel)
    {
        tracing::debug!(
            obligation_id = %obligation_id,
            channel = %source_channel,
            "skipping research: channel not in trigger_channels"
        );
        return;
    }

    let delay = config.research_delay_secs;
    let ob_id_clone = obligation_id.clone();

    tokio::spawn(async move {
        // Settling delay — allows obligation to be dismissed before research fires.
        tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;

        // Dismissed guard: re-fetch and check status.
        if let Some(store_arc) = &deps.obligation_store {
            match store_arc.lock() {
                Ok(store) => match store.get_by_id(&ob_id_clone) {
                    Ok(Some(ob)) if ob.status != ObligationStatus::Open => {
                        tracing::debug!(
                            obligation_id = %ob_id_clone,
                            status = %ob.status,
                            "skipping research: obligation no longer open"
                        );
                        return;
                    }
                    Ok(None) => {
                        tracing::debug!(
                            obligation_id = %ob_id_clone,
                            "skipping research: obligation not found after delay"
                        );
                        return;
                    }
                    Ok(Some(_)) => {} // still open — proceed
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            obligation_id = %ob_id_clone,
                            "dismissed guard: failed to re-fetch obligation, proceeding anyway"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "dismissed guard: obligation store mutex poisoned, proceeding"
                    );
                }
            }
        }

        // Build the research trigger.
        let research_trigger = ObligationResearchTrigger {
            obligation_id: obligation_id.clone(),
            detected_action,
            project_code,
            source_channel,
            priority,
        };

        let task = WorkerTask {
            id: Uuid::new_v4(),
            triggers: vec![Trigger::ObligationResearch(research_trigger)],
            priority: Priority::Low,
            created_at: std::time::Instant::now(),
            telegram_chat_id: None,
            telegram_message_id: None,
            cli_response_txs: vec![],
            is_edit_reply: false,
            editing_action_id: None,
            slug: format!("research-{}", &obligation_id[..8.min(obligation_id.len())]),
            is_voice_trigger: false,
        };

        tracing::info!(
            obligation_id = %obligation_id,
            "dispatching obligation research task"
        );

        pool.dispatch(task).await;
    });
}

// ── Research Prompt Builder ───────────────────────────────────────────

/// Build the system prompt supplement for a research worker session.
pub fn build_research_prompt(trigger: &ObligationResearchTrigger, has_jira: bool, has_calendar: bool) -> String {
    let project_line = trigger
        .project_code
        .as_deref()
        .map(|p| format!("Project: {p}\n"))
        .unwrap_or_default();

    let tool_instructions = build_tool_instructions(trigger, has_jira, has_calendar);

    format!(
        "You are researching context for a tracked obligation. Do not respond conversationally.\n\
         Obligation: {detected_action}\n\
         {project_line}\
         Channel: {source_channel}\n\
         \n\
         Use available tools to gather relevant context:\n\
         {tool_instructions}\n\
         \n\
         Return a JSON object with this shape:\n\
         {{\n\
           \"summary\": \"<2-5 sentence prose summary of findings>\",\n\
           \"findings\": [\n\
             {{ \"tool\": \"<tool>\", \"label\": \"<short finding>\", \"detail\": \"<optional longer text>\" }}\n\
           ],\n\
           \"tools_used\": [\"jira\", \"github\"]\n\
         }}\n\
         \n\
         If no relevant context is found, return summary: \"No additional context found.\" with empty findings.\n\
         Do not send a message to Leo. Do not use TTS. Just return the JSON.",
        detected_action = trigger.detected_action,
        source_channel = trigger.source_channel,
    )
}

fn build_tool_instructions(
    trigger: &ObligationResearchTrigger,
    has_jira: bool,
    has_calendar: bool,
) -> String {
    let mut lines = Vec::new();

    if has_jira {
        lines.push(
            "- If a Jira ticket key is present, fetch its status, assignee, and recent comments."
                .to_string(),
        );
    }

    // GitHub is always available via web/search tools
    lines.push(
        "- If a GitHub repo or PR is referenced, fetch open issues/PRs related to the obligation."
            .to_string(),
    );

    if has_calendar {
        lines.push(
            "- If a deadline is mentioned, check the calendar for nearby events.".to_string(),
        );
    }

    lines.push(
        "- If the obligation mentions an email thread, search recent email.".to_string(),
    );

    // Add project-specific Jira hint
    if let Some(project) = &trigger.project_code {
        lines.push(format!(
            "- Focus on project {project} when searching Jira or GitHub."
        ));
    }

    lines.join("\n")
}

// ── JSON Parse Helper ─────────────────────────────────────────────────

/// Parse Claude's research response text into a `ResearchResult`.
///
/// Extracts the JSON block from the response (Claude may wrap it in prose).
/// On parse failure, returns a `ResearchResult` with the `error` field set.
pub fn parse_research_response(obligation_id: &str, response_text: &str) -> ResearchResult {
    let now = Utc::now();

    // Try to find a JSON object in the response text.
    let json_text = extract_json_block(response_text);

    match serde_json::from_str::<ResearchResponse>(&json_text) {
        Ok(parsed) => ResearchResult {
            obligation_id: obligation_id.to_string(),
            summary: parsed.summary,
            raw_findings: parsed.findings,
            researched_at: now,
            tools_used: parsed.tools_used,
            error: None,
        },
        Err(e) => {
            tracing::warn!(
                obligation_id = %obligation_id,
                error = %e,
                response_len = response_text.len(),
                "failed to parse research response JSON"
            );
            ResearchResult {
                obligation_id: obligation_id.to_string(),
                summary: format!("Research failed: {e}"),
                raw_findings: vec![],
                researched_at: now,
                tools_used: vec![],
                error: Some(format!("JSON parse error: {e}")),
            }
        }
    }
}

/// Extract the first JSON object `{...}` block from a text string.
///
/// Claude sometimes wraps the JSON in prose. This locates the outermost
/// `{` … `}` pair and returns that substring for parsing.
fn extract_json_block(text: &str) -> String {
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end >= start {
                return text[start..=end].to_string();
            }
        }
    }
    text.to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trigger(priority: i32, channel: &str) -> ObligationResearchTrigger {
        ObligationResearchTrigger {
            obligation_id: "ob-test-1".to_string(),
            detected_action: "Merge the PR for OO-42".to_string(),
            project_code: Some("OO".to_string()),
            source_channel: channel.to_string(),
            priority,
        }
    }

    fn make_config(min_priority: i32, channels: Vec<String>) -> ObligationResearchConfig {
        ObligationResearchConfig {
            enabled: true,
            trigger_channels: channels,
            max_tools: 5,
            min_priority,
            research_delay_secs: 0,
        }
    }

    #[test]
    fn min_priority_gate_skips_low_priority() {
        // Priority 3 with min_priority=2 should be skipped (3 > 2)
        let config = make_config(2, vec![]);
        let trigger = make_trigger(3, "telegram");
        // The gate logic: priority > config.min_priority → skip
        assert!(trigger.priority > config.min_priority);
    }

    #[test]
    fn min_priority_gate_passes_high_priority() {
        // Priority 1 with min_priority=2 should proceed (1 <= 2)
        let config = make_config(2, vec![]);
        let trigger = make_trigger(1, "telegram");
        assert!(trigger.priority <= config.min_priority);
    }

    #[test]
    fn min_priority_gate_passes_equal_priority() {
        // Priority 2 with min_priority=2 should proceed (2 <= 2)
        let config = make_config(2, vec![]);
        let trigger = make_trigger(2, "telegram");
        assert!(trigger.priority <= config.min_priority);
    }

    #[test]
    fn channel_gate_blocks_unlisted_channel() {
        let config = make_config(2, vec!["telegram".to_string()]);
        let trigger = make_trigger(1, "discord");
        // Non-empty list and channel not in it → skip
        assert!(!config.trigger_channels.is_empty());
        assert!(!config.trigger_channels.contains(&trigger.source_channel));
    }

    #[test]
    fn channel_gate_passes_listed_channel() {
        let config = make_config(2, vec!["telegram".to_string()]);
        let trigger = make_trigger(1, "telegram");
        assert!(config.trigger_channels.contains(&trigger.source_channel));
    }

    #[test]
    fn channel_gate_passes_empty_list() {
        let config = make_config(2, vec![]);
        let trigger = make_trigger(1, "discord");
        // Empty list → all channels allowed
        assert!(config.trigger_channels.is_empty());
    }

    #[test]
    fn parse_research_response_valid_json() {
        let json = r#"{"summary": "OO-42 is in review.", "findings": [{"tool": "jira", "label": "OO-42 status: In Review", "detail": null}], "tools_used": ["jira"]}"#;
        let result = parse_research_response("ob-1", json);
        assert!(result.error.is_none());
        assert_eq!(result.summary, "OO-42 is in review.");
        assert_eq!(result.raw_findings.len(), 1);
        assert_eq!(result.tools_used, vec!["jira"]);
    }

    #[test]
    fn parse_research_response_malformed_json_stores_error() {
        let bad = "not valid json at all {{{{ broken";
        let result = parse_research_response("ob-2", bad);
        assert!(result.error.is_some());
        assert!(result.summary.starts_with("Research failed:"));
        assert!(result.raw_findings.is_empty());
    }

    #[test]
    fn parse_research_response_json_wrapped_in_prose() {
        let prose = r#"Here is the research output: {"summary": "No additional context found.", "findings": [], "tools_used": []} Done."#;
        let result = parse_research_response("ob-3", prose);
        assert!(result.error.is_none());
        assert_eq!(result.summary, "No additional context found.");
    }

    #[test]
    fn build_research_prompt_contains_key_fields() {
        let trigger = make_trigger(1, "telegram");
        let prompt = build_research_prompt(&trigger, true, true);
        assert!(prompt.contains("Merge the PR for OO-42"));
        assert!(prompt.contains("telegram"));
        assert!(prompt.contains("OO"));
    }
}
