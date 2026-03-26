//! Obligation store: CRUD operations on the `obligations` table in messages.db.
//!
//! The schema is created by migration v2 in `messages.rs`. This module provides
//! the `ObligationStore` struct that wraps a SQLite `Connection` and exposes
//! typed CRUD methods.

use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use nv_core::types::{Obligation, ObligationOwner, ObligationStatus};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::obligation_research::{Finding, ResearchResult};

// ── Input Type ───────────────────────────────────────────────────────

/// Parameters for creating a new obligation.
#[derive(Debug, Clone)]
pub struct NewObligation {
    /// UUID for the obligation (caller must provide, e.g. `Uuid::new_v4().to_string()`).
    pub id: String,
    /// Channel the obligation was detected in.
    pub source_channel: String,
    /// Excerpt or identifier of the source message (optional).
    pub source_message: Option<String>,
    /// The specific action or commitment detected.
    pub detected_action: String,
    /// Optional project code.
    pub project_code: Option<String>,
    /// Priority 0-4.
    pub priority: i32,
    /// Initial owner.
    pub owner: ObligationOwner,
    /// Optional reasoning for the owner assignment.
    pub owner_reason: Option<String>,
    /// Optional deadline — RFC 3339 UTC string; `None` means no explicit deadline.
    pub deadline: Option<String>,
}

// ── Store ─────────────────────────────────────────────────────────────

/// SQLite-backed obligation store.
///
/// Shares messages.db with `MessageStore` — the schema migration is run by
/// `MessageStore::init`, so callers must ensure `MessageStore::init` has been
/// called before constructing `ObligationStore`.
pub struct ObligationStore {
    pub(crate) conn: Connection,
}

impl ObligationStore {
    /// Open the SQLite database.
    ///
    /// Schema migrations are managed exclusively by `MessageStore`. Callers must
    /// ensure `MessageStore::init` has been called before constructing an
    /// `ObligationStore` against the same `messages.db` path.
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        Ok(Self { conn })
    }

    /// Insert a new obligation. Returns the created `Obligation`.
    pub fn create(&self, new: NewObligation) -> Result<Obligation> {
        self.conn.execute(
            "INSERT INTO obligations
                (id, source_channel, source_message, detected_action, project_code,
                 priority, status, owner, owner_reason,
                 deadline, created_at, updated_at)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'), datetime('now'))",
            params![
                new.id,
                new.source_channel,
                new.source_message,
                new.detected_action,
                new.project_code,
                new.priority,
                ObligationStatus::Open.as_str(),
                new.owner.as_str(),
                new.owner_reason,
                new.deadline,
            ],
        )?;

        // Read back the created row to get the timestamp values from SQLite.
        self.get_by_id(&new.id)?
            .ok_or_else(|| anyhow::anyhow!("obligation not found after insert: {}", new.id))
    }

    /// Retrieve an obligation by its UUID.
    pub fn get_by_id(&self, id: &str) -> Result<Option<Obligation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, deadline, created_at, updated_at,
                    last_attempt_at
             FROM obligations
             WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;

        match rows.next()? {
            Some(row) => Ok(Some(row_to_obligation(row)?)),
            None => Ok(None),
        }
    }

    /// List obligations filtered by status.
    ///
    /// Results are ordered by priority ASC (0 = most urgent), then created_at ASC.
    #[allow(dead_code)] // reserved for Next.js dashboard API exposure
    pub fn list_by_status(&self, status: &ObligationStatus) -> Result<Vec<Obligation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, deadline, created_at, updated_at,
                    last_attempt_at
             FROM obligations
             WHERE status = ?1
             ORDER BY priority ASC, created_at ASC",
        )?;

        let rows = stmt.query_map(params![status.as_str()], |row| {
            row_to_obligation(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("list_by_status query failed: {e}"))
    }

    /// List obligations filtered by owner.
    ///
    /// Results are ordered by priority ASC, then created_at ASC.
    #[allow(dead_code)] // reserved for Next.js dashboard API exposure
    pub fn list_by_owner(&self, owner: &ObligationOwner) -> Result<Vec<Obligation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, deadline, created_at, updated_at,
                    last_attempt_at
             FROM obligations
             WHERE owner = ?1
             ORDER BY priority ASC, created_at ASC",
        )?;

        let rows = stmt.query_map(params![owner.as_str()], |row| {
            row_to_obligation(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("list_by_owner query failed: {e}"))
    }

    /// List all obligations, ordered by priority ASC then created_at ASC.
    #[allow(dead_code)] // reserved for Next.js dashboard API exposure
    pub fn list_all(&self) -> Result<Vec<Obligation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, deadline, created_at, updated_at,
                    last_attempt_at
             FROM obligations
             ORDER BY priority ASC, created_at ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            row_to_obligation(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("list_all query failed: {e}"))
    }

    /// Update the status of an obligation and touch `updated_at`.
    ///
    /// Returns `true` if a row was updated, `false` if the id was not found.
    pub fn update_status(&self, id: &str, new_status: &ObligationStatus) -> Result<bool> {
        let rows_changed = self.conn.execute(
            "UPDATE obligations
             SET status = ?1, updated_at = datetime('now')
             WHERE id = ?2",
            params![new_status.as_str(), id],
        )?;

        Ok(rows_changed > 0)
    }

    /// Update both the status and owner of an obligation and touch `updated_at`.
    ///
    /// Used when a Telegram inline keyboard action changes who owns the obligation
    /// (e.g. "Handle" sets owner=Leo, "Delegate to Nova" sets owner=Nova).
    ///
    /// Returns `true` if a row was updated, `false` if the id was not found.
    pub fn update_status_and_owner(
        &self,
        id: &str,
        new_status: &ObligationStatus,
        new_owner: &ObligationOwner,
    ) -> Result<bool> {
        let rows_changed = self.conn.execute(
            "UPDATE obligations
             SET status = ?1, owner = ?2, updated_at = datetime('now')
             WHERE id = ?3",
            params![new_status.as_str(), new_owner.as_str(), id],
        )?;

        Ok(rows_changed > 0)
    }

    /// Count open obligations grouped by priority.
    ///
    /// Returns a `Vec<(priority, count)>` ordered by priority ASC.
    pub fn count_open_by_priority(&self) -> Result<Vec<(i32, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT priority, COUNT(*) FROM obligations
             WHERE status = 'open'
             GROUP BY priority
             ORDER BY priority ASC",
        )?;

        let rows = stmt.query_map([], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?)))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("count_open_by_priority failed: {e}"))
    }

    /// Update the `detected_action` text of an obligation and touch `updated_at`.
    ///
    /// Returns `true` if a row was updated, `false` if the id was not found.
    pub fn update_detected_action(&self, id: &str, new_text: &str) -> Result<bool> {
        let rows_changed = self.conn.execute(
            "UPDATE obligations SET detected_action = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![new_text, id],
        )?;
        Ok(rows_changed > 0)
    }

    /// Count obligations that are currently open.
    #[allow(dead_code)] // reserved for future dashboard/API exposure
    pub fn count_open(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM obligations WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;

        Ok(count)
    }

    /// Reset the staleness clock of an open obligation by touching `updated_at`.
    ///
    /// Returns `true` if a row was updated (i.e., the obligation exists and is open),
    /// `false` otherwise.
    pub fn snooze(&self, id: &str) -> Result<bool> {
        let rows_changed = self.conn.execute(
            "UPDATE obligations SET updated_at = datetime('now') WHERE id = ?1 AND status = 'open'",
            params![id],
        )?;
        Ok(rows_changed > 0)
    }

    /// List open obligations whose deadline is set and is at or before `cutoff`.
    ///
    /// Ordered by priority ASC, deadline ASC.
    pub fn list_open_with_deadline_before(&self, cutoff: &DateTime<Utc>) -> Result<Vec<Obligation>> {
        let cutoff_str = cutoff.to_rfc3339();
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, deadline, created_at, updated_at,
                    last_attempt_at
             FROM obligations
             WHERE status = 'open'
               AND deadline IS NOT NULL
               AND deadline <= ?1
             ORDER BY priority ASC, deadline ASC",
        )?;

        let rows = stmt.query_map(params![cutoff_str], |row| {
            row_to_obligation(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("list_open_with_deadline_before query failed: {e}"))
    }

    // ── Autonomous Execution ──────────────────────────────────────────

    /// Update the `last_attempt_at` timestamp for an obligation.
    ///
    /// Called by the obligation executor regardless of result to record
    /// when execution last ran. Enables the 2-hour cooldown between retries.
    ///
    /// Returns `true` if a row was updated, `false` if the id was not found.
    pub fn update_last_attempt_at(&self, id: &str, timestamp: &DateTime<Utc>) -> Result<bool> {
        let ts_str = timestamp.to_rfc3339();
        let rows_changed = self.conn.execute(
            "UPDATE obligations SET last_attempt_at = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![ts_str, id],
        )?;
        Ok(rows_changed > 0)
    }

    /// List obligations ready for autonomous execution.
    ///
    /// Filters:
    /// - owner = "nova"
    /// - status IN ("open", "in_progress")
    /// - last_attempt_at IS NULL OR last_attempt_at < now - cooldown_hours
    ///
    /// Ordered by priority ASC, created_at ASC (highest-priority first).
    pub fn list_ready_for_execution(&self, cooldown_hours: u32) -> Result<Vec<Obligation>> {
        let cooldown_cutoff = Utc::now()
            - chrono::Duration::hours(i64::from(cooldown_hours));
        let cutoff_str = cooldown_cutoff.to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, deadline, created_at, updated_at,
                    last_attempt_at
             FROM obligations
             WHERE owner = 'nova'
               AND status IN ('open', 'in_progress')
               AND (last_attempt_at IS NULL OR last_attempt_at < ?1)
             ORDER BY priority ASC, created_at ASC",
        )?;

        let rows = stmt.query_map(params![cutoff_str], |row| {
            row_to_obligation(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("list_ready_for_execution query failed: {e}"))
    }

    /// Append a plain-text note to `obligation_notes` for execution results.
    ///
    /// Stores a timestamped entry recording what happened during an execution attempt
    /// (success summary or failure reason).
    pub fn append_execution_note(&self, obligation_id: &str, note: &str) -> Result<()> {
        let note_id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO obligation_notes
                (id, obligation_id, note_type, content, created_at)
             VALUES (?1, ?2, 'execution', ?3, datetime('now'))",
            params![note_id, obligation_id, note],
        )?;
        Ok(())
    }

    // ── Research Notes ────────────────────────────────────────────────

    /// Persist a `ResearchResult` to the `obligation_notes` table.
    pub fn save_research_result(&self, result: &ResearchResult) -> Result<()> {
        let findings_json = serde_json::to_string(&result.raw_findings)
            .map_err(|e| anyhow::anyhow!("failed to serialize findings: {e}"))?;
        let tools_used_json = serde_json::to_string(&result.tools_used)
            .map_err(|e| anyhow::anyhow!("failed to serialize tools_used: {e}"))?;
        let researched_at = result.researched_at.to_rfc3339();

        self.conn.execute(
            "INSERT INTO obligation_notes
                (id, obligation_id, summary, findings_json, tools_used, error, researched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                Uuid::new_v4().to_string(),
                result.obligation_id,
                result.summary,
                findings_json,
                tools_used_json,
                result.error,
                researched_at,
            ],
        )?;

        Ok(())
    }

    /// Retrieve the most recent `ResearchResult` for a given obligation.
    ///
    /// Returns `None` if no notes exist for the obligation.
    #[allow(dead_code)] // wired up by proactive-obligation-research spec
    pub fn get_latest_research(&self, obligation_id: &str) -> Result<Option<ResearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT obligation_id, summary, findings_json, tools_used, error, researched_at
             FROM obligation_notes
             WHERE obligation_id = ?1
             ORDER BY researched_at DESC
             LIMIT 1",
        )?;

        let mut rows = stmt.query(params![obligation_id])?;

        match rows.next()? {
            Some(row) => Ok(Some(row_to_research_result(row)?)),
            None => Ok(None),
        }
    }

    /// List all research results for a given obligation, newest first.
    #[allow(dead_code)] // reserved for dashboard/API exposure
    pub fn list_research_by_obligation(
        &self,
        obligation_id: &str,
    ) -> Result<Vec<ResearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT obligation_id, summary, findings_json, tools_used, error, researched_at
             FROM obligation_notes
             WHERE obligation_id = ?1
             ORDER BY researched_at DESC",
        )?;

        let rows = stmt.query_map(params![obligation_id], |row| {
            row_to_research_result(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("list_research_by_obligation failed: {e}"))
    }

    /// List open obligations last updated before `since` (stale obligations).
    ///
    /// Ordered by priority ASC, updated_at ASC.
    pub fn list_stale_open(&self, since: &DateTime<Utc>) -> Result<Vec<Obligation>> {
        let since_str = since.to_rfc3339();
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, deadline, created_at, updated_at,
                    last_attempt_at
             FROM obligations
             WHERE status = 'open'
               AND updated_at <= ?1
             ORDER BY priority ASC, updated_at ASC",
        )?;

        let rows = stmt.query_map(params![since_str], |row| {
            row_to_obligation(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("list_stale_open query failed: {e}"))
    }
}

// ── Row Mappers ───────────────────────────────────────────────────────

/// Map a SQLite row to a `ResearchResult`.
///
/// Column order (0-based):
///   0 obligation_id, 1 summary, 2 findings_json, 3 tools_used, 4 error, 5 researched_at
fn row_to_research_result(row: &rusqlite::Row<'_>) -> Result<ResearchResult> {
    let obligation_id: String = row.get(0)?;
    let summary: String = row.get(1)?;
    let findings_json: String = row.get(2)?;
    let tools_used_json: String = row.get(3)?;
    let error: Option<String> = row.get(4)?;
    let researched_at_str: String = row.get(5)?;

    let raw_findings: Vec<Finding> = serde_json::from_str(&findings_json)
        .map_err(|e| anyhow::anyhow!("failed to deserialize findings: {e}"))?;
    let tools_used: Vec<String> = serde_json::from_str(&tools_used_json)
        .map_err(|e| anyhow::anyhow!("failed to deserialize tools_used: {e}"))?;
    let researched_at: DateTime<Utc> = researched_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    Ok(ResearchResult {
        obligation_id,
        summary,
        raw_findings,
        researched_at,
        tools_used,
        error,
    })
}

/// Map a SQLite row to an `Obligation`.
///
/// Column order (0-based):
///   0  id, 1 source_channel, 2 source_message, 3 detected_action, 4 project_code,
///   5  priority, 6 status, 7 owner, 8 owner_reason, 9 deadline, 10 created_at, 11 updated_at,
///   12 last_attempt_at
fn row_to_obligation(row: &rusqlite::Row<'_>) -> Result<Obligation> {
    let status_str: String = row.get(6)?;
    let owner_str: String = row.get(7)?;

    Ok(Obligation {
        id: row.get(0)?,
        source_channel: row.get(1)?,
        source_message: row.get(2)?,
        detected_action: row.get(3)?,
        project_code: row.get(4)?,
        priority: row.get(5)?,
        status: ObligationStatus::from_str(&status_str)
            .map_err(|e| anyhow::anyhow!("invalid status in DB: {e}"))?,
        owner: ObligationOwner::from_str(&owner_str)
            .map_err(|e| anyhow::anyhow!("invalid owner in DB: {e}"))?,
        owner_reason: row.get(8)?,
        deadline: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        last_attempt_at: row.get(12)?,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn temp_store() -> (ObligationStore, NamedTempFile) {
        let file = NamedTempFile::new().expect("temp file");
        let store = ObligationStore::new(file.path()).expect("store init");

        // Apply the obligations schema directly — MessageStore owns migrations in
        // production, but tests use a fresh temp DB with no MessageStore.
        store.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS obligations (
                id TEXT PRIMARY KEY,
                source_channel TEXT NOT NULL,
                source_message TEXT,
                detected_action TEXT NOT NULL,
                project_code TEXT,
                priority INTEGER NOT NULL DEFAULT 2,
                status TEXT NOT NULL DEFAULT 'open',
                owner TEXT NOT NULL DEFAULT 'nova',
                owner_reason TEXT,
                deadline TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_attempt_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_obligations_status ON obligations(status);
            CREATE INDEX IF NOT EXISTS idx_obligations_priority ON obligations(priority);
            CREATE INDEX IF NOT EXISTS idx_obligations_owner ON obligations(owner);
            CREATE TABLE IF NOT EXISTS obligation_notes (
                id            TEXT PRIMARY KEY,
                obligation_id TEXT NOT NULL,
                note_type     TEXT NOT NULL DEFAULT 'research',
                content       TEXT NOT NULL,
                summary       TEXT NOT NULL DEFAULT '',
                findings_json TEXT NOT NULL DEFAULT '[]',
                tools_used    TEXT NOT NULL DEFAULT '[]',
                error         TEXT,
                created_at    TEXT NOT NULL DEFAULT (datetime('now')),
                researched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );
            CREATE INDEX IF NOT EXISTS idx_obligation_notes_obligation
                ON obligation_notes(obligation_id);"
        ).expect("test schema setup");

        (store, file)
    }

    fn new_obligation(id: &str, channel: &str, action: &str, priority: i32) -> NewObligation {
        NewObligation {
            id: id.to_string(),
            source_channel: channel.to_string(),
            source_message: Some(format!("message for {id}")),
            detected_action: action.to_string(),
            project_code: None,
            priority,
            owner: ObligationOwner::Nova,
            owner_reason: None,
            deadline: None,
        }
    }

    #[test]
    fn create_and_get_by_id() {
        let (store, _f) = temp_store();

        let ob = store
            .create(new_obligation("id-1", "telegram", "send report", 2))
            .unwrap();

        assert_eq!(ob.id, "id-1");
        assert_eq!(ob.source_channel, "telegram");
        assert_eq!(ob.detected_action, "send report");
        assert_eq!(ob.priority, 2);
        assert_eq!(ob.status, ObligationStatus::Open);
        assert_eq!(ob.owner, ObligationOwner::Nova);

        let fetched = store.get_by_id("id-1").unwrap().expect("should exist");
        assert_eq!(fetched.id, ob.id);
    }

    #[test]
    fn get_by_id_missing_returns_none() {
        let (store, _f) = temp_store();
        assert!(store.get_by_id("not-there").unwrap().is_none());
    }

    #[test]
    fn list_by_status_returns_matching_rows() {
        let (store, _f) = temp_store();

        store
            .create(new_obligation("a", "discord", "action A", 1))
            .unwrap();
        store
            .create(new_obligation("b", "telegram", "action B", 2))
            .unwrap();

        // Mark "a" as done
        store
            .update_status("a", &ObligationStatus::Done)
            .unwrap();

        let open = store.list_by_status(&ObligationStatus::Open).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, "b");

        let done = store.list_by_status(&ObligationStatus::Done).unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].id, "a");
    }

    #[test]
    fn list_by_owner_filters_correctly() {
        let (store, _f) = temp_store();

        let mut nova_ob = new_obligation("n1", "telegram", "nova task", 2);
        nova_ob.owner = ObligationOwner::Nova;
        store.create(nova_ob).unwrap();

        let mut leo_ob = new_obligation("l1", "discord", "leo task", 1);
        leo_ob.owner = ObligationOwner::Leo;
        store.create(leo_ob).unwrap();

        let nova_list = store.list_by_owner(&ObligationOwner::Nova).unwrap();
        assert_eq!(nova_list.len(), 1);
        assert_eq!(nova_list[0].id, "n1");

        let leo_list = store.list_by_owner(&ObligationOwner::Leo).unwrap();
        assert_eq!(leo_list.len(), 1);
        assert_eq!(leo_list[0].id, "l1");
    }

    #[test]
    fn update_status_changes_status() {
        let (store, _f) = temp_store();

        store
            .create(new_obligation("x", "telegram", "do thing", 0))
            .unwrap();

        let updated = store.update_status("x", &ObligationStatus::InProgress).unwrap();
        assert!(updated);

        let ob = store.get_by_id("x").unwrap().unwrap();
        assert_eq!(ob.status, ObligationStatus::InProgress);
    }

    #[test]
    fn update_status_missing_id_returns_false() {
        let (store, _f) = temp_store();
        let result = store.update_status("ghost", &ObligationStatus::Done).unwrap();
        assert!(!result);
    }

    #[test]
    fn count_open_tracks_correctly() {
        let (store, _f) = temp_store();

        assert_eq!(store.count_open().unwrap(), 0);

        store
            .create(new_obligation("o1", "telegram", "task 1", 2))
            .unwrap();
        store
            .create(new_obligation("o2", "discord", "task 2", 1))
            .unwrap();

        assert_eq!(store.count_open().unwrap(), 2);

        store.update_status("o1", &ObligationStatus::Done).unwrap();

        assert_eq!(store.count_open().unwrap(), 1);
    }

    #[test]
    fn list_by_status_ordered_by_priority() {
        let (store, _f) = temp_store();

        // Insert in reverse priority order
        store
            .create(new_obligation("p3", "telegram", "low", 3))
            .unwrap();
        store
            .create(new_obligation("p0", "telegram", "critical", 0))
            .unwrap();
        store
            .create(new_obligation("p1", "telegram", "high", 1))
            .unwrap();

        let open = store.list_by_status(&ObligationStatus::Open).unwrap();
        let ids: Vec<&str> = open.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, vec!["p0", "p1", "p3"]);
    }

    #[test]
    fn update_status_and_owner_changes_both_fields() {
        let (store, _f) = temp_store();

        // Create with default owner=Nova
        let ob = store
            .create(new_obligation("us1", "telegram", "coordinate deploy", 1))
            .unwrap();

        assert_eq!(ob.owner, ObligationOwner::Nova);
        assert_eq!(ob.status, ObligationStatus::Open);

        // Update status to InProgress and reassign owner to Leo
        let updated = store
            .update_status_and_owner("us1", &ObligationStatus::InProgress, &ObligationOwner::Leo)
            .unwrap();
        assert!(updated);

        let fetched = store.get_by_id("us1").unwrap().unwrap();
        assert_eq!(fetched.status, ObligationStatus::InProgress);
        assert_eq!(fetched.owner, ObligationOwner::Leo);
        // updated_at must be >= created_at (SQLite datetime strings sort lexicographically)
        assert!(fetched.updated_at >= fetched.created_at);
    }

    #[test]
    fn count_open_by_priority_groups_correctly() {
        let (store, _f) = temp_store();

        // One at P0, one at P1, three at P2
        store
            .create(new_obligation("cp0", "telegram", "critical task", 0))
            .unwrap();
        store
            .create(new_obligation("cp1", "telegram", "high task", 1))
            .unwrap();
        store
            .create(new_obligation("cp2a", "telegram", "normal task a", 2))
            .unwrap();
        store
            .create(new_obligation("cp2b", "telegram", "normal task b", 2))
            .unwrap();
        store
            .create(new_obligation("cp2c", "telegram", "normal task c", 2))
            .unwrap();

        // Close one P2 — should not appear in open counts
        store
            .update_status("cp2c", &ObligationStatus::Done)
            .unwrap();

        let counts = store.count_open_by_priority().unwrap();

        // Expect [(0, 1), (1, 1), (2, 2)] — P2 count is 2 because cp2c is closed
        assert_eq!(counts, vec![(0, 1), (1, 1), (2, 2)]);
    }

    #[test]
    fn update_detected_action_changes_text() {
        let (store, _f) = temp_store();

        store
            .create(new_obligation("da1", "telegram", "original action text", 2))
            .unwrap();

        let updated = store.update_detected_action("da1", "revised action text").unwrap();
        assert!(updated, "expected update_detected_action to return true for existing id");

        let ob = store.get_by_id("da1").unwrap().unwrap();
        assert_eq!(ob.detected_action, "revised action text");
        assert!(ob.updated_at >= ob.created_at, "updated_at should be >= created_at");
    }

    #[test]
    fn update_detected_action_missing_id_returns_false() {
        let (store, _f) = temp_store();
        let result = store.update_detected_action("nonexistent", "new text").unwrap();
        assert!(!result);
    }

    #[test]
    fn list_all_returns_every_status() {
        let (store, _f) = temp_store();

        store
            .create(new_obligation("la0", "telegram", "open task", 2))
            .unwrap();
        store
            .create(new_obligation("la1", "telegram", "done task", 1))
            .unwrap();
        store
            .create(new_obligation("la2", "discord", "dismissed task", 3))
            .unwrap();

        // Close and dismiss two obligations
        store
            .update_status("la1", &ObligationStatus::Done)
            .unwrap();
        store
            .update_status("la2", &ObligationStatus::Dismissed)
            .unwrap();

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 3);

        // Ordered by priority ASC: la1 (P1), la0 (P2), la2 (P3)
        let ids: Vec<&str> = all.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, vec!["la1", "la0", "la2"]);

        // Verify all statuses are present
        let statuses: Vec<&ObligationStatus> = all.iter().map(|o| &o.status).collect();
        assert!(statuses.contains(&&ObligationStatus::Open));
        assert!(statuses.contains(&&ObligationStatus::Done));
        assert!(statuses.contains(&&ObligationStatus::Dismissed));
    }

    #[test]
    fn snooze_refreshes_updated_at_for_open_obligation() {
        let (store, _f) = temp_store();

        store.create(new_obligation("sn1", "telegram", "do something", 2)).unwrap();

        // Sleep briefly to ensure the clock advances between insert and update.
        // SQLite datetime('now') has 1-second granularity — use a direct SQL update
        // to back-date updated_at so snooze moves it forward noticeably.
        store.conn.execute(
            "UPDATE obligations SET updated_at = datetime('now', '-10 seconds') WHERE id = 'sn1'",
            [],
        ).unwrap();

        let before = store.get_by_id("sn1").unwrap().unwrap().updated_at;
        let updated = store.snooze("sn1").unwrap();
        assert!(updated, "snooze should return true for an open obligation");

        let after = store.get_by_id("sn1").unwrap().unwrap().updated_at;
        // updated_at must be >= the back-dated value (it was refreshed)
        assert!(after >= before, "updated_at should be refreshed by snooze");
    }

    #[test]
    fn snooze_returns_false_for_missing_id() {
        let (store, _f) = temp_store();
        let result = store.snooze("does-not-exist").unwrap();
        assert!(!result);
    }

    #[test]
    fn snooze_returns_false_for_closed_obligation() {
        let (store, _f) = temp_store();
        store.create(new_obligation("sn2", "telegram", "closed task", 2)).unwrap();
        store.update_status("sn2", &ObligationStatus::Done).unwrap();
        let result = store.snooze("sn2").unwrap();
        assert!(!result, "snooze must not affect non-open obligations");
    }

    #[test]
    fn list_open_with_deadline_before_returns_overdue() {
        let (store, _f) = temp_store();

        // Obligation with a deadline in the past
        let past_deadline = "2020-01-01T00:00:00+00:00".to_string();
        let future_deadline = "2099-12-31T00:00:00+00:00".to_string();

        let mut ob_past = new_obligation("dl_past", "telegram", "past deadline", 1);
        ob_past.deadline = Some(past_deadline);
        store.create(ob_past).unwrap();

        let mut ob_future = new_obligation("dl_future", "telegram", "future deadline", 1);
        ob_future.deadline = Some(future_deadline);
        store.create(ob_future).unwrap();

        // Obligation with no deadline
        store.create(new_obligation("dl_none", "telegram", "no deadline", 1)).unwrap();

        let now = Utc::now();
        let results = store.list_open_with_deadline_before(&now).unwrap();

        assert_eq!(results.len(), 1, "should only return the overdue obligation");
        assert_eq!(results[0].id, "dl_past");
    }

    #[test]
    fn list_stale_open_returns_old_obligations() {
        let (store, _f) = temp_store();

        store.create(new_obligation("stale1", "telegram", "stale task", 2)).unwrap();
        store.create(new_obligation("stale2", "telegram", "fresh task", 2)).unwrap();

        // Back-date stale1's updated_at to 3 days ago
        store.conn.execute(
            "UPDATE obligations SET updated_at = datetime('now', '-3 days') WHERE id = 'stale1'",
            [],
        ).unwrap();

        // Threshold: 2 days ago
        let threshold = Utc::now() - chrono::Duration::days(2);
        let results = store.list_stale_open(&threshold).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "stale1");
    }

    // ── Research Notes ────────────────────────────────────────────────

    fn make_research_result(obligation_id: &str) -> ResearchResult {
        use crate::obligation_research::Finding;
        ResearchResult {
            obligation_id: obligation_id.to_string(),
            summary: "OO-42 is in code review, assigned to alice.".to_string(),
            raw_findings: vec![Finding {
                tool: "jira".to_string(),
                label: "OO-42 status: In Review".to_string(),
                detail: Some("Assigned to alice, 2 comments.".to_string()),
            }],
            researched_at: Utc::now(),
            tools_used: vec!["jira".to_string()],
            error: None,
        }
    }

    #[test]
    fn save_and_get_latest_research_round_trip() {
        let (store, _f) = temp_store();
        store.create(new_obligation("rr1", "telegram", "merge OO-42", 1)).unwrap();

        let result = make_research_result("rr1");
        store.save_research_result(&result).unwrap();

        let fetched = store.get_latest_research("rr1").unwrap();
        assert!(fetched.is_some(), "should find research note");
        let note = fetched.unwrap();
        assert_eq!(note.obligation_id, "rr1");
        assert_eq!(note.summary, "OO-42 is in code review, assigned to alice.");
        assert_eq!(note.raw_findings.len(), 1);
        assert_eq!(note.tools_used, vec!["jira"]);
        assert!(note.error.is_none());
    }

    #[test]
    fn get_latest_research_returns_none_for_missing() {
        let (store, _f) = temp_store();
        store.create(new_obligation("rr2", "telegram", "do thing", 2)).unwrap();

        let result = store.get_latest_research("rr2").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn list_research_by_obligation_returns_all() {
        let (store, _f) = temp_store();
        store.create(new_obligation("rr3", "telegram", "deploy app", 1)).unwrap();

        let r1 = make_research_result("rr3");
        store.save_research_result(&r1).unwrap();
        let r2 = ResearchResult {
            obligation_id: "rr3".to_string(),
            summary: "Second research pass.".to_string(),
            raw_findings: vec![],
            researched_at: Utc::now(),
            tools_used: vec![],
            error: None,
        };
        store.save_research_result(&r2).unwrap();

        let all = store.list_research_by_obligation("rr3").unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn save_research_result_with_error_field() {
        let (store, _f) = temp_store();
        store.create(new_obligation("rr4", "discord", "fix bug", 0)).unwrap();

        let result = ResearchResult {
            obligation_id: "rr4".to_string(),
            summary: "Research failed: JSON parse error: ...".to_string(),
            raw_findings: vec![],
            researched_at: Utc::now(),
            tools_used: vec![],
            error: Some("JSON parse error: unexpected token".to_string()),
        };
        store.save_research_result(&result).unwrap();

        let fetched = store.get_latest_research("rr4").unwrap().unwrap();
        assert!(fetched.error.is_some());
        assert!(fetched.error.unwrap().contains("JSON parse error"));
    }

    // ── Autonomous Execution Tests ────────────────────────────────────

    #[test]
    fn proposed_done_status_round_trips() {
        let (store, _f) = temp_store();

        store.create(new_obligation("pd1", "telegram", "do thing", 2)).unwrap();
        store.update_status("pd1", &ObligationStatus::ProposedDone).unwrap();

        let ob = store.get_by_id("pd1").unwrap().unwrap();
        assert_eq!(ob.status, ObligationStatus::ProposedDone);
        assert_eq!(ob.status.as_str(), "proposed_done");

        // Round-trip through from_str
        let parsed: ObligationStatus = "proposed_done".parse().unwrap();
        assert_eq!(parsed, ObligationStatus::ProposedDone);
    }

    #[test]
    fn list_ready_for_execution_returns_nova_open_obligations() {
        let (store, _f) = temp_store();

        // Nova-owned open obligation — should be returned
        store.create(new_obligation("exec1", "telegram", "nova task", 2)).unwrap();

        // Leo-owned open — should NOT be returned
        let mut leo_ob = new_obligation("exec2", "telegram", "leo task", 1);
        leo_ob.owner = ObligationOwner::Leo;
        store.create(leo_ob).unwrap();

        // Nova-owned but done — should NOT be returned
        store.create(new_obligation("exec3", "telegram", "done task", 2)).unwrap();
        store.update_status("exec3", &ObligationStatus::Done).unwrap();

        let ready = store.list_ready_for_execution(2).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "exec1");
    }

    #[test]
    fn list_ready_for_execution_respects_cooldown() {
        let (store, _f) = temp_store();

        store.create(new_obligation("cool1", "telegram", "cooldown task", 2)).unwrap();

        // Set last_attempt_at to 1 hour ago (within 2-hour cooldown)
        store.conn.execute(
            "UPDATE obligations SET last_attempt_at = datetime('now', '-1 hour') WHERE id = 'cool1'",
            [],
        ).unwrap();

        // With 2-hour cooldown: should NOT be returned (only 1h ago)
        let ready = store.list_ready_for_execution(2).unwrap();
        assert!(ready.is_empty(), "should not be ready — within cooldown window");

        // With 30-minute cooldown: should be returned (1h ago > 30m cooldown)
        // Use a very short cooldown (0 hours) to force it through
        let ready_short = store.list_ready_for_execution(0).unwrap();
        assert!(!ready_short.is_empty(), "should be ready with 0-hour cooldown");
    }

    #[test]
    fn update_last_attempt_at_persists_timestamp() {
        let (store, _f) = temp_store();
        store.create(new_obligation("lat1", "telegram", "attempt task", 2)).unwrap();

        let before = store.get_by_id("lat1").unwrap().unwrap();
        assert!(before.last_attempt_at.is_none());

        let now = Utc::now();
        store.update_last_attempt_at("lat1", &now).unwrap();

        let after = store.get_by_id("lat1").unwrap().unwrap();
        assert!(after.last_attempt_at.is_some());
    }
}
