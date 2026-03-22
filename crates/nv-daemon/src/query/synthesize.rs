use super::gather::GatheredContext;

/// Build a context block for Claude to synthesize an answer from gathered data.
///
/// This is injected into the agent loop as additional context when Claude
/// is processing a query. The agent loop's existing tool use mechanism
/// already handles synthesis -- this function formats the pre-gathered
/// context so Claude can produce a well-attributed answer.
pub fn format_gathered_context(ctx: &GatheredContext) -> String {
    let mut parts = Vec::new();

    if !ctx.jira_results.is_empty() && ctx.jira_results != "No issues found." {
        parts.push(format!(
            "<jira_data>\n{}\n</jira_data>",
            ctx.jira_results
        ));
    }

    if !ctx.memory_results.is_empty() && !ctx.memory_results.contains("No matches found") {
        parts.push(format!(
            "<memory_data>\n{}\n</memory_data>",
            ctx.memory_results
        ));
    }

    if !ctx.nexus_results.is_empty()
        && !ctx.nexus_results.contains("not connected")
        && !ctx.nexus_results.contains("not configured")
        && !ctx.nexus_results.contains("stub")
    {
        parts.push(format!(
            "<nexus_data>\n{}\n</nexus_data>",
            ctx.nexus_results
        ));
    }

    if !ctx.errors.is_empty() {
        parts.push(format!(
            "<data_errors>\n{}\n</data_errors>",
            ctx.errors.join("\n")
        ));
    }

    if parts.is_empty() {
        "No relevant data found from any source.".to_string()
    } else {
        parts.join("\n\n")
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_gathered_context_with_all_sources() {
        let ctx = GatheredContext {
            jira_results: "Found 2 issue(s):\n- OO-1 | Bug | Status: Open".into(),
            memory_results: "Found matches for \"OO\":\n[decisions:3]\nOO release planned".into(),
            nexus_results: "Session s-1 running: builder".into(),
            errors: vec![],
        };

        let formatted = format_gathered_context(&ctx);
        assert!(formatted.contains("<jira_data>"));
        assert!(formatted.contains("<memory_data>"));
        assert!(formatted.contains("<nexus_data>"));
        assert!(!formatted.contains("<data_errors>"));
    }

    #[test]
    fn format_gathered_context_with_errors() {
        let ctx = GatheredContext {
            jira_results: "Found 1 issue(s):\n- OO-1 | Bug".into(),
            memory_results: "No matches found for: xyz".into(),
            nexus_results: "Nexus not connected (integration pending).".into(),
            errors: vec!["Memory: search timed out".into()],
        };

        let formatted = format_gathered_context(&ctx);
        assert!(formatted.contains("<jira_data>"));
        assert!(!formatted.contains("<memory_data>")); // filtered: "No matches found"
        assert!(!formatted.contains("<nexus_data>")); // filtered: "not connected"
        assert!(formatted.contains("<data_errors>"));
    }

    #[test]
    fn format_gathered_context_empty() {
        let ctx = GatheredContext {
            jira_results: "No issues found.".into(),
            memory_results: "No matches found for: xyz".into(),
            nexus_results: "Nexus not connected (integration pending).".into(),
            errors: vec![],
        };

        let formatted = format_gathered_context(&ctx);
        assert_eq!(formatted, "No relevant data found from any source.");
    }
}
