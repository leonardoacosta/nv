//! Schedule CRUD and cron validation for user-defined recurring schedules.
//!
//! `ScheduleStore` persists user schedules in `~/.nv/schedules.db` (SQLite),
//! following the same pattern as `MessageStore`. Built-in schedules (digest,
//! memory-cleanup) are read-only and hardcoded in the formatter.

use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Schedule struct ─────────────────────────────────────────────────

/// A user-created recurring schedule persisted in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub cron_expr: String,
    pub action: String,
    pub channel: String,
    pub enabled: bool,
    pub created_at: String,
    pub last_run_at: Option<String>,
}

// ── Reserved names ──────────────────────────────────────────────────

/// Names that map to built-in (hardcoded) schedules and cannot be used
/// for user schedules.
pub const RESERVED_NAMES: &[&str] = &["digest", "memory-cleanup"];

// ── ScheduleStore ───────────────────────────────────────────────────

/// SQLite-backed store for user-defined schedules.
pub struct ScheduleStore {
    conn: Connection,
}

impl ScheduleStore {
    /// Open (or create) the `schedules.db` database inside `nv_base`.
    ///
    /// Runs `CREATE TABLE IF NOT EXISTS` on every open so the schema is
    /// always present.
    pub fn new(nv_base: &Path) -> Result<Self> {
        let db_path = nv_base.join("schedules.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| anyhow!("failed to open schedules.db at {}: {e}", db_path.display()))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schedules (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL UNIQUE,
                cron_expr   TEXT NOT NULL,
                action      TEXT NOT NULL,
                channel     TEXT NOT NULL,
                enabled     INTEGER NOT NULL DEFAULT 1,
                created_at  TEXT NOT NULL,
                last_run_at TEXT
            );",
        )
        .map_err(|e| anyhow!("failed to create schedules table: {e}"))?;

        Ok(Self { conn })
    }

    /// Return all schedules (user-created) from the database.
    pub fn list(&self) -> Result<Vec<Schedule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, cron_expr, action, channel, enabled, created_at, last_run_at
             FROM schedules ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Schedule {
                id: row.get(0)?,
                name: row.get(1)?,
                cron_expr: row.get(2)?,
                action: row.get(3)?,
                channel: row.get(4)?,
                enabled: row.get::<_, i64>(5)? != 0,
                created_at: row.get(6)?,
                last_run_at: row.get(7)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("failed to list schedules: {e}"))
    }

    /// Look up a schedule by its unique name. Returns `None` if not found.
    pub fn get(&self, name: &str) -> Result<Option<Schedule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, cron_expr, action, channel, enabled, created_at, last_run_at
             FROM schedules WHERE name = ?1",
        )?;

        let mut rows = stmt.query_map(params![name], |row| {
            Ok(Schedule {
                id: row.get(0)?,
                name: row.get(1)?,
                cron_expr: row.get(2)?,
                action: row.get(3)?,
                channel: row.get(4)?,
                enabled: row.get::<_, i64>(5)? != 0,
                created_at: row.get(6)?,
                last_run_at: row.get(7)?,
            })
        })?;

        match rows.next() {
            Some(Ok(s)) => Ok(Some(s)),
            Some(Err(e)) => Err(anyhow!("query error: {e}")),
            None => Ok(None),
        }
    }

    /// Insert a new schedule. Fails if the name is already in use (UNIQUE constraint).
    pub fn insert(&self, schedule: &Schedule) -> Result<()> {
        self.conn.execute(
            "INSERT INTO schedules (id, name, cron_expr, action, channel, enabled, created_at, last_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                schedule.id,
                schedule.name,
                schedule.cron_expr,
                schedule.action,
                schedule.channel,
                if schedule.enabled { 1i64 } else { 0i64 },
                schedule.created_at,
                schedule.last_run_at,
            ],
        )
        .map_err(|e| anyhow!("failed to insert schedule '{}': {e}", schedule.name))?;
        Ok(())
    }

    /// Update the cron expression for a schedule by name.
    pub fn update_cron(&self, name: &str, cron_expr: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE schedules SET cron_expr = ?1 WHERE name = ?2",
            params![cron_expr, name],
        )?;
        if rows == 0 {
            return Err(anyhow!("schedule '{}' not found", name));
        }
        Ok(())
    }

    /// Enable or pause a schedule by name.
    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE schedules SET enabled = ?1 WHERE name = ?2",
            params![if enabled { 1i64 } else { 0i64 }, name],
        )?;
        if rows == 0 {
            return Err(anyhow!("schedule '{}' not found", name));
        }
        Ok(())
    }

    /// Delete a schedule by name. Returns `true` if a row was deleted.
    pub fn delete(&self, name: &str) -> Result<bool> {
        let rows = self
            .conn
            .execute("DELETE FROM schedules WHERE name = ?1", params![name])?;
        Ok(rows > 0)
    }

    /// Set `last_run_at` to the current UTC time for the named schedule.
    pub fn mark_run(&self, name: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let rows = self.conn.execute(
            "UPDATE schedules SET last_run_at = ?1 WHERE name = ?2",
            params![now, name],
        )?;
        if rows == 0 {
            return Err(anyhow!("schedule '{}' not found", name));
        }
        Ok(())
    }
}

// ── Cron helpers ────────────────────────────────────────────────────

/// Validate a standard 5-field cron expression and return the parsed
/// `cron::Schedule`.
///
/// The `cron` crate uses 7-field expressions (`sec min hr dom mon dow yr`).
/// This helper prepends `0` (seconds) and appends `*` (any year) so users
/// can supply the familiar 5-field `min hr dom mon dow` format.
pub fn validate_cron_expr(expr: &str) -> Result<cron::Schedule> {
    let seven_field = format!("0 {expr} *");
    cron::Schedule::from_str(&seven_field)
        .map_err(|e| anyhow!("invalid cron expression '{}': {e}", expr))
}

/// Return the next upcoming fire time for a 5-field cron expression,
/// calculated from `Utc::now()`.
pub fn next_fire_time(cron_expr: &str) -> Result<Option<DateTime<Utc>>> {
    let schedule = validate_cron_expr(cron_expr)?;
    Ok(schedule.upcoming(Utc).next())
}

/// Return a human-readable description for a 5-field cron expression.
///
/// Delegates to the `cron` crate for parsing; formats the next fire time
/// as a relative label.
pub fn describe_cron(cron_expr: &str) -> String {
    match next_fire_time(cron_expr) {
        Ok(Some(t)) => {
            let now = Utc::now();
            let secs = (t - now).num_seconds().max(0);
            if secs < 60 {
                format!("next in {secs}s")
            } else if secs < 3600 {
                format!("next in {}m", secs / 60)
            } else if secs < 86400 {
                format!("next in {}h {}m", secs / 3600, (secs % 3600) / 60)
            } else {
                format!("next in {}d {}h", secs / 86400, (secs % 86400) / 3600)
            }
        }
        Ok(None) => "never fires".to_string(),
        Err(_) => "unknown schedule".to_string(),
    }
}

/// Validate that a schedule name contains only lowercase alphanumeric chars and hyphens.
pub fn validate_schedule_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("schedule name cannot be empty"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow!(
            "schedule name '{}' must contain only lowercase letters, digits, and hyphens",
            name
        ));
    }
    Ok(())
}

/// Build a new `Schedule` value ready for insertion.
pub fn build_schedule(
    name: String,
    cron_expr: String,
    action: String,
    channel: String,
) -> Schedule {
    Schedule {
        id: Uuid::new_v4().to_string(),
        name,
        cron_expr,
        action,
        channel,
        enabled: true,
        created_at: Utc::now().to_rfc3339(),
        last_run_at: None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store() -> (TempDir, ScheduleStore) {
        let dir = TempDir::new().unwrap();
        let store = ScheduleStore::new(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn insert_and_list() {
        let (_dir, store) = make_store();
        let sched = build_schedule(
            "test-job".into(),
            "0 8 * * *".into(),
            "digest".into(),
            "telegram".into(),
        );
        store.insert(&sched).unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test-job");
        assert!(list[0].enabled);
    }

    #[test]
    fn get_by_name() {
        let (_dir, store) = make_store();
        let sched = build_schedule(
            "morning".into(),
            "0 9 * * 1-5".into(),
            "health_check".into(),
            "telegram".into(),
        );
        store.insert(&sched).unwrap();
        let found = store.get("morning").unwrap().unwrap();
        assert_eq!(found.action, "health_check");
        assert!(store.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn update_cron_and_enabled() {
        let (_dir, store) = make_store();
        let sched = build_schedule(
            "my-sched".into(),
            "0 6 * * *".into(),
            "reminder".into(),
            "telegram".into(),
        );
        store.insert(&sched).unwrap();
        store.update_cron("my-sched", "30 9 * * *").unwrap();
        store.set_enabled("my-sched", false).unwrap();
        let updated = store.get("my-sched").unwrap().unwrap();
        assert_eq!(updated.cron_expr, "30 9 * * *");
        assert!(!updated.enabled);
    }

    #[test]
    fn delete_returns_true_when_found() {
        let (_dir, store) = make_store();
        let sched = build_schedule(
            "gone".into(),
            "0 0 * * *".into(),
            "digest".into(),
            "telegram".into(),
        );
        store.insert(&sched).unwrap();
        assert!(store.delete("gone").unwrap());
        assert!(!store.delete("gone").unwrap());
    }

    #[test]
    fn mark_run_sets_last_run_at() {
        let (_dir, store) = make_store();
        let sched = build_schedule(
            "runner".into(),
            "*/5 * * * *".into(),
            "digest".into(),
            "telegram".into(),
        );
        store.insert(&sched).unwrap();
        assert!(store.get("runner").unwrap().unwrap().last_run_at.is_none());
        store.mark_run("runner").unwrap();
        assert!(store.get("runner").unwrap().unwrap().last_run_at.is_some());
    }

    #[test]
    fn validate_cron_expr_accepts_valid() {
        assert!(validate_cron_expr("0 8 * * *").is_ok());
        assert!(validate_cron_expr("*/5 * * * *").is_ok());
        assert!(validate_cron_expr("30 9 * * 1-5").is_ok());
    }

    #[test]
    fn validate_cron_expr_rejects_invalid() {
        assert!(validate_cron_expr("not-a-cron").is_err());
        assert!(validate_cron_expr("99 99 99 99 99").is_err());
    }

    #[test]
    fn validate_schedule_name_rules() {
        assert!(validate_schedule_name("morning-health").is_ok());
        assert!(validate_schedule_name("abc123").is_ok());
        assert!(validate_schedule_name("").is_err());
        assert!(validate_schedule_name("Has_Capitals").is_err());
        assert!(validate_schedule_name("has spaces").is_err());
    }
}
