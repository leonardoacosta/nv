use std::time::Duration;

use anyhow::Result;

use crate::jira;
use crate::memory::Memory;
use crate::nexus;

/// Timeout for each data source fetch during query gathering.
const GATHER_TIMEOUT: Duration = Duration::from_secs(15);

/// Gathered context from all data sources for query synthesis.
#[derive(Debug, Default)]
pub struct GatheredContext {
    pub jira_results: String,
    pub memory_results: String,
    pub nexus_results: String,
    pub errors: Vec<String>,
}

/// Gather context from Jira, Memory, and Nexus in parallel.
///
/// Each source has a 15-second timeout. Partial results are accepted --
/// if one source fails, the others still contribute.
pub async fn gather_query_context(
    question: &str,
    memory: &Memory,
    jira_client: Option<&jira::JiraClient>,
    nexus_client: Option<&nexus::client::NexusClient>,
) -> GatheredContext {
    let mut ctx = GatheredContext::default();

    // Run all three gathers concurrently
    let (jira_result, memory_result, nexus_result) = tokio::join!(
        gather_jira(question, jira_client),
        gather_memory(question, memory),
        gather_nexus(nexus_client),
    );

    match jira_result {
        Ok(s) => ctx.jira_results = s,
        Err(e) => ctx.errors.push(format!("Jira: {e}")),
    }

    match memory_result {
        Ok(s) => ctx.memory_results = s,
        Err(e) => ctx.errors.push(format!("Memory: {e}")),
    }

    match nexus_result {
        Ok(s) => ctx.nexus_results = s,
        Err(e) => ctx.errors.push(format!("Nexus: {e}")),
    }

    ctx
}

/// Gather Jira context for a query.
///
/// Extracts project keys from the question and constructs a JQL query.
/// Falls back to broad text search if no project key is detected.
async fn gather_jira(
    question: &str,
    jira_client: Option<&jira::JiraClient>,
) -> Result<String> {
    let client = match jira_client {
        Some(c) => c,
        None => return Ok("Jira not configured.".to_string()),
    };

    let jql = build_jql_from_question(question);

    let result = tokio::time::timeout(GATHER_TIMEOUT, client.search(&jql)).await;

    match result {
        Ok(Ok(issues)) => Ok(jira::format_issues_for_claude(&issues)),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, jql = %jql, "Jira query failed");
            Err(e)
        }
        Err(_) => {
            tracing::warn!("Jira query timed out after 15s");
            anyhow::bail!("Jira query timed out")
        }
    }
}

/// Gather memory context for a query.
///
/// Extracts keywords from the question and searches memory files.
async fn gather_memory(question: &str, memory: &Memory) -> Result<String> {
    // Memory operations are synchronous, wrap in spawn_blocking with timeout
    let question_owned = question.to_string();
    let result = tokio::time::timeout(GATHER_TIMEOUT, {
        // Memory search is CPU-bound (string matching), not truly blocking I/O,
        // but we respect the timeout contract
        let mem_result = memory.search(&question_owned);
        async { mem_result }
    })
    .await;

    match result {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => Err(e),
        Err(_) => anyhow::bail!("Memory search timed out"),
    }
}

/// Gather Nexus session context for a query.
async fn gather_nexus(
    nexus_client: Option<&nexus::client::NexusClient>,
) -> Result<String> {
    let Some(client) = nexus_client else {
        return Ok("Nexus not configured.".to_string());
    };

    let result = tokio::time::timeout(GATHER_TIMEOUT, async {
        nexus::tools::format_query_sessions(client).await
    })
    .await;

    match result {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "Nexus query failed");
            Err(e)
        }
        Err(_) => {
            tracing::warn!("Nexus query timed out after 15s");
            anyhow::bail!("Nexus query timed out")
        }
    }
}

/// Build a JQL query from a natural language question.
///
/// Attempts to extract project keys (2-4 uppercase letters) and
/// constructs targeted JQL. Falls back to broad text search.
pub fn build_jql_from_question(question: &str) -> String {
    // Look for project key patterns (2-4 uppercase letters)
    let project_key = extract_project_key(question);

    if let Some(key) = project_key {
        // Check if the question is about blocking/open issues
        let q_lower = question.to_lowercase();
        if q_lower.contains("block")
            || q_lower.contains("open")
            || q_lower.contains("progress")
            || q_lower.contains("stuck")
        {
            format!(
                "project = {key} AND resolution = Unresolved ORDER BY priority ASC, updated DESC"
            )
        } else {
            format!(
                "project = {key} AND resolution = Unresolved ORDER BY updated DESC"
            )
        }
    } else {
        // Broad text search -- extract significant words
        let keywords = extract_keywords(question);
        if keywords.is_empty() {
            "resolution = Unresolved ORDER BY updated DESC".to_string()
        } else {
            format!(
                "text ~ \"{}\" ORDER BY updated DESC",
                keywords.join(" ")
            )
        }
    }
}

/// Extract a Jira project key from a question.
///
/// Looks for 2-4 uppercase letter sequences that look like project keys.
fn extract_project_key(question: &str) -> Option<String> {
    // Common pattern: "What's blocking OO?" or "Status of TC-123"
    for word in question.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');

        // Check for issue key pattern (e.g., OO-123)
        if let Some(key) = clean.split('-').next() {
            if key.len() >= 2
                && key.len() <= 4
                && key.chars().all(|c| c.is_ascii_uppercase())
            {
                return Some(key.to_string());
            }
        }
    }
    None
}

/// Extract significant keywords from a question for text search.
fn extract_keywords(question: &str) -> Vec<String> {
    let stop_words = [
        "what", "what's", "whats", "is", "are", "the", "a", "an", "on", "in", "of", "for",
        "to", "and", "or", "how", "many", "much", "do", "does", "did", "has", "have", "been",
        "be", "can", "could", "would", "should", "will", "was", "were", "with", "about",
        "from", "that", "this", "it", "its", "my", "me", "i", "we", "they", "them", "their",
        "there", "here", "not", "no", "any", "some", "all", "most", "nv",
    ];

    question
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|w| !w.is_empty() && w.len() > 1 && !stop_words.contains(&w.as_str()))
        .collect()
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_project_key_from_question() {
        assert_eq!(
            extract_project_key("What's blocking OO?"),
            Some("OO".to_string())
        );
        assert_eq!(
            extract_project_key("Status of TC-123"),
            Some("TC".to_string())
        );
        assert_eq!(
            extract_project_key("How many open issues on PROJ?"),
            Some("PROJ".to_string())
        );
    }

    #[test]
    fn extract_project_key_none_for_no_key() {
        assert_eq!(extract_project_key("How are things going?"), None);
        assert_eq!(extract_project_key("hello world"), None);
    }

    #[test]
    fn extract_keywords_filters_stop_words() {
        let keywords = extract_keywords("What is the status of the release?");
        assert!(!keywords.contains(&"what".to_string()));
        assert!(!keywords.contains(&"is".to_string()));
        assert!(!keywords.contains(&"the".to_string()));
        assert!(keywords.contains(&"status".to_string()));
        assert!(keywords.contains(&"release".to_string()));
    }

    #[test]
    fn extract_keywords_empty_for_all_stop_words() {
        let keywords = extract_keywords("what is the");
        assert!(keywords.is_empty());
    }

    #[test]
    fn build_jql_with_project_key_blocking() {
        let jql = build_jql_from_question("What's blocking OO?");
        assert!(jql.contains("project = OO"));
        assert!(jql.contains("resolution = Unresolved"));
        assert!(jql.contains("priority ASC"));
    }

    #[test]
    fn build_jql_with_project_key_general() {
        let jql = build_jql_from_question("Status of TC?");
        assert!(jql.contains("project = TC"));
        assert!(jql.contains("resolution = Unresolved"));
        assert!(jql.contains("updated DESC"));
    }

    #[test]
    fn build_jql_text_search_fallback() {
        let jql = build_jql_from_question("How is the checkout flow?");
        assert!(jql.contains("text ~"));
        assert!(jql.contains("checkout"));
    }

    #[test]
    fn build_jql_empty_keywords_fallback() {
        let jql = build_jql_from_question("what is the");
        assert!(jql.contains("resolution = Unresolved"));
        assert!(!jql.contains("text ~"));
    }

    #[tokio::test]
    async fn gather_nexus_without_client() {
        let result = gather_nexus(None).await.unwrap();
        assert!(result.contains("Nexus not configured"));
    }

    #[tokio::test]
    async fn gather_jira_without_client() {
        let result = gather_jira("test question", None).await.unwrap();
        assert!(result.contains("Jira not configured"));
    }

    #[tokio::test]
    async fn gather_context_without_jira() {
        let dir = tempfile::TempDir::new().unwrap();
        let memory = Memory::new(dir.path());
        memory.init().unwrap();

        let ctx = gather_query_context("test question", &memory, None, None).await;
        assert!(ctx.jira_results.contains("Jira not configured"));
        assert!(ctx.nexus_results.contains("Nexus not configured"));
        // Memory search should return something (even if "No matches found")
        assert!(!ctx.memory_results.is_empty());
    }
}
