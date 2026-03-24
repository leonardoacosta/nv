//! Shared conversation history for API-level message persistence across worker invocations.
//!
//! Maintains a bounded list of (user, assistant) turn pairs with session expiry.
//! Tool result content blocks exceeding [`MAX_TOOL_RESULT_CHARS`] are truncated on push.

use std::time::{Duration, Instant};

use crate::claude::{ContentBlock, Message, MessageContent};

// ── Constants ────────────────────────────────────────────────────────

/// Maximum number of conversation turns (user+assistant pairs) to retain.
pub const MAX_HISTORY_TURNS: usize = 20;

/// Maximum total characters across all stored turns.
pub const MAX_HISTORY_CHARS: usize = 50_000;

/// Session timeout — clears history after this duration of inactivity.
pub const SESSION_TIMEOUT: Duration = Duration::from_secs(600);

/// Maximum characters for a single tool result content block before truncation.
const MAX_TOOL_RESULT_CHARS: usize = 1_000;

// ── ConversationStore ────────────────────────────────────────────────

/// Persists Claude API message pairs across worker invocations within a session.
///
/// Each turn is a (user_message, assistant_message) pair. Sessions expire after
/// [`SESSION_TIMEOUT`] of inactivity, at which point the store is cleared.
pub struct ConversationStore {
    turns: Vec<(Message, Message)>,
    last_activity: Instant,
}

impl ConversationStore {
    /// Create a new empty conversation store.
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            last_activity: Instant::now(),
        }
    }

    /// Push a completed turn (user + assistant messages) to the store.
    ///
    /// The assistant message has its tool result content blocks truncated to
    /// [`MAX_TOOL_RESULT_CHARS`]. After pushing, the store is trimmed to stay
    /// within [`MAX_HISTORY_TURNS`] and [`MAX_HISTORY_CHARS`].
    pub fn push(&mut self, user_msg: Message, assistant_msg: Message) {
        let assistant_msg = truncate_tool_results(assistant_msg);
        self.turns.push((user_msg, assistant_msg));
        self.last_activity = Instant::now();
        self.trim();
    }

    /// Load the conversation history as a flat list of messages.
    ///
    /// Returns an empty vec if the session has expired (no activity for
    /// [`SESSION_TIMEOUT`]). Touching this method updates `last_activity`.
    pub fn load(&mut self) -> Vec<Message> {
        if self.last_activity.elapsed() >= SESSION_TIMEOUT {
            self.turns.clear();
            self.last_activity = Instant::now();
            return Vec::new();
        }

        self.last_activity = Instant::now();
        self.turns
            .iter()
            .flat_map(|(user, assistant)| vec![user.clone(), assistant.clone()])
            .collect()
    }

    /// Number of stored turns.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.turns.len()
    }

    /// Trim turns to stay within bounds (turn count and character limit).
    fn trim(&mut self) {
        // Trim by turn count
        while self.turns.len() > MAX_HISTORY_TURNS {
            self.turns.remove(0);
        }

        // Trim by total character count
        while self.total_chars() > MAX_HISTORY_CHARS && !self.turns.is_empty() {
            self.turns.remove(0);
        }
    }

    /// Total characters across all stored turns.
    fn total_chars(&self) -> usize {
        self.turns
            .iter()
            .map(|(u, a)| u.content_len() + a.content_len())
            .sum()
    }
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Truncate tool result content blocks in a message to [`MAX_TOOL_RESULT_CHARS`].
fn truncate_tool_results(msg: Message) -> Message {
    let content = match msg.content {
        MessageContent::Blocks(blocks) => {
            let truncated_blocks = blocks
                .into_iter()
                .map(|block| match block {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let content = if content.len() > MAX_TOOL_RESULT_CHARS {
                            let mut end = MAX_TOOL_RESULT_CHARS;
                            while end > 0 && !content.is_char_boundary(end) {
                                end -= 1;
                            }
                            format!("{}...[truncated]", &content[..end])
                        } else {
                            content
                        };
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        }
                    }
                    other => other,
                })
                .collect();
            MessageContent::Blocks(truncated_blocks)
        }
        other => other,
    };

    Message {
        role: msg.role,
        content,
    }
}

// ── Context Window Management ────────────────────────────────────────

/// Truncate a flat conversation history list to stay within context budget.
///
/// Enforces both a turn count limit and a character budget.
/// Always keeps at least the 2 most recent turns.
pub(crate) fn truncate_history(history: &mut Vec<Message>) {
    // Keep at most MAX_HISTORY_TURNS turns
    if history.len() > MAX_HISTORY_TURNS {
        let drain_count = history.len() - MAX_HISTORY_TURNS;
        history.drain(..drain_count);
    }

    // If still too large by character count, drop oldest turns
    let mut total_chars: usize = history.iter().map(|m| m.content_len()).sum();

    while total_chars > MAX_HISTORY_CHARS && history.len() > 2 {
        if let Some(removed) = history.first() {
            total_chars -= removed.content_len();
        }
        history.remove(0);
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_user_msg(text: &str) -> Message {
        Message::user(text)
    }

    fn make_assistant_msg(text: &str) -> Message {
        Message {
            role: "assistant".into(),
            content: MessageContent::Blocks(vec![ContentBlock::Text {
                text: text.to_string(),
            }]),
        }
    }

    fn make_assistant_with_tool_result(tool_content: &str) -> Message {
        Message {
            role: "assistant".into(),
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "response".into(),
                },
                ContentBlock::ToolResult {
                    tool_use_id: "tool_1".into(),
                    content: tool_content.to_string(),
                    is_error: false,
                },
            ]),
        }
    }

    // ── Task 4.1: push/load/expire/trim ──────────────────────────────

    #[test]
    fn push_and_load_returns_turns() {
        let mut store = ConversationStore::new();
        store.push(make_user_msg("hello"), make_assistant_msg("hi"));
        store.push(make_user_msg("how are you"), make_assistant_msg("good"));

        let messages = store.load();
        assert_eq!(messages.len(), 4); // 2 turns * 2 messages
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
    }

    #[test]
    fn load_empty_store_returns_empty() {
        let mut store = ConversationStore::new();
        let messages = store.load();
        assert!(messages.is_empty());
    }

    #[test]
    fn trim_by_turn_count() {
        let mut store = ConversationStore::new();
        for i in 0..(MAX_HISTORY_TURNS + 5) {
            store.push(
                make_user_msg(&format!("msg {i}")),
                make_assistant_msg(&format!("reply {i}")),
            );
        }
        assert_eq!(store.len(), MAX_HISTORY_TURNS);
    }

    #[test]
    fn trim_by_char_limit() {
        let mut store = ConversationStore::new();
        // Each turn: ~10K chars user + ~10K chars assistant = ~20K
        let big_text = "x".repeat(10_000);
        for i in 0..10 {
            store.push(
                make_user_msg(&format!("{big_text}{i}")),
                make_assistant_msg(&format!("{big_text}{i}")),
            );
        }
        // Total should be trimmed to <= MAX_HISTORY_CHARS
        assert!(store.total_chars() <= MAX_HISTORY_CHARS);
        assert!(store.len() < 10);
    }

    #[test]
    fn session_expiry_clears_turns() {
        let mut store = ConversationStore::new();
        store.push(make_user_msg("hello"), make_assistant_msg("hi"));
        assert_eq!(store.len(), 1);

        // Simulate expired session by backdating last_activity
        store.last_activity = Instant::now() - SESSION_TIMEOUT - Duration::from_secs(1);

        let messages = store.load();
        assert!(messages.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn activity_resets_on_push() {
        let mut store = ConversationStore::new();
        store.last_activity = Instant::now() - Duration::from_secs(500);
        store.push(make_user_msg("hello"), make_assistant_msg("hi"));
        // push should have reset last_activity to now
        assert!(store.last_activity.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn activity_resets_on_load() {
        let mut store = ConversationStore::new();
        store.push(make_user_msg("hello"), make_assistant_msg("hi"));
        store.last_activity = Instant::now() - Duration::from_secs(300);
        let _messages = store.load();
        // load should have reset last_activity to now (session not expired)
        assert!(store.last_activity.elapsed() < Duration::from_secs(1));
    }

    // ── Task 4.2: tool_result truncation ─────────────────────────────

    #[test]
    fn tool_result_truncated_at_1000_chars() {
        let mut store = ConversationStore::new();
        let long_result = "y".repeat(2000);
        store.push(
            make_user_msg("query"),
            make_assistant_with_tool_result(&long_result),
        );

        let messages = store.load();
        let assistant = &messages[1];
        if let MessageContent::Blocks(blocks) = &assistant.content {
            for block in blocks {
                if let ContentBlock::ToolResult { content, .. } = block {
                    assert!(
                        content.len() < 2000,
                        "tool result should be truncated, got {} chars",
                        content.len()
                    );
                    assert!(content.ends_with("...[truncated]"));
                    // The truncated content before the marker should be <= MAX_TOOL_RESULT_CHARS
                    let marker = "...[truncated]";
                    let body = &content[..content.len() - marker.len()];
                    assert!(body.len() <= MAX_TOOL_RESULT_CHARS);
                }
            }
        } else {
            panic!("expected blocks content");
        }
    }

    #[test]
    fn short_tool_result_not_truncated() {
        let mut store = ConversationStore::new();
        let short_result = "short result";
        store.push(
            make_user_msg("query"),
            make_assistant_with_tool_result(short_result),
        );

        let messages = store.load();
        let assistant = &messages[1];
        if let MessageContent::Blocks(blocks) = &assistant.content {
            for block in blocks {
                if let ContentBlock::ToolResult { content, .. } = block {
                    assert_eq!(content, short_result);
                }
            }
        }
    }

    // ── Task 4.3: format_recent_for_context tests are in messages.rs ─
}
