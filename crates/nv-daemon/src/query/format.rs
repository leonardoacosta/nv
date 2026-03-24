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
    let truncated = crate::channels::util::safe_truncate(answer_text, TELEGRAM_MAX_CHARS - 30);
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

    /// Verify that format_query_for_telegram does not panic when the truncation
    /// point (TELEGRAM_MAX_CHARS - 30 = 4066 bytes) falls inside a multi-byte
    /// UTF-8 sequence.  Before safe_truncate was used, a raw byte slice
    /// `&answer_text[..4066]` would panic on a character boundary violation.
    #[test]
    fn format_telegram_does_not_panic_on_multibyte_cut_point() {
        // Each '中' is 3 UTF-8 bytes. Craft a string whose byte length exceeds
        // TELEGRAM_MAX_CHARS so truncation is needed, and arrange for the
        // cut point to fall inside a 3-byte sequence.
        //
        // Strategy: fill up to just past 4066 bytes with 3-byte chars so the
        // naive byte slice at 4066 would land mid-character.
        let repeat = 4096 / "中".len() + 10; // more than enough
        let text = "中".repeat(repeat);
        assert!(text.len() > TELEGRAM_MAX_CHARS, "test setup: text must exceed limit");

        // Must not panic.
        let result = format_query_for_telegram(&text);
        assert!(result.len() <= TELEGRAM_MAX_CHARS + 30, "result must fit within telegram limit (plus truncation suffix)");
    }
}
