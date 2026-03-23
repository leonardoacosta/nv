use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
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

// ── Tool Stats Report ─────────────────────────────────────────────

/// Per-tool breakdown entry.
#[derive(Debug, Clone, Serialize)]
pub struct ToolBreakdown {
    pub name: String,
    pub count: i64,
    pub success_count: i64,
    pub avg_duration_ms: Option<f64>,
}

/// One of the top-N slowest tool invocations.
#[derive(Debug, Clone, Serialize)]
pub struct SlowestInvocation {
    pub tool_name: String,
    pub duration_ms: i64,
    pub timestamp: String,
}

/// Aggregated tool usage stats.
#[derive(Debug, Clone, Serialize)]
pub struct ToolStatsReport {
    pub total_invocations: i64,
    pub invocations_today: i64,
    pub per_tool: Vec<ToolBreakdown>,
    pub slowest: Vec<SlowestInvocation>,
}

// ── Message Store ──────────────────────────────────────────────────

/// Persistent SQLite message store at `~/.nv/messages.db`.
pub struct MessageStore {
    conn: Connection,
}

/// Versioned migrations for messages.db.
///
/// Version 1 is the initial schema, converting the CREATE TABLE IF NOT EXISTS
/// pattern to a migration so future ALTER TABLE changes are safe.
fn messages_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE messages (
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
            CREATE INDEX idx_messages_timestamp ON messages(timestamp);
            CREATE INDEX idx_messages_direction ON messages(direction);

            CREATE TABLE tool_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                worker_id TEXT,
                tool_name TEXT NOT NULL,
                input_summary TEXT,
                result_summary TEXT,
                success INTEGER NOT NULL DEFAULT 1,
                duration_ms INTEGER,
                tokens_in INTEGER,
                tokens_out INTEGER
            );
            CREATE INDEX idx_tool_usage_name ON tool_usage(tool_name);
            CREATE INDEX idx_tool_usage_timestamp ON tool_usage(timestamp);

            CREATE VIRTUAL TABLE messages_fts USING fts5(
                content,
                content=messages,
                content_rowid=id
            );

            CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
            END;

            CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', old.id, old.content);
            END;

            CREATE TRIGGER messages_au AFTER UPDATE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', old.id, old.content);
                INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
            END;

            CREATE TABLE api_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                worker_id TEXT NOT NULL,
                cost_usd REAL,
                tokens_in INTEGER NOT NULL,
                tokens_out INTEGER NOT NULL,
                model TEXT NOT NULL,
                session_id TEXT NOT NULL
            );
            CREATE INDEX idx_api_usage_timestamp ON api_usage(timestamp);
            CREATE INDEX idx_api_usage_worker ON api_usage(worker_id);

            CREATE TABLE budget_alert_sent (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        ),
    ])
}

impl MessageStore {
    /// Open (or create) the SQLite database and run versioned migrations.
    ///
    /// Uses PRAGMA user_version to track schema version. Safe for ALTER TABLE
    /// changes in future migration versions.
    pub fn init(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        messages_migrations()
            .to_latest(&mut conn)
            .map_err(|e| anyhow::anyhow!("failed to run messages.db migrations: {e}"))?;

        // One-time backfill: populate FTS index with any messages that exist
        // but are not yet indexed (idempotent — skips already-indexed rows).
        conn.execute_batch(
            "INSERT OR IGNORE INTO messages_fts(rowid, content)
             SELECT id, content FROM messages
             WHERE id NOT IN (SELECT rowid FROM messages_fts);",
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

    /// Search messages using FTS5 full-text search.
    ///
    /// Returns messages matching the query ranked by relevance.
    /// Default limit: 10, max: 50.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<StoredMessage>> {
        let limit = limit.min(50);
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.timestamp, m.direction, m.channel,
                    COALESCE(m.sender, ''), m.content,
                    m.response_time_ms, m.tokens_in, m.tokens_out
             FROM messages m
             JOIN messages_fts f ON m.id = f.rowid
             WHERE messages_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![query, limit as i64], |row| {
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

        Ok(rows)
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

        let mut lines = Vec::with_capacity(messages.len() + 10);
        let mut in_turn = false;
        let mut prev_direction: Option<&str> = None;

        for msg in &messages {
            // Extract HH:MM from the timestamp (format: "YYYY-MM-DD HH:MM:SS")
            let time_part = if msg.timestamp.len() >= 16 {
                &msg.timestamp[11..16]
            } else {
                &msg.timestamp
            };

            // Truncate content at 2000 chars
            let content = if msg.content.len() > 2000 {
                // Find a valid char boundary
                let mut end = 2000;
                while end > 0 && !msg.content.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &msg.content[..end])
            } else {
                msg.content.clone()
            };

            let sender = if msg.direction == "outbound" {
                "Nova"
            } else {
                &msg.sender
            };

            // Turn-pair grouping: start a new turn group when we see a user message
            // after an assistant message (or at the start)
            if msg.direction == "inbound" {
                if in_turn {
                    lines.push("--- end turn ---".to_string());
                }
                lines.push("--- turn ---".to_string());
                in_turn = true;
            } else if !in_turn && prev_direction.is_none() {
                // Orphan assistant message at the start
                lines.push("--- turn ---".to_string());
                in_turn = true;
            }

            lines.push(format!("[{time_part}] {sender}: {content}"));
            prev_direction = Some(if msg.direction == "outbound" {
                "outbound"
            } else {
                "inbound"
            });
        }

        if in_turn {
            lines.push("--- end turn ---".to_string());
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

    // ── Tool Usage Logging ─────────────────────────────────────────

    /// Log a single tool invocation to the `tool_usage` table.
    ///
    /// `input_summary` and `result_summary` are truncated to 500 chars.
    #[allow(clippy::too_many_arguments)]
    pub fn log_tool_usage(
        &self,
        tool_name: &str,
        input_summary: &str,
        result_summary: &str,
        success: bool,
        duration_ms: i64,
        worker_id: Option<&str>,
        tokens_in: Option<i64>,
        tokens_out: Option<i64>,
    ) -> Result<()> {
        let input_trunc = truncate_str(input_summary, 500);
        let result_trunc = truncate_str(result_summary, 500);

        self.conn.execute(
            "INSERT INTO tool_usage (timestamp, worker_id, tool_name, input_summary, result_summary, success, duration_ms, tokens_in, tokens_out)
             VALUES (datetime('now'), ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                worker_id,
                tool_name,
                input_trunc,
                result_trunc,
                success as i32,
                duration_ms,
                tokens_in,
                tokens_out,
            ],
        )?;
        Ok(())
    }

    /// Query aggregated tool usage statistics.
    pub fn tool_stats(&self) -> Result<ToolStatsReport> {
        let total_invocations: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tool_usage", [], |row| row.get(0))?;

        let invocations_today: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tool_usage WHERE date(timestamp) = date('now')",
            [],
            |row| row.get(0),
        )?;

        // Per-tool breakdown
        let mut stmt = self.conn.prepare(
            "SELECT tool_name,
                    COUNT(*) as cnt,
                    SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as success_cnt,
                    AVG(duration_ms) as avg_dur
             FROM tool_usage
             GROUP BY tool_name
             ORDER BY cnt DESC",
        )?;

        let per_tool = stmt
            .query_map([], |row| {
                Ok(ToolBreakdown {
                    name: row.get(0)?,
                    count: row.get(1)?,
                    success_count: row.get(2)?,
                    avg_duration_ms: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Top 5 slowest invocations
        let mut stmt = self.conn.prepare(
            "SELECT tool_name, duration_ms, timestamp
             FROM tool_usage
             WHERE duration_ms IS NOT NULL
             ORDER BY duration_ms DESC
             LIMIT 5",
        )?;

        let slowest = stmt
            .query_map([], |row| {
                Ok(SlowestInvocation {
                    tool_name: row.get(0)?,
                    duration_ms: row.get(1)?,
                    timestamp: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(ToolStatsReport {
            total_invocations,
            invocations_today,
            per_tool,
            slowest,
        })
    }

    // ── API Usage Logging ────────────────────────────────────────────

    /// Log a single Claude API call to the `api_usage` table.
    pub fn log_api_usage(
        &self,
        worker_id: &str,
        cost_usd: Option<f64>,
        tokens_in: i64,
        tokens_out: i64,
        model: &str,
        session_id: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO api_usage (timestamp, worker_id, cost_usd, tokens_in, tokens_out, model, session_id)
             VALUES (datetime('now'), ?1, ?2, ?3, ?4, ?5, ?6)",
            params![worker_id, cost_usd, tokens_in, tokens_out, model, session_id],
        )?;
        Ok(())
    }

    /// Query aggregated Claude API usage statistics.
    pub fn usage_stats(&self) -> Result<UsageStatsReport> {
        // Today
        let (today_cost, today_calls, today_tokens_in, today_tokens_out): (f64, i64, i64, i64) =
            self.conn.query_row(
                "SELECT COALESCE(SUM(cost_usd), 0.0),
                        COUNT(*),
                        COALESCE(SUM(tokens_in), 0),
                        COALESCE(SUM(tokens_out), 0)
                 FROM api_usage
                 WHERE date(timestamp) = date('now')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;

        // Rolling 7-day
        let week_cost: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(cost_usd), 0.0)
             FROM api_usage
             WHERE timestamp >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )?;

        // Rolling 30-day
        let month_cost: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(cost_usd), 0.0)
             FROM api_usage
             WHERE timestamp >= datetime('now', '-30 days')",
            [],
            |row| row.get(0),
        )?;

        // Daily breakdown (last 7 days)
        let mut stmt = self.conn.prepare(
            "SELECT date(timestamp) as d,
                    COALESCE(SUM(cost_usd), 0.0),
                    COUNT(*),
                    COALESCE(SUM(tokens_in), 0),
                    COALESCE(SUM(tokens_out), 0)
             FROM api_usage
             WHERE timestamp >= datetime('now', '-7 days')
             GROUP BY d
             ORDER BY d DESC",
        )?;

        let daily_breakdown = stmt
            .query_map([], |row| {
                Ok(DailyUsage {
                    date: row.get(0)?,
                    cost: row.get(1)?,
                    calls: row.get(2)?,
                    tokens_in: row.get(3)?,
                    tokens_out: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(UsageStatsReport {
            today_cost,
            today_calls,
            today_tokens_in,
            today_tokens_out,
            week_cost,
            month_cost,
            daily_breakdown,
        })
    }

    /// Calculate budget status against a weekly budget.
    pub fn usage_budget_status(&self, weekly_budget: f64) -> Result<BudgetStatus> {
        let rolling_7d_cost: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(cost_usd), 0.0)
             FROM api_usage
             WHERE timestamp >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )?;

        let pct_used = if weekly_budget > 0.0 {
            (rolling_7d_cost / weekly_budget) * 100.0
        } else {
            0.0
        };

        Ok(BudgetStatus {
            rolling_7d_cost,
            weekly_budget,
            pct_used,
        })
    }

    /// Check whether a budget alert was sent within the last `hours` hours.
    pub fn budget_alert_sent_within(&self, hours: u32) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM budget_alert_sent WHERE timestamp >= datetime('now', '-{hours} hours')"
            ),
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Record that a budget alert was sent now.
    pub fn record_budget_alert(&self) -> Result<()> {
        self.conn.execute(
            "INSERT INTO budget_alert_sent (timestamp) VALUES (datetime('now'))",
            [],
        )?;
        Ok(())
    }
}

// ── Usage Stats ──────────────────────────────────────────────────

/// Daily usage breakdown entry.
#[derive(Debug, Clone, Serialize)]
pub struct DailyUsage {
    pub date: String,
    pub cost: f64,
    pub calls: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
}

/// Aggregated Claude API usage stats.
#[derive(Debug, Clone, Serialize)]
pub struct UsageStatsReport {
    pub today_cost: f64,
    pub today_calls: i64,
    pub today_tokens_in: i64,
    pub today_tokens_out: i64,
    pub week_cost: f64,
    pub month_cost: f64,
    pub daily_breakdown: Vec<DailyUsage>,
}

/// Budget status against a weekly budget.
#[derive(Debug, Clone, Serialize)]
pub struct BudgetStatus {
    pub rolling_7d_cost: f64,
    pub weekly_budget: f64,
    pub pct_used: f64,
}

/// Truncate a string to at most `max_len` characters.
fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        // Find a valid char boundary at or before max_len
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
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
        // Should have turn markers + 2 messages + end marker
        assert!(ctx.contains("--- turn ---"));
        assert!(ctx.contains("--- end turn ---"));
        assert!(ctx.contains("leo: hello"));
        assert!(ctx.contains("Nova: hi there"));
        // 4 lines: turn marker, user msg, assistant msg, end turn marker
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn format_recent_truncates_long_content() {
        let (_dir, store) = setup();
        let long_content = "x".repeat(2500);
        store
            .log_inbound("telegram", "leo", &long_content, "message")
            .unwrap();

        let ctx = store.format_recent_for_context(20).unwrap();
        // Should be truncated to 2000 chars + "..."
        assert!(ctx.contains("..."));
        // The total context line should be < 2500 (plus some overhead for markers and prefix)
        let content_line = ctx.lines().find(|l| l.contains("leo:")).unwrap();
        // Content portion is after "[HH:MM] leo: " prefix
        assert!(content_line.len() < 2100);
    }

    #[test]
    fn format_recent_2000_char_content_not_truncated() {
        let (_dir, store) = setup();
        let content = "x".repeat(1999);
        store
            .log_inbound("telegram", "leo", &content, "message")
            .unwrap();

        let ctx = store.format_recent_for_context(20).unwrap();
        // Should NOT be truncated (under 2000)
        assert!(!ctx.contains("..."));
    }

    #[test]
    fn format_recent_turn_grouping_multiple_turns() {
        let (_dir, store) = setup();
        // Turn 1
        store.log_inbound("telegram", "leo", "question 1", "message").unwrap();
        store.log_outbound("telegram", "answer 1", None, None, None, None).unwrap();
        // Turn 2
        store.log_inbound("telegram", "leo", "question 2", "message").unwrap();
        store.log_outbound("telegram", "answer 2", None, None, None, None).unwrap();

        let ctx = store.format_recent_for_context(20).unwrap();
        let turn_starts: Vec<_> = ctx.lines().filter(|l| *l == "--- turn ---").collect();
        let turn_ends: Vec<_> = ctx.lines().filter(|l| *l == "--- end turn ---").collect();
        assert_eq!(turn_starts.len(), 2, "should have 2 turn groups");
        assert_eq!(turn_ends.len(), 2, "should have 2 end markers");
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

    // ── Tool Usage Tests ────────────────────────────────────────────

    #[test]
    fn tool_usage_table_created_on_init() {
        let (_dir, store) = setup();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM tool_usage", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn log_tool_usage_inserts_and_tool_stats_retrieves() {
        let (_dir, store) = setup();
        store
            .log_tool_usage(
                "read_memory",
                r#"{"topic":"tasks"}"#,
                "Tasks: ...",
                true,
                42,
                Some("w-1"),
                None,
                None,
            )
            .unwrap();

        let report = store.tool_stats().unwrap();
        assert_eq!(report.total_invocations, 1);
        assert_eq!(report.invocations_today, 1);
        assert_eq!(report.per_tool.len(), 1);
        assert_eq!(report.per_tool[0].name, "read_memory");
        assert_eq!(report.per_tool[0].count, 1);
        assert_eq!(report.per_tool[0].success_count, 1);
    }

    #[test]
    fn tool_stats_per_tool_breakdown_multiple_tools() {
        let (_dir, store) = setup();
        // 3 read_memory calls (2 success, 1 fail)
        store
            .log_tool_usage("read_memory", "{}", "ok", true, 10, None, None, None)
            .unwrap();
        store
            .log_tool_usage("read_memory", "{}", "ok", true, 30, None, None, None)
            .unwrap();
        store
            .log_tool_usage("read_memory", "{}", "err", false, 50, None, None, None)
            .unwrap();
        // 1 jira_search call
        store
            .log_tool_usage("jira_search", "{}", "ok", true, 200, None, None, None)
            .unwrap();

        let report = store.tool_stats().unwrap();
        assert_eq!(report.total_invocations, 4);
        assert_eq!(report.per_tool.len(), 2);

        // read_memory should be first (most calls)
        let rm = &report.per_tool[0];
        assert_eq!(rm.name, "read_memory");
        assert_eq!(rm.count, 3);
        assert_eq!(rm.success_count, 2);
        let avg = rm.avg_duration_ms.unwrap();
        assert!((avg - 30.0).abs() < 0.1); // (10+30+50)/3 = 30

        let js = &report.per_tool[1];
        assert_eq!(js.name, "jira_search");
        assert_eq!(js.count, 1);
    }

    #[test]
    fn tool_stats_truncates_input_and_result_to_500() {
        let (_dir, store) = setup();
        let long_input = "x".repeat(700);
        let long_result = "y".repeat(700);
        store
            .log_tool_usage(
                "write_memory",
                &long_input,
                &long_result,
                true,
                5,
                None,
                None,
                None,
            )
            .unwrap();

        // Verify truncation by reading back
        let input_summary: String = store
            .conn
            .query_row(
                "SELECT input_summary FROM tool_usage WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let result_summary: String = store
            .conn
            .query_row(
                "SELECT result_summary FROM tool_usage WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(input_summary.len(), 500);
        assert_eq!(result_summary.len(), 500);
    }

    #[test]
    fn tool_stats_top5_slowest_ordered_correctly() {
        let (_dir, store) = setup();
        for ms in [100, 500, 200, 800, 50, 300, 1000] {
            store
                .log_tool_usage("tool_a", "{}", "ok", true, ms, None, None, None)
                .unwrap();
        }

        let report = store.tool_stats().unwrap();
        assert_eq!(report.slowest.len(), 5);
        // Should be 1000, 800, 500, 300, 200 (descending)
        let durations: Vec<i64> = report.slowest.iter().map(|s| s.duration_ms).collect();
        assert_eq!(durations, vec![1000, 800, 500, 300, 200]);
    }

    #[test]
    fn tool_stats_empty_db() {
        let (_dir, store) = setup();
        let report = store.tool_stats().unwrap();
        assert_eq!(report.total_invocations, 0);
        assert_eq!(report.invocations_today, 0);
        assert!(report.per_tool.is_empty());
        assert!(report.slowest.is_empty());
    }

    // ── FTS5 Search Tests ────────────────────────────────────────────

    #[test]
    fn fts5_table_created_on_init() {
        let (_dir, store) = setup();
        // FTS table should exist — querying it should not fail
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages_fts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn search_with_matches() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "discuss Stripe fee structure", "message")
            .unwrap();
        store
            .log_inbound("telegram", "leo", "deploy the new API endpoint", "message")
            .unwrap();
        store
            .log_outbound("telegram", "Stripe fees are 2.9% + 30c per transaction", None, None, None, None)
            .unwrap();

        let results = store.search("Stripe", 10).unwrap();
        assert_eq!(results.len(), 2);
        // Both messages mentioning "Stripe" should be returned
        assert!(results.iter().all(|m| m.content.contains("Stripe") || m.content.contains("stripe")));
    }

    #[test]
    fn search_with_no_matches() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "hello world", "message")
            .unwrap();

        let results = store.search("nonexistent", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_with_invalid_query() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "hello world", "message")
            .unwrap();

        // Invalid FTS5 syntax should return an error
        let result = store.search("AND OR NOT", 10);
        assert!(result.is_err());
    }

    #[test]
    fn search_respects_limit() {
        let (_dir, store) = setup();
        for i in 0..20 {
            store
                .log_inbound("telegram", "leo", &format!("message about topic {i}"), "message")
                .unwrap();
        }

        let results = store.search("topic", 5).unwrap();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn search_limit_capped_at_50() {
        let (_dir, store) = setup();
        store
            .log_inbound("telegram", "leo", "test message", "message")
            .unwrap();

        // Requesting 100 should be capped to 50 internally
        let results = store.search("test", 100).unwrap();
        assert_eq!(results.len(), 1); // Only 1 message exists
    }

    #[test]
    fn backfill_idempotency() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("messages.db");

        // First init — insert messages before FTS exists won't matter
        // because init creates FTS + backfills atomically.
        let store1 = MessageStore::init(&db_path).unwrap();
        store1
            .log_inbound("telegram", "leo", "backfill test message", "message")
            .unwrap();
        drop(store1);

        // Second init — should not duplicate FTS entries
        let store2 = MessageStore::init(&db_path).unwrap();
        let results = store2.search("backfill", 10).unwrap();
        assert_eq!(results.len(), 1);

        // Third init — still idempotent
        drop(store2);
        let store3 = MessageStore::init(&db_path).unwrap();
        let results = store3.search("backfill", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    // ── API Usage Tests ──────────────────────────────────────────────

    #[test]
    fn api_usage_table_created_on_init() {
        let (_dir, store) = setup();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM api_usage", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn log_api_usage_inserts_row() {
        let (_dir, store) = setup();
        store
            .log_api_usage("w-1", Some(0.05), 1000, 200, "claude-sonnet", "sess-1")
            .unwrap();

        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM api_usage", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let (cost, model): (Option<f64>, String) = store
            .conn
            .query_row(
                "SELECT cost_usd, model FROM api_usage WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!((cost.unwrap() - 0.05).abs() < 0.001);
        assert_eq!(model, "claude-sonnet");
    }

    #[test]
    fn log_api_usage_null_cost() {
        let (_dir, store) = setup();
        store
            .log_api_usage("w-1", None, 500, 100, "claude-opus", "sess-2")
            .unwrap();

        let cost: Option<f64> = store
            .conn
            .query_row("SELECT cost_usd FROM api_usage WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(cost.is_none());
    }

    #[test]
    fn usage_stats_empty_db() {
        let (_dir, store) = setup();
        let report = store.usage_stats().unwrap();
        assert_eq!(report.today_cost, 0.0);
        assert_eq!(report.today_calls, 0);
        assert_eq!(report.today_tokens_in, 0);
        assert_eq!(report.today_tokens_out, 0);
        assert_eq!(report.week_cost, 0.0);
        assert_eq!(report.month_cost, 0.0);
        assert!(report.daily_breakdown.is_empty());
    }

    #[test]
    fn usage_stats_with_data() {
        let (_dir, store) = setup();
        store
            .log_api_usage("w-1", Some(1.50), 1000, 200, "claude-sonnet", "s1")
            .unwrap();
        store
            .log_api_usage("w-2", Some(2.00), 2000, 400, "claude-sonnet", "s2")
            .unwrap();

        let report = store.usage_stats().unwrap();
        assert!((report.today_cost - 3.50).abs() < 0.01);
        assert_eq!(report.today_calls, 2);
        assert_eq!(report.today_tokens_in, 3000);
        assert_eq!(report.today_tokens_out, 600);
        assert!((report.week_cost - 3.50).abs() < 0.01);
        assert!((report.month_cost - 3.50).abs() < 0.01);
        assert_eq!(report.daily_breakdown.len(), 1);
    }

    #[test]
    fn budget_status_calculation() {
        let (_dir, store) = setup();
        store
            .log_api_usage("w-1", Some(40.0), 1000, 200, "claude-sonnet", "s1")
            .unwrap();

        let status = store.usage_budget_status(50.0).unwrap();
        assert!((status.rolling_7d_cost - 40.0).abs() < 0.01);
        assert_eq!(status.weekly_budget, 50.0);
        assert!((status.pct_used - 80.0).abs() < 0.1);
    }

    #[test]
    fn budget_status_zero_budget() {
        let (_dir, store) = setup();
        let status = store.usage_budget_status(0.0).unwrap();
        assert_eq!(status.pct_used, 0.0);
    }

    #[test]
    fn budget_alert_debounce() {
        let (_dir, store) = setup();
        // No alerts sent yet
        assert!(!store.budget_alert_sent_within(6).unwrap());

        // Record an alert
        store.record_budget_alert().unwrap();
        assert!(store.budget_alert_sent_within(6).unwrap());
    }

    #[test]
    fn budget_alert_sent_table_created() {
        let (_dir, store) = setup();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM budget_alert_sent", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
