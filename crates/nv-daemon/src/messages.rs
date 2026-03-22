use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;

// ── Stored Message ─────────────────────────────────────────────────

/// A single message stored in the SQLite database.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StoredMessage {
    pub id: i64,
    pub timestamp: String,
    pub direction: String,
    pub channel: String,
    pub sender: String,
    pub content: String,
    pub response_time_ms: Option<i64>,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
}

// ── Stats Report ───────────────────────────────────────────────────

/// Aggregated stats from the message store.
#[derive(Debug, Clone, Serialize)]
pub struct StatsReport {
    pub total_messages: i64,
    pub messages_today: i64,
    pub avg_response_time_ms: Option<f64>,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub daily_counts: Vec<(String, i64)>,
}

// ── Message Store ──────────────────────────────────────────────────

/// Persistent SQLite message store at `~/.nv/messages.db`.
pub struct MessageStore {
    conn: Connection,
}

impl MessageStore {
    /// Open (or create) the SQLite database and ensure the schema exists.
    pub fn init(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                direction TEXT NOT NULL,
                channel TEXT NOT NULL,
                sender TEXT,
                content TEXT NOT NULL,
                telegram_message_id INTEGER,
                trigger_type TEXT,
                response_time_ms INTEGER,
                tokens_in INTEGER,
                tokens_out INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);
            CREATE INDEX IF NOT EXISTS idx_messages_direction ON messages(direction);",
        )?;

        Ok(Self { conn })
    }

    /// Log an inbound message (from a user/channel).
    pub fn log_inbound(
        &self,
        channel: &str,
        sender: &str,
        content: &str,
        trigger_type: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO messages (timestamp, direction, channel, sender, content, trigger_type)
             VALUES (datetime('now'), 'inbound', ?1, ?2, ?3, ?4)",
            params![channel, sender, content, trigger_type],
        )?;
        Ok(())
    }

    /// Log an outbound message (Nova's response).
    pub fn log_outbound(
        &self,
        channel: &str,
        content: &str,
        telegram_message_id: Option<i64>,
        response_time_ms: Option<i64>,
        tokens_in: Option<i64>,
        tokens_out: Option<i64>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO messages (timestamp, direction, channel, sender, content, telegram_message_id, response_time_ms, tokens_in, tokens_out)
             VALUES (datetime('now'), 'outbound', ?1, 'nova', ?2, ?3, ?4, ?5, ?6)",
            params![channel, content, telegram_message_id, response_time_ms, tokens_in, tokens_out],
        )?;
        Ok(())
    }

    /// Return the last `count` messages, newest last.
    pub fn recent(&self, count: usize) -> Result<Vec<StoredMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, direction, channel, COALESCE(sender, ''), content, response_time_ms, tokens_in, tokens_out
             FROM messages ORDER BY id DESC LIMIT ?1",
        )?;

        let rows = stmt
            .query_map(params![count as i64], |row| {
                Ok(StoredMessage {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    direction: row.get(2)?,
                    channel: row.get(3)?,
                    sender: row.get(4)?,
                    content: row.get(5)?,
                    response_time_ms: row.get(6)?,
                    tokens_in: row.get(7)?,
                    tokens_out: row.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Reverse so newest is last (chronological order)
        let mut messages = rows;
        messages.reverse();
        Ok(messages)
    }

    /// Format recent messages for context injection.
    ///
    /// Returns a string like:
    /// ```text
    /// [12:30] Leo: What projects am I working on?
    /// [12:31] Nova: You have 14 active projects...
    /// ```
    pub fn format_recent_for_context(&self, count: usize) -> Result<String> {
        let messages = self.recent(count)?;
        if messages.is_empty() {
            return Ok(String::new());
        }

        let mut lines = Vec::with_capacity(messages.len());
        for msg in &messages {
            // Extract HH:MM from the timestamp (format: "YYYY-MM-DD HH:MM:SS")
            let time_part = if msg.timestamp.len() >= 16 {
                &msg.timestamp[11..16]
            } else {
                &msg.timestamp
            };

            // Truncate content at 500 chars
            let content = if msg.content.len() > 500 {
                format!("{}...", &msg.content[..500])
            } else {
                msg.content.clone()
            };

            let sender = if msg.direction == "outbound" {
                "Nova"
            } else {
                &msg.sender
            };

            lines.push(format!("[{time_part}] {sender}: {content}"));
        }

        Ok(lines.join("\n"))
    }

    /// Compute aggregate stats for the dashboard.
    pub fn stats(&self) -> Result<StatsReport> {
        let total_messages: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?;

        let messages_today: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE date(timestamp) = date('now')",
            [],
            |row| row.get(0),
        )?;

        let avg_response_time_ms: Option<f64> = self.conn.query_row(
            "SELECT AVG(response_time_ms) FROM messages WHERE response_time_ms IS NOT NULL",
            [],
            |row| row.get(0),
        )?;

        let total_tokens_in: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(tokens_in), 0) FROM messages",
            [],
            |row| row.get(0),
        )?;

        let total_tokens_out: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(tokens_out), 0) FROM messages",
            [],
            |row| row.get(0),
        )?;

        // Daily counts for last 7 days
        let mut stmt = self.conn.prepare(
            "SELECT date(timestamp) as d, COUNT(*) as c
             FROM messages
             WHERE timestamp >= datetime('now', '-7 days')
             GROUP BY d
             ORDER BY d DESC",
        )?;

        let daily_counts = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(StatsReport {
            total_messages,
            messages_today,
            avg_response_time_ms,
            total_tokens_in,
            total_tokens_out,
            daily_counts,
        })
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, MessageStore) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("messages.db");
        let store = MessageStore::init(&db_path).unwrap();
        (dir, store)
    }

    #[test]
    fn init_creates_table() {
        let (_dir, store) = setup();
        // Table should exist — querying it should not fail
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn init_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("messages.db");
        let _store1 = MessageStore::init(&db_path).unwrap();
        let _store2 = MessageStore::init(&db_path).unwrap();
    }

    #[test]
    fn log_inbound_inserts_row() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "hello", "message")
            .unwrap();

        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let direction: String = store
            .conn
            .query_row("SELECT direction FROM messages WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(direction, "inbound");
    }

    #[test]
    fn log_outbound_inserts_row_with_metrics() {
        let (_dir, store) = setup();
        store
            .log_outbound("telegram", "response text", Some(42), Some(1500), Some(100), Some(50))
            .unwrap();

        let (direction, response_time, tokens_in, tokens_out): (String, Option<i64>, Option<i64>, Option<i64>) = store
            .conn
            .query_row(
                "SELECT direction, response_time_ms, tokens_in, tokens_out FROM messages WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(direction, "outbound");
        assert_eq!(response_time, Some(1500));
        assert_eq!(tokens_in, Some(100));
        assert_eq!(tokens_out, Some(50));
    }

    #[test]
    fn recent_returns_messages_in_chronological_order() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "first", "message")
            .unwrap();
        store
            .log_inbound("telegram", "leo", "second", "message")
            .unwrap();
        store
            .log_outbound("telegram", "response", None, None, None, None)
            .unwrap();

        let messages = store.recent(10).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].content, "first");
        assert_eq!(messages[1].content, "second");
        assert_eq!(messages[2].content, "response");
    }

    #[test]
    fn recent_limits_count() {
        let (_dir, store) = setup();
        for i in 0..10 {
            store
                .log_inbound("telegram", "leo", &format!("msg {i}"), "message")
                .unwrap();
        }

        let messages = store.recent(3).unwrap();
        assert_eq!(messages.len(), 3);
        // Should be the last 3 messages
        assert_eq!(messages[0].content, "msg 7");
        assert_eq!(messages[1].content, "msg 8");
        assert_eq!(messages[2].content, "msg 9");
    }

    #[test]
    fn format_recent_for_context_empty() {
        let (_dir, store) = setup();
        let ctx = store.format_recent_for_context(20).unwrap();
        assert!(ctx.is_empty());
    }

    #[test]
    fn format_recent_for_context_formats_correctly() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "hello", "message")
            .unwrap();
        store
            .log_outbound("telegram", "hi there", None, None, None, None)
            .unwrap();

        let ctx = store.format_recent_for_context(20).unwrap();
        let lines: Vec<&str> = ctx.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("leo: hello"));
        assert!(lines[1].contains("Nova: hi there"));
    }

    #[test]
    fn format_recent_truncates_long_content() {
        let (_dir, store) = setup();
        let long_content = "x".repeat(600);
        store
            .log_inbound("telegram", "leo", &long_content, "message")
            .unwrap();

        let ctx = store.format_recent_for_context(20).unwrap();
        // Should be truncated to 500 chars + "..."
        assert!(ctx.contains("..."));
        assert!(ctx.len() < 600);
    }

    #[test]
    fn stats_empty_db() {
        let (_dir, store) = setup();
        let report = store.stats().unwrap();
        assert_eq!(report.total_messages, 0);
        assert_eq!(report.messages_today, 0);
        assert!(report.avg_response_time_ms.is_none());
        assert_eq!(report.total_tokens_in, 0);
        assert_eq!(report.total_tokens_out, 0);
        assert!(report.daily_counts.is_empty());
    }

    #[test]
    fn stats_with_data() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "hello", "message")
            .unwrap();
        store
            .log_outbound("telegram", "response", None, Some(2000), Some(100), Some(50))
            .unwrap();
        store
            .log_outbound("telegram", "response2", None, Some(1000), Some(200), Some(100))
            .unwrap();

        let report = store.stats().unwrap();
        assert_eq!(report.total_messages, 3);
        assert_eq!(report.total_tokens_in, 300);
        assert_eq!(report.total_tokens_out, 150);
        assert!(report.avg_response_time_ms.is_some());
        let avg = report.avg_response_time_ms.unwrap();
        assert!((avg - 1500.0).abs() < 0.1);
    }

    #[test]
    fn stats_daily_counts() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "msg1", "message")
            .unwrap();
        store
            .log_inbound("telegram", "leo", "msg2", "message")
            .unwrap();

        let report = store.stats().unwrap();
        // Both messages are "today"
        assert!(!report.daily_counts.is_empty());
        let today_count: i64 = report.daily_counts.iter().map(|(_, c)| c).sum();
        assert_eq!(today_count, 2);
    }
}
