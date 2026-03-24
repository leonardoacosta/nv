//! Obligation store: CRUD operations on the `obligations` table in messages.db.
//!
//! The schema is created by migration v2 in `messages.rs`. This module provides
//! the `ObligationStore` struct that wraps a SQLite `Connection` and exposes
//! typed CRUD methods.

use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use nv_core::types::{Obligation, ObligationOwner, ObligationStatus};
use rusqlite::{params, Connection};

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
                 created_at, updated_at)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'), datetime('now'))",
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
                    priority, status, owner, owner_reason, created_at, updated_at
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
    pub fn list_by_status(&self, status: &ObligationStatus) -> Result<Vec<Obligation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, created_at, updated_at
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
    pub fn list_by_owner(&self, owner: &ObligationOwner) -> Result<Vec<Obligation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, created_at, updated_at
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
    pub fn list_all(&self) -> Result<Vec<Obligation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_channel, source_message, detected_action, project_code,
                    priority, status, owner, owner_reason, created_at, updated_at
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
}

// ── Row Mapper ───────────────────────────────────────────────────────

/// Map a SQLite row to an `Obligation`.
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
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
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
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_obligations_status ON obligations(status);
            CREATE INDEX IF NOT EXISTS idx_obligations_priority ON obligations(priority);
            CREATE INDEX IF NOT EXISTS idx_obligations_owner ON obligations(owner);"
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
}
