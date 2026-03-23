use std::time::Duration;

use anyhow::Result;

use crate::calendar_tools;
use crate::jira;
use crate::memory::Memory;
use crate::nexus;

// ── Context Types ───────────────────────────────────────────────────

/// Aggregated context for digest synthesis.
#[derive(Debug, Clone)]
pub struct DigestContext {
    pub jira_issues: Vec<JiraDigestIssue>,
    pub nexus_sessions: Vec<SessionSummary>,
    pub memory_entries: Vec<MemoryEntry>,
    pub calendar_events: Vec<CalendarDigestEvent>,
    pub errors: Vec<String>,
}

/// Lightweight event summary for the digest prompt.
#[derive(Debug, Clone)]
pub struct CalendarDigestEvent {
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub attendees_count: usize,
    pub has_meeting_link: bool,
}

/// Simplified Jira issue for digest display.
#[derive(Debug, Clone)]
pub struct JiraDigestIssue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub priority: String,
    pub project: String,
    pub updated: String,
}

/// Nexus session summary (stub until spec-9).
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub agent_name: String,
    pub session_id: String,
    pub status: String,
}

/// Memory entry for digest context.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub topic: String,
    pub excerpt: String,
}

/// Timeout for individual context fetches.
const GATHER_TIMEOUT: Duration = Duration::from_secs(30);

// ── Gathering ───────────────────────────────────────────────────────

/// Gather context from all sources in parallel.
///
/// Each source has an independent 30-second timeout. Partial results are
/// accepted -- if Jira is down, the digest still includes memory + nexus.
/// Calendar failure logs a warning and produces an empty calendar section.
pub async fn gather_context(
    jira_client: Option<&jira::JiraClient>,
    memory: &Memory,
    nexus_client: Option<&nexus::client::NexusClient>,
    calendar_credentials: Option<&str>,
    calendar_id: &str,
) -> DigestContext {
    let (jira_result, memory_result, nexus_result, calendar_result) = tokio::join!(
        gather_jira(jira_client),
        gather_memory(memory),
        gather_nexus(nexus_client),
        gather_calendar(calendar_credentials, calendar_id),
    );

    let mut errors = Vec::new();

    let jira_issues = match jira_result {
        Ok(issues) => issues,
        Err(e) => {
            tracing::warn!(error = %e, "digest: Jira gather failed");
            errors.push(format!("Jira unavailable: {e}"));
            Vec::new()
        }
    };

    let memory_entries = match memory_result {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(error = %e, "digest: memory gather failed");
            errors.push(format!("Memory unavailable: {e}"));
            Vec::new()
        }
    };

    let nexus_sessions = match nexus_result {
        Ok(sessions) => sessions,
        Err(e) => {
            tracing::warn!(error = %e, "digest: nexus gather failed");
            errors.push(format!("Nexus unavailable: {e}"));
            Vec::new()
        }
    };

    let calendar_events = match calendar_result {
        Ok(events) => events,
        Err(e) => {
            tracing::warn!(error = %e, "digest: calendar gather failed");
            errors.push(format!("Calendar unavailable: {e}"));
            Vec::new()
        }
    };

    DigestContext {
        jira_issues,
        nexus_sessions,
        memory_entries,
        calendar_events,
        errors,
    }
}

/// Fetch open Jira issues assigned to the current user.
async fn gather_jira(
    jira_client: Option<&jira::JiraClient>,
) -> Result<Vec<JiraDigestIssue>> {
    let Some(client) = jira_client else {
        return Ok(Vec::new());
    };

    let jql = "assignee = currentUser() AND resolution = Unresolved ORDER BY priority ASC, updated DESC";

    let issues = tokio::time::timeout(GATHER_TIMEOUT, client.search(jql))
        .await
        .map_err(|_| anyhow::anyhow!("Jira search timed out after 30s"))??;

    Ok(issues
        .into_iter()
        .map(|issue| JiraDigestIssue {
            key: issue.key,
            summary: issue.fields.summary,
            status: issue.fields.status.name,
            priority: issue
                .fields
                .priority
                .map(|p| p.name)
                .unwrap_or_else(|| "None".into()),
            project: issue.fields.project.key,
            updated: issue.fields.updated,
        })
        .collect())
}

/// Fetch recent memory entries.
async fn gather_memory(memory: &Memory) -> Result<Vec<MemoryEntry>> {
    // Use tokio::time::timeout for consistency even though memory is sync
    let memory_result = tokio::time::timeout(GATHER_TIMEOUT, async {
        let topics = memory.list_topics()?;
        let mut entries = Vec::new();

        for topic in topics {
            if let Ok(content) = memory.read(&topic) {
                // Take last ~500 chars as excerpt
                let excerpt = if content.len() > 500 {
                    content[content.len() - 500..].to_string()
                } else {
                    content.clone()
                };
                if !excerpt.trim().is_empty() {
                    entries.push(MemoryEntry { topic, excerpt });
                }
            }
        }

        Ok::<Vec<MemoryEntry>, anyhow::Error>(entries)
    })
    .await
    .map_err(|_| anyhow::anyhow!("Memory gather timed out after 30s"))??;

    Ok(memory_result)
}

/// Gather Nexus session info from connected agents.
async fn gather_nexus(
    nexus_client: Option<&nexus::client::NexusClient>,
) -> Result<Vec<SessionSummary>> {
    let Some(client) = nexus_client else {
        return Ok(Vec::new());
    };

    let sessions = tokio::time::timeout(GATHER_TIMEOUT, client.query_sessions())
        .await
        .map_err(|_| anyhow::anyhow!("Nexus query timed out after 30s"))??;

    Ok(sessions
        .into_iter()
        .map(|s| SessionSummary {
            agent_name: s.agent_name,
            session_id: s.id,
            status: s.status,
        })
        .collect())
}

/// Fetch today's calendar events for the digest.
///
/// Returns empty vec (not an error) when calendar is not configured.
async fn gather_calendar(
    credentials_b64: Option<&str>,
    calendar_id: &str,
) -> Result<Vec<CalendarDigestEvent>> {
    if credentials_b64.is_none() {
        return Ok(Vec::new());
    }

    let events = tokio::time::timeout(
        GATHER_TIMEOUT,
        calendar_tools::gather_today_for_digest(credentials_b64, calendar_id),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Calendar gather timed out after 30s"))??;

    // Map from calendar_tools types to digest types
    Ok(events
        .into_iter()
        .map(|e| CalendarDigestEvent {
            title: e.title,
            start_time: e.start_time,
            end_time: e.end_time,
            attendees_count: e.attendees_count,
            has_meeting_link: e.has_meeting_link,
        })
        .collect())
}

// ── Format for Claude ───────────────────────────────────────────────

/// Format gathered context into a text block for the Claude digest prompt.
pub fn format_context_for_prompt(ctx: &DigestContext) -> String {
    let mut parts = Vec::new();

    // Jira section
    if ctx.jira_issues.is_empty() {
        parts.push("[Jira] No open issues assigned to you.".to_string());
    } else {
        let mut jira_section = format!("[Jira] {} open issues:\n", ctx.jira_issues.len());
        for issue in &ctx.jira_issues {
            jira_section.push_str(&format!(
                "- {} {} [{}] (priority: {}, project: {}, updated: {})\n",
                issue.key, issue.summary, issue.status, issue.priority, issue.project, issue.updated
            ));
        }
        parts.push(jira_section);
    }

    // Nexus section
    if ctx.nexus_sessions.is_empty() {
        parts.push("[Nexus] Not connected — no session data available.".to_string());
    } else {
        let mut nexus_section = format!(
            "[Nexus] {} active sessions:\n",
            ctx.nexus_sessions.len()
        );
        for session in &ctx.nexus_sessions {
            nexus_section.push_str(&format!(
                "- {} (session {}) — {}\n",
                session.agent_name, session.session_id, session.status
            ));
        }
        parts.push(nexus_section);
    }

    // Memory section
    if ctx.memory_entries.is_empty() {
        parts.push("[Memory] No recent entries.".to_string());
    } else {
        let mut mem_section = format!("[Memory] {} topics:\n", ctx.memory_entries.len());
        for entry in &ctx.memory_entries {
            // Truncate excerpt for the prompt
            let short = if entry.excerpt.len() > 200 {
                format!("{}...", &entry.excerpt[..200])
            } else {
                entry.excerpt.clone()
            };
            mem_section.push_str(&format!("- {}: {}\n", entry.topic, short.trim()));
        }
        parts.push(mem_section);
    }

    // Calendar section
    if !ctx.calendar_events.is_empty() {
        let mut cal_section = format!(
            "[Calendar] {} event{} today:\n",
            ctx.calendar_events.len(),
            if ctx.calendar_events.len() == 1 { "" } else { "s" }
        );
        for evt in &ctx.calendar_events {
            let link_indicator = if evt.has_meeting_link { " [meeting link]" } else { "" };
            let attendees = if evt.attendees_count > 0 {
                format!(" ({} attendee{})", evt.attendees_count, if evt.attendees_count == 1 { "" } else { "s" })
            } else {
                String::new()
            };
            cal_section.push_str(&format!(
                "- {}-{}  {}{}{}\n",
                evt.start_time, evt.end_time, evt.title, attendees, link_indicator
            ));
        }
        parts.push(cal_section);
    }

    // Errors section
    if !ctx.errors.is_empty() {
        let mut err_section = "[Errors]\n".to_string();
        for err in &ctx.errors {
            err_section.push_str(&format!("- {err}\n"));
        }
        parts.push(err_section);
    }

    parts.join("\n")
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_context_empty() {
        let ctx = DigestContext {
            jira_issues: vec![],
            nexus_sessions: vec![],
            memory_entries: vec![],
            calendar_events: vec![],
            errors: vec![],
        };
        let text = format_context_for_prompt(&ctx);
        assert!(text.contains("[Jira] No open issues"));
        assert!(text.contains("[Nexus] Not connected"));
        assert!(text.contains("[Memory] No recent entries"));
    }

    #[test]
    fn format_context_with_issues() {
        let ctx = DigestContext {
            jira_issues: vec![
                JiraDigestIssue {
                    key: "OO-42".into(),
                    summary: "Fix login flow".into(),
                    status: "In Progress".into(),
                    priority: "High".into(),
                    project: "OO".into(),
                    updated: "2026-03-21".into(),
                },
                JiraDigestIssue {
                    key: "OO-43".into(),
                    summary: "Add tests".into(),
                    status: "To Do".into(),
                    priority: "Medium".into(),
                    project: "OO".into(),
                    updated: "2026-03-20".into(),
                },
            ],
            nexus_sessions: vec![],
            memory_entries: vec![MemoryEntry {
                topic: "decisions".into(),
                excerpt: "Decided to use Stripe for payments".into(),
            }],
            calendar_events: vec![],
            errors: vec![],
        };
        let text = format_context_for_prompt(&ctx);
        assert!(text.contains("2 open issues"));
        assert!(text.contains("OO-42"));
        assert!(text.contains("OO-43"));
        assert!(text.contains("1 topics"));
        assert!(text.contains("decisions"));
        assert!(text.contains("Stripe"));
    }

    #[test]
    fn format_context_with_errors() {
        let ctx = DigestContext {
            jira_issues: vec![],
            nexus_sessions: vec![],
            memory_entries: vec![],
            calendar_events: vec![],
            errors: vec!["Jira unavailable: timeout".into()],
        };
        let text = format_context_for_prompt(&ctx);
        assert!(text.contains("[Errors]"));
        assert!(text.contains("Jira unavailable: timeout"));
    }

    #[tokio::test]
    async fn gather_context_no_clients() {
        // With no Jira client and no Nexus client, should still succeed with empty results
        let dir = tempfile::TempDir::new().unwrap();
        let memory = Memory::new(dir.path());
        memory.init().unwrap();

        let ctx = gather_context(None, &memory, None, None, "primary").await;
        assert!(ctx.jira_issues.is_empty());
        assert!(ctx.nexus_sessions.is_empty());
        // Memory should have default topics
        assert!(!ctx.memory_entries.is_empty());
        assert!(ctx.errors.is_empty());
    }
}
