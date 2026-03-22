/// Maximum Telegram message length.
const TELEGRAM_MAX_CHARS: usize = 4096;

/// Format a query answer for Telegram delivery.
///
/// Ensures the message fits within Telegram's 4096-char limit by
/// truncating source details first, then the main text if needed.
pub fn format_query_for_telegram(answer_text: &str) -> String {
    if answer_text.len() <= TELEGRAM_MAX_CHARS {
        return answer_text.to_string();
    }

    // Truncate to fit, preserving as much of the answer as possible
    let truncated = &answer_text[..TELEGRAM_MAX_CHARS - 30];
    // Find last newline to avoid cutting mid-line
    if let Some(pos) = truncated.rfind('\n') {
        format!("{}\n\n[Answer truncated]", &truncated[..pos])
    } else {
        format!("{}\n\n[Answer truncated]", truncated)
    }
}

/// Format a query answer for CLI output (plain text, no Telegram formatting).
pub fn format_query_for_cli(answer_text: &str) -> String {
    answer_text.to_string()
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_telegram_short_message() {
        let text = "Here's what's blocking OO:\n- OO-42: Login flow broken [Jira: OO-42]";
        let formatted = format_query_for_telegram(text);
        assert_eq!(formatted, text);
    }

    #[test]
    fn format_telegram_truncates_long_message() {
        let text = "x\n".repeat(3000); // ~6000 chars
        let formatted = format_query_for_telegram(&text);
        assert!(formatted.len() <= TELEGRAM_MAX_CHARS);
        assert!(formatted.ends_with("[Answer truncated]"));
    }

    #[test]
    fn format_cli_passthrough() {
        let text = "Answer with [Jira: OO-42] citations";
        let formatted = format_query_for_cli(text);
        assert_eq!(formatted, text);
    }
}
