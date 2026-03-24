// UTF-8-safe string truncation and message chunking utilities shared across
// channel adapters.

// ── Safe Truncate ────────────────────────────────────────────────────

/// Truncate `s` to at most `max_bytes` bytes without splitting a multi-byte
/// UTF-8 character.
///
/// Uses `char_indices` to walk forward and find the largest byte offset that
/// is both a valid char boundary and does not exceed `max_bytes`. Returns a
/// subslice of `s`; the original bytes are not copied.
///
/// # Examples
///
/// ```
/// use nv_daemon::channels::util::safe_truncate;
///
/// // ASCII: no-op
/// assert_eq!(safe_truncate("hello", 10), "hello");
///
/// // Exact boundary
/// assert_eq!(safe_truncate("hello", 5), "hello");
///
/// // Truncation at ASCII char boundary
/// assert_eq!(safe_truncate("hello", 3), "hel");
///
/// // Emoji is 4 bytes — must not be split
/// let s = "\u{1F600}"; // 😀
/// assert_eq!(safe_truncate(s, 3), "");
/// assert_eq!(safe_truncate(s, 4), "\u{1F600}");
/// ```
pub fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Walk char boundaries from the start; keep the largest boundary <= max_bytes.
    let mut last_boundary = 0;
    for (byte_idx, _ch) in s.char_indices() {
        if byte_idx > max_bytes {
            break;
        }
        last_boundary = byte_idx;
    }
    // Handle the case where the very first char already exceeds max_bytes:
    // last_boundary stays 0, so we return "".
    &s[..last_boundary]
}

// ── Chunk Message ────────────────────────────────────────────────────

/// Split a message into chunks that fit within `max_len` bytes.
///
/// Prefers splitting at paragraph boundaries (`\n\n`), then line boundaries
/// (`\n`), and falls back to a hard cut at `max_len`. This is the single
/// canonical implementation shared by Discord and Telegram channel adapters.
pub fn chunk_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        // Find split point: prefer paragraph break, then line break, then hard cut
        let split_at = remaining[..max_len]
            .rfind("\n\n")
            .or_else(|| remaining[..max_len].rfind('\n'))
            .unwrap_or(max_len);

        // Avoid zero-length splits
        let split_at = if split_at == 0 { max_len } else { split_at };

        chunks.push(remaining[..split_at].to_string());
        remaining = remaining[split_at..].trim_start();
    }

    chunks
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── safe_truncate ──────────────────────────────────────────────

    #[test]
    fn safe_truncate_short_string_unchanged() {
        assert_eq!(safe_truncate("hello", 10), "hello");
    }

    #[test]
    fn safe_truncate_empty_string() {
        assert_eq!(safe_truncate("", 10), "");
    }

    #[test]
    fn safe_truncate_exact_boundary() {
        assert_eq!(safe_truncate("hello", 5), "hello");
    }

    #[test]
    fn safe_truncate_ascii_truncation() {
        assert_eq!(safe_truncate("hello world", 5), "hello");
    }

    #[test]
    fn safe_truncate_emoji_not_split() {
        // 😀 is U+1F600, encoded as 4 bytes: F0 9F 98 80
        let emoji = "\u{1F600}";
        assert_eq!(emoji.len(), 4);
        // Cutting at 1, 2, or 3 bytes would land mid-character — must return ""
        assert_eq!(safe_truncate(emoji, 1), "");
        assert_eq!(safe_truncate(emoji, 2), "");
        assert_eq!(safe_truncate(emoji, 3), "");
        // 4 bytes is the full character
        assert_eq!(safe_truncate(emoji, 4), "\u{1F600}");
    }

    #[test]
    fn safe_truncate_multibyte_boundary() {
        // "café" = c(1) + a(1) + f(1) + é(2) = 5 bytes
        let s = "café";
        assert_eq!(s.len(), 5);
        // Truncating at 4 bytes would split é (which starts at byte 3)
        assert_eq!(safe_truncate(s, 4), "caf");
        assert_eq!(safe_truncate(s, 5), "café");
    }

    #[test]
    fn safe_truncate_mixed_ascii_and_multibyte() {
        // "AB😀CD" = 2 + 4 + 2 = 8 bytes
        let s = "AB\u{1F600}CD";
        assert_eq!(safe_truncate(s, 6), "AB\u{1F600}"); // 2+4 = 6 — exact
        assert_eq!(safe_truncate(s, 5), "AB");           // 2+4 > 5 — drop emoji
        assert_eq!(safe_truncate(s, 7), "AB\u{1F600}C");
    }

    // ── chunk_message ─────────────────────────────────────────────

    #[test]
    fn chunk_short_message_single_chunk() {
        let chunks = chunk_message("Hello, world!", 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello, world!");
    }

    #[test]
    fn chunk_empty_message() {
        let chunks = chunk_message("", 2000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }

    #[test]
    fn chunk_splits_at_paragraph() {
        let para1 = "A".repeat(1500);
        let para2 = "B".repeat(1500);
        let text = format!("{para1}\n\n{para2}");
        let chunks = chunk_message(&text, 2000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], para1);
        assert_eq!(chunks[1], para2);
    }

    #[test]
    fn chunk_splits_at_line() {
        let line1 = "A".repeat(1500);
        let line2 = "B".repeat(1500);
        let text = format!("{line1}\n{line2}");
        let chunks = chunk_message(&text, 2000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], line1);
        assert_eq!(chunks[1], line2);
    }

    #[test]
    fn chunk_hard_cut_no_breaks() {
        let text = "A".repeat(5000);
        let chunks = chunk_message(&text, 2000);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 2000);
        assert_eq!(chunks[1].len(), 2000);
        assert_eq!(chunks[2].len(), 1000);
    }
}
