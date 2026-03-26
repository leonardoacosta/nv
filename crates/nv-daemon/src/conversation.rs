//! SQLite-backed conversation history with TTL expiry and per-thread isolation.
//!
//! Replaces the former in-memory `ConversationStore`.  Each conversation is
//! keyed by `(channel, thread_id)` and stored as a JSON blob of
//! `(Message, Message)` pairs.  Rows older than `ttl_hours` are treated as
//! expired and deleted on first access.
//!
//! Thread isolation guarantees that conversations from different channels or
//! different threads within the same channel do not bleed into each other.
//!
//! Tool result content blocks exceeding [`MAX_TOOL_RESULT_CHARS`] are
//! truncated on push.  When the serialized history exceeds
//! [`MAX_HISTORY_CHARS`], the oldest turns are dropped.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::claude::{ContentBlock, Message, MessageContent};

// ── Constants ────────────────────────────────────────────────────────

/// Maximum number of conversation turns (user+assistant pairs) to retain.
pub const MAX_HISTORY_TURNS: usize = 20;

/// Maximum total characters across all stored turns.
pub const MAX_HISTORY_CHARS: usize = 50_000;

/// Maximum characters for a single tool result content block before truncation.
const MAX_TOOL_RESULT_CHARS: usize = 1_000;

/// Character budget above which the oldest turns are summarised / dropped.
const SUMMARY_BUDGET_CHARS: usize = 40_000;

// ── PersistentConversationStore ──────────────────────────────────────

/// SQLite-backed conversation store scoped to one `(channel, thread_id)` pair.
///
/// Construct one per worker invocation with the channel and thread_id derived
/// from the incoming trigger.  The underlying `Connection` is shared via an
/// `Arc<Mutex<Connection>>` — WAL mode allows concurrent reads from separate
/// connections, but we share a single connection here because `rusqlite` is
/// not `Send` across threads without wrapping.
pub struct PersistentConversationStore {
    conn: Arc<Mutex<Connection>>,
    /// Source channel name (e.g. `"telegram"`, `"discord"`).
    channel: String,
    /// Thread or conversation identifier within the channel.
    thread_id: String,
    /// Hours before a row is considered expired (0 = never expire).
    ttl_hours: u64,
}

impl PersistentConversationStore {
    /// Create a new store scoped to `(channel, thread_id)`.
    pub fn new(
        conn: Arc<Mutex<Connection>>,
        channel: impl Into<String>,
        thread_id: impl Into<String>,
        ttl_hours: u64,
    ) -> Self {
        Self {
            conn,
            channel: channel.into(),
            thread_id: thread_id.into(),
            ttl_hours,
        }
    }

    /// Push a completed turn (user + assistant messages) to the store.
    ///
    /// Steps:
    /// 1. Truncate tool results in the assistant message.
    /// 2. Load existing turns from SQLite.
    /// 3. Append the new turn.
    /// 4. Apply character-budget summarization (drop oldest if over budget).
    /// 5. Trim to `MAX_HISTORY_TURNS`.
    /// 6. Upsert back to SQLite.
    pub fn push(&self, user_msg: Message, assistant_msg: Message) -> Result<()> {
        let assistant_msg = truncate_tool_results(assistant_msg);

        let mut turns = self.load_turns()?;
        turns.push((user_msg, assistant_msg));
        turns = apply_budget(turns);
        trim_turns(&mut turns);

        let json = serde_json::to_string(&turns)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations (channel, thread_id, turns_json, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(channel, thread_id) DO UPDATE SET
               turns_json = excluded.turns_json,
               updated_at = excluded.updated_at",
            params![self.channel, self.thread_id, json],
        )?;
        Ok(())
    }

    /// Load the conversation history as a flat list of messages.
    ///
    /// Returns an empty vec if:
    /// - The row does not exist yet.
    /// - The row has expired (`updated_at` is older than `ttl_hours`).
    ///
    /// Expired rows are deleted from the database.
    pub fn load(&self) -> Result<Vec<Message>> {
        let turns = self.load_turns()?;
        Ok(turns
            .into_iter()
            .flat_map(|(user, assistant)| vec![user, assistant])
            .collect())
    }

    /// Delete the row for this `(channel, thread_id)`.
    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM conversations WHERE channel = ?1 AND thread_id = ?2",
            params![self.channel, self.thread_id],
        )?;
        Ok(())
    }

    // ── Private helpers ──────────────────────────────────────────────

    /// Load turns from SQLite, honouring TTL expiry.
    fn load_turns(&self) -> Result<Vec<(Message, Message)>> {
        let conn = self.conn.lock().unwrap();

        // Build the expiry condition string — SQLite datetime arithmetic.
        let ttl_clause = if self.ttl_hours > 0 {
            format!(
                " AND updated_at >= datetime('now', '-{} hours')",
                self.ttl_hours
            )
        } else {
            String::new()
        };

        let query = format!(
            "SELECT turns_json FROM conversations \
             WHERE channel = ?1 AND thread_id = ?2{}",
            ttl_clause
        );

        let result: Option<String> = conn
            .query_row(&query, params![self.channel, self.thread_id], |row| {
                row.get(0)
            })
            .optional()?;

        match result {
            Some(json) => {
                let turns: Vec<(Message, Message)> = serde_json::from_str(&json)?;
                Ok(turns)
            }
            None => {
                // Either missing or expired — delete any stale row and return empty.
                drop(conn); // release lock before re-acquiring for delete
                let conn2 = self.conn.lock().unwrap();
                conn2.execute(
                    "DELETE FROM conversations WHERE channel = ?1 AND thread_id = ?2",
                    params![self.channel, self.thread_id],
                )?;
                Ok(Vec::new())
            }
        }
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

/// Drop oldest turns until total characters fall below [`SUMMARY_BUDGET_CHARS`].
///
/// Always retains at least 1 turn (the most recent).
fn apply_budget(mut turns: Vec<(Message, Message)>) -> Vec<(Message, Message)> {
    let mut total_chars: usize = turns
        .iter()
        .map(|(u, a)| u.content_len() + a.content_len())
        .sum();

    while total_chars > SUMMARY_BUDGET_CHARS && turns.len() > 1 {
        let (u, a) = turns.remove(0);
        total_chars -= u.content_len() + a.content_len();
    }

    turns
}

/// Trim turns to stay within [`MAX_HISTORY_TURNS`].
fn trim_turns(turns: &mut Vec<(Message, Message)>) {
    while turns.len() > MAX_HISTORY_TURNS {
        turns.remove(0);
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
    use rusqlite::Connection;

    // ── Test helpers ─────────────────────────────────────────────────

    fn open_test_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // Run the conversations migration directly (no full migration chain needed).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                channel     TEXT NOT NULL,
                thread_id   TEXT NOT NULL,
                turns_json  TEXT NOT NULL DEFAULT '[]',
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (channel, thread_id)
            );
            CREATE INDEX IF NOT EXISTS idx_conversations_updated ON conversations(updated_at);",
        )
        .expect("create table");
        Arc::new(Mutex::new(conn))
    }

    fn make_store(conn: Arc<Mutex<Connection>>) -> PersistentConversationStore {
        PersistentConversationStore::new(conn, "telegram", "chat-1", 24)
    }

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

    // ── Task 6.1: round-trip ─────────────────────────────────────────

    #[test]
    fn push_and_load_round_trip() {
        let conn = open_test_db();
        let store = make_store(conn);

        store
            .push(make_user_msg("hello"), make_assistant_msg("hi"))
            .unwrap();
        store
            .push(make_user_msg("how are you"), make_assistant_msg("good"))
            .unwrap();

        let messages = store.load().unwrap();
        assert_eq!(messages.len(), 4); // 2 turns * 2 messages
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[3].role, "assistant");
    }

    #[test]
    fn load_missing_returns_empty() {
        let conn = open_test_db();
        let store = make_store(conn);
        let messages = store.load().unwrap();
        assert!(messages.is_empty());
    }

    // ── Task 6.2: TTL expiry ─────────────────────────────────────────

    #[test]
    fn expired_row_returns_empty() {
        let conn = open_test_db();
        let store = make_store(Arc::clone(&conn));

        store
            .push(make_user_msg("hello"), make_assistant_msg("hi"))
            .unwrap();

        // Backdate the row so it appears expired (ttl=24, backdate 25h).
        {
            let c = conn.lock().unwrap();
            c.execute(
                "UPDATE conversations SET updated_at = datetime('now', '-25 hours')",
                [],
            )
            .unwrap();
        }

        // TTL=24 → row is expired → should return empty and delete the row.
        let messages = store.load().unwrap();
        assert!(messages.is_empty());

        // Row should be deleted.
        let count: i64 = {
            let c = conn.lock().unwrap();
            c.query_row(
                "SELECT COUNT(*) FROM conversations",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(count, 0);
    }

    #[test]
    fn ttl_zero_never_expires() {
        let conn = open_test_db();
        let store =
            PersistentConversationStore::new(Arc::clone(&conn), "telegram", "chat-1", 0);

        store
            .push(make_user_msg("hello"), make_assistant_msg("hi"))
            .unwrap();

        // Backdate far into the past.
        {
            let c = conn.lock().unwrap();
            c.execute(
                "UPDATE conversations SET updated_at = datetime('now', '-1000 hours')",
                [],
            )
            .unwrap();
        }

        // TTL=0 → no expiry clause → still returns data.
        let messages = store.load().unwrap();
        assert_eq!(messages.len(), 2);
    }

    // ── Task 6.3: missing row ────────────────────────────────────────

    #[test]
    fn load_missing_row_is_empty() {
        let conn = open_test_db();
        let store = make_store(conn);
        assert!(store.load().unwrap().is_empty());
    }

    // ── Task 6.4: trim by turn count ─────────────────────────────────

    #[test]
    fn trim_by_turn_count() {
        let conn = open_test_db();
        let store = make_store(conn);

        for i in 0..(MAX_HISTORY_TURNS + 5) {
            store
                .push(
                    make_user_msg(&format!("msg {i}")),
                    make_assistant_msg(&format!("reply {i}")),
                )
                .unwrap();
        }

        let messages = store.load().unwrap();
        // Each turn contributes 2 messages; should not exceed MAX_HISTORY_TURNS turns.
        assert!(messages.len() <= MAX_HISTORY_TURNS * 2);
    }

    // ── Task 6.5: character-budget summarization ──────────────────────

    #[test]
    fn budget_drops_oldest_turns() {
        let conn = open_test_db();
        let store = make_store(conn);

        // Push turns large enough to exceed SUMMARY_BUDGET_CHARS.
        let big_text = "x".repeat(10_000);
        for i in 0..6 {
            store
                .push(
                    make_user_msg(&format!("{big_text}{i}")),
                    make_assistant_msg(&format!("{big_text}{i}")),
                )
                .unwrap();
        }

        let messages = store.load().unwrap();
        // 6 turns × ~20K chars = ~120K > SUMMARY_BUDGET_CHARS(40K)
        // Should have been trimmed substantially.
        assert!(messages.len() < 12); // less than 6 turns worth
    }

    // ── Task 6.6: tool result truncation ─────────────────────────────

    #[test]
    fn tool_result_truncated_at_1000_chars() {
        let conn = open_test_db();
        let store = make_store(conn);

        let long_result = "y".repeat(2000);
        store
            .push(
                make_user_msg("query"),
                make_assistant_with_tool_result(&long_result),
            )
            .unwrap();

        let messages = store.load().unwrap();
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
        let conn = open_test_db();
        let store = make_store(conn);

        let short_result = "short result";
        store
            .push(
                make_user_msg("query"),
                make_assistant_with_tool_result(short_result),
            )
            .unwrap();

        let messages = store.load().unwrap();
        let assistant = &messages[1];
        if let MessageContent::Blocks(blocks) = &assistant.content {
            for block in blocks {
                if let ContentBlock::ToolResult { content, .. } = block {
                    assert_eq!(content, short_result);
                }
            }
        }
    }

    // ── Task 6.7: thread isolation ────────────────────────────────────

    #[test]
    fn thread_isolation() {
        let conn = open_test_db();

        let store_a = PersistentConversationStore::new(
            Arc::clone(&conn),
            "telegram",
            "thread-A",
            24,
        );
        let store_b = PersistentConversationStore::new(
            Arc::clone(&conn),
            "telegram",
            "thread-B",
            24,
        );

        store_a
            .push(make_user_msg("A message"), make_assistant_msg("A reply"))
            .unwrap();

        // thread-B should see nothing from thread-A
        let b_messages = store_b.load().unwrap();
        assert!(b_messages.is_empty());

        // thread-A should still have its data
        let a_messages = store_a.load().unwrap();
        assert_eq!(a_messages.len(), 2);
    }

    #[test]
    fn channel_isolation() {
        let conn = open_test_db();

        let tg = PersistentConversationStore::new(
            Arc::clone(&conn),
            "telegram",
            "chat-1",
            24,
        );
        let discord = PersistentConversationStore::new(
            Arc::clone(&conn),
            "discord",
            "chat-1",
            24,
        );

        tg.push(make_user_msg("tg msg"), make_assistant_msg("tg reply"))
            .unwrap();

        // Same thread_id but different channel — should not bleed.
        assert!(discord.load().unwrap().is_empty());
    }

    // ── Task 6.8: clear ───────────────────────────────────────────────

    #[test]
    fn clear_removes_row() {
        let conn = open_test_db();
        let store = make_store(Arc::clone(&conn));

        store
            .push(make_user_msg("hello"), make_assistant_msg("hi"))
            .unwrap();
        assert_eq!(store.load().unwrap().len(), 2);

        store.clear().unwrap();
        assert!(store.load().unwrap().is_empty());

        // Verify the row is gone from DB.
        let count: i64 = {
            let c = conn.lock().unwrap();
            c.query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(count, 0);
    }
}
