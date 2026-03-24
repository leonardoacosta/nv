use anyhow::Result;

use crate::claude::{ApiResponse, ClaudeClient, ContentBlock, Message, ToolDefinition};

use super::gather::{DigestContext, format_context_for_prompt};
use super::state::SuggestedAction;

// ── Digest Result ───────────────────────────────────────────────────

/// The result of synthesizing a digest through Claude.
#[derive(Debug, Clone)]
pub struct DigestResult {
    /// The full text content of the digest.
    pub content: String,
    /// Suggested actions extracted from Claude's response.
    pub suggested_actions: Vec<SuggestedAction>,
}

// ── System Prompt ───────────────────────────────────────────────────

const DIGEST_SYSTEM_PROMPT: &str = r#"You are NV generating a periodic digest for Leo. Given the following context from multiple sources, produce a structured summary.

## Output Format

Produce a text digest with these sections:

### Jira
- List open issues grouped by priority
- Highlight any P0/P1 (Highest/High) items with a warning indicator
- Flag issues untouched for more than 3 days as stale
- If no issues: "Nothing to report"

### Sessions
- List any running or recently completed Nexus sessions
- Note any errors or failures
- If no sessions: "No active sessions"

### Memory
- Notable recent decisions or context
- Items flagged for follow-up
- If nothing recent: "Nothing new"

### Suggested Actions
- List 3-5 actionable items based on the gathered context
- Each action should be concrete and specific (e.g., "Close OO-142 — resolved yesterday")
- Format each as: ACTION_ID: description

## Rules
- Be concise — this is a quick scan, not a report
- Prioritize actionable information
- Use plain text, no markdown formatting
- Keep total length under 3000 characters"#;

// ── Synthesis ───────────────────────────────────────────────────────

/// Build a digest by sending gathered context through Claude for synthesis.
///
/// Returns the synthesized digest text and any suggested actions.
pub async fn synthesize_digest(
    client: &ClaudeClient,
    context: &DigestContext,
) -> Result<DigestResult> {
    let context_text = format_context_for_prompt(context);

    let user_message = format!(
        "Generate a digest from the following context:\n\n{context_text}"
    );

    let messages = vec![Message::user(user_message)];
    let tools: Vec<ToolDefinition> = vec![]; // No tools needed for digest synthesis

    let response = client
        .send_messages_with_options(DIGEST_SYSTEM_PROMPT, &messages, &tools, Some(1024))
        .await?;

    let content = extract_text_content(&response);

    if content.is_empty() {
        anyhow::bail!("Claude returned empty digest");
    }

    // Parse suggested actions from the content
    let suggested_actions = parse_suggested_actions(&content);

    Ok(DigestResult {
        content,
        suggested_actions,
    })
}

/// Append a budget warning line to the digest content if applicable.
///
/// Called after synthesis — injects a warning when cost exceeds 80% of budget.
pub fn inject_budget_warning(result: &mut DigestResult, budget_line: &str) {
    result.content.push_str("\n\n");
    result.content.push_str(budget_line);
}

/// Build a digest locally without calling Claude (fallback for when Claude is unavailable).
///
/// Produces a simpler, template-based digest from the raw context.
pub fn synthesize_digest_fallback(context: &DigestContext) -> DigestResult {
    let mut parts = Vec::new();

    // Jira section
    parts.push("-- Jira --".to_string());
    if context.jira_issues.is_empty() {
        parts.push("Nothing to report".to_string());
    } else {
        for issue in &context.jira_issues {
            let priority_marker = match issue.priority.as_str() {
                "Highest" | "High" => "(!)",
                _ => "",
            };
            parts.push(format!(
                "{} {} {} [{}] {}",
                priority_marker, issue.key, issue.summary, issue.status, issue.priority
            ));
        }
    }

    // Sessions section
    parts.push(String::new());
    parts.push("-- Sessions --".to_string());
    if context.nexus_sessions.is_empty() {
        parts.push("No active sessions".to_string());
    } else {
        for session in &context.nexus_sessions {
            parts.push(format!(
                "{} ({}) — {}",
                session.agent_name, session.session_id, session.status
            ));
        }
    }

    // Memory section
    parts.push(String::new());
    parts.push("-- Memory --".to_string());
    if context.memory_entries.is_empty() {
        parts.push("Nothing new".to_string());
    } else {
        for entry in &context.memory_entries {
            let short = if entry.excerpt.len() > 100 {
                format!("{}...", &entry.excerpt[..100])
            } else {
                entry.excerpt.clone()
            };
            parts.push(format!("{}: {}", entry.topic, short.trim()));
        }
    }

    // Errors
    if !context.errors.is_empty() {
        parts.push(String::new());
        parts.push("-- Errors --".to_string());
        for err in &context.errors {
            parts.push(err.clone());
        }
    }

    DigestResult {
        content: parts.join("\n"),
        suggested_actions: vec![],
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Extract text content from a Claude API response.
fn extract_text_content(response: &ApiResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse suggested actions from digest text.
///
/// Looks for lines matching the pattern "ACT_N: description" in the
/// Suggested Actions section.
fn parse_suggested_actions(content: &str) -> Vec<SuggestedAction> {
    use super::state::{DigestActionStatus, DigestActionType};

    let mut actions = Vec::new();
    let mut in_actions_section = false;
    let mut action_counter = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect the Suggested Actions section header
        if trimmed.contains("Suggested Actions") || trimmed.contains("suggested actions") {
            in_actions_section = true;
            continue;
        }

        // End section on a new section header
        if in_actions_section && trimmed.starts_with("--") && !trimmed.contains("Suggested") {
            break;
        }

        if !in_actions_section {
            continue;
        }

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Parse action lines — either "ACT_N: description" or "- description"
        let label = if let Some(rest) = trimmed.strip_prefix("- ") {
            rest.to_string()
        } else if trimmed.contains(':') {
            trimmed
                .split_once(':')
                .map(|x| x.1)
                .unwrap_or(trimmed)
                .trim()
                .to_string()
        } else {
            continue;
        };

        if label.is_empty() {
            continue;
        }

        action_counter += 1;
        actions.push(SuggestedAction {
            id: format!("digest_act_{action_counter}"),
            label: label.clone(),
            action_type: DigestActionType::FollowUpQuery, // Default; could be refined
            payload: serde_json::json!({"description": label}),
            status: DigestActionStatus::Pending,
        });

        if actions.len() >= 5 {
            break;
        }
    }

    actions
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::gather::{
        DigestContext, JiraDigestIssue, MemoryEntry, SessionSummary,
    };

    #[test]
    fn fallback_digest_empty_context() {
        let ctx = DigestContext {
            jira_issues: vec![],
            nexus_sessions: vec![],
            memory_entries: vec![],
            errors: vec![],
            calendar_events: vec![],
        };
        let result = synthesize_digest_fallback(&ctx);
        assert!(result.content.contains("-- Jira --"));
        assert!(result.content.contains("Nothing to report"));
        assert!(result.content.contains("-- Sessions --"));
        assert!(result.content.contains("No active sessions"));
        assert!(result.content.contains("-- Memory --"));
        assert!(result.content.contains("Nothing new"));
        assert!(result.suggested_actions.is_empty());
    }

    #[test]
    fn fallback_digest_with_issues() {
        let ctx = DigestContext {
            jira_issues: vec![
                JiraDigestIssue {
                    key: "OO-42".into(),
                    summary: "Fix login".into(),
                    status: "In Progress".into(),
                    priority: "Highest".into(),
                    project: "OO".into(),
                    updated: "2026-03-21".into(),
                },
                JiraDigestIssue {
                    key: "OO-43".into(),
                    summary: "Add docs".into(),
                    status: "To Do".into(),
                    priority: "Low".into(),
                    project: "OO".into(),
                    updated: "2026-03-20".into(),
                },
            ],
            nexus_sessions: vec![],
            memory_entries: vec![],
            errors: vec![],
            calendar_events: vec![],
        };
        let result = synthesize_digest_fallback(&ctx);
        assert!(result.content.contains("(!) OO-42"));
        assert!(result.content.contains("OO-43"));
        assert!(!result.content.contains("(!) OO-43")); // Low priority, no marker
    }

    #[test]
    fn fallback_digest_with_errors() {
        let ctx = DigestContext {
            jira_issues: vec![],
            nexus_sessions: vec![],
            memory_entries: vec![],
            errors: vec!["Jira unavailable: timeout".into()],
            calendar_events: vec![],
        };
        let result = synthesize_digest_fallback(&ctx);
        assert!(result.content.contains("-- Errors --"));
        assert!(result.content.contains("Jira unavailable: timeout"));
    }

    #[test]
    fn parse_actions_from_content() {
        let content = r#"-- Jira --
OO-42 Fix login [In Progress]

-- Suggested Actions --
- Close OO-142 — resolved yesterday
- Review PR #234 for frontend changes
- Update sprint board — stale items need triage

-- End --"#;
        let actions = parse_suggested_actions(content);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].id, "digest_act_1");
        assert!(actions[0].label.contains("Close OO-142"));
        assert!(actions[1].label.contains("Review PR"));
        assert!(actions[2].label.contains("Update sprint board"));
    }

    #[test]
    fn parse_actions_max_five() {
        let content = "-- Suggested Actions --\n\
            - One\n- Two\n- Three\n- Four\n- Five\n- Six\n- Seven\n";
        let actions = parse_suggested_actions(content);
        assert_eq!(actions.len(), 5);
    }

    #[test]
    fn parse_actions_empty() {
        let content = "-- Jira --\nNothing to report\n";
        let actions = parse_suggested_actions(content);
        assert!(actions.is_empty());
    }

    #[test]
    fn fallback_digest_with_sessions() {
        let ctx = DigestContext {
            jira_issues: vec![],
            nexus_sessions: vec![SessionSummary {
                agent_name: "builder".into(),
                session_id: "s-1".into(),
                status: "running".into(),
            }],
            memory_entries: vec![],
            errors: vec![],
            calendar_events: vec![],
        };
        let result = synthesize_digest_fallback(&ctx);
        assert!(result.content.contains("builder"));
        assert!(result.content.contains("s-1"));
        assert!(result.content.contains("running"));
    }

    #[test]
    fn fallback_digest_with_memory() {
        let ctx = DigestContext {
            jira_issues: vec![],
            nexus_sessions: vec![],
            memory_entries: vec![MemoryEntry {
                topic: "decisions".into(),
                excerpt: "Use Stripe for payments".into(),
            }],
            errors: vec![],
            calendar_events: vec![],
        };
        let result = synthesize_digest_fallback(&ctx);
        assert!(result.content.contains("decisions: Use Stripe"));
    }
}
