//! Server health store: read/write operations on the `server_health` table.
//!
//! The schema is created by migration v4 in `messages.rs`. This module
//! provides typed access to the per-poll snapshots and historical queries.

use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

// ── Types ──────────────────────────────────────────────────────────────

/// A single server health snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHealthSnapshot {
    pub id: i64,
    pub timestamp: String,
    pub cpu_percent: Option<f64>,
    pub memory_used_mb: Option<i64>,
    pub memory_total_mb: Option<i64>,
    pub disk_used_gb: Option<f64>,
    pub disk_total_gb: Option<f64>,
    pub uptime_seconds: Option<i64>,
    pub load_avg_1m: Option<f64>,
    pub load_avg_5m: Option<f64>,
}

/// Values to insert for a new health snapshot.
#[derive(Debug, Clone)]
pub struct NewServerHealth {
    pub cpu_percent: Option<f64>,
    pub memory_used_mb: Option<i64>,
    pub memory_total_mb: Option<i64>,
    pub disk_used_gb: Option<f64>,
    pub disk_total_gb: Option<f64>,
    pub uptime_seconds: Option<i64>,
    pub load_avg_1m: Option<f64>,
    pub load_avg_5m: Option<f64>,
}

/// Overall health status classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
}

impl HealthStatus {
    /// Classify status from thresholds.
    ///
    /// - Critical: CPU ≥ 90% or memory ≥ 95% full
    /// - Degraded: CPU ≥ 70% or memory ≥ 80% full
    /// - Healthy: below both thresholds
    pub fn from_metrics(snapshot: &ServerHealthSnapshot) -> Self {
        let cpu_pct = snapshot.cpu_percent.unwrap_or(0.0);
        let mem_pct = snapshot
            .memory_used_mb
            .zip(snapshot.memory_total_mb)
            .map(|(used, total)| {
                if total > 0 {
                    (used as f64 / total as f64) * 100.0
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);

        if cpu_pct >= 90.0 || mem_pct >= 95.0 {
            HealthStatus::Critical
        } else if cpu_pct >= 70.0 || mem_pct >= 80.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
}

// ── Store ──────────────────────────────────────────────────────────────

/// SQLite-backed server health store.
pub struct ServerHealthStore {
    conn: Connection,
}

impl ServerHealthStore {
    /// Open (or create) the SQLite database. Relies on `MessageStore::init`
    /// having run migration v4 first.
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        Ok(Self { conn })
    }

    /// Insert a new health snapshot. Returns the inserted row id.
    pub fn insert(&self, health: &NewServerHealth) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO server_health
                (timestamp, cpu_percent, memory_used_mb, memory_total_mb,
                 disk_used_gb, disk_total_gb, uptime_seconds, load_avg_1m, load_avg_5m)
             VALUES
                (datetime('now'), ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                health.cpu_percent,
                health.memory_used_mb,
                health.memory_total_mb,
                health.disk_used_gb,
                health.disk_total_gb,
                health.uptime_seconds,
                health.load_avg_1m,
                health.load_avg_5m,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return the most recent health snapshot, or `None` if the table is empty.
    pub fn latest(&self) -> Result<Option<ServerHealthSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, cpu_percent, memory_used_mb, memory_total_mb,
                    disk_used_gb, disk_total_gb, uptime_seconds, load_avg_1m, load_avg_5m
             FROM server_health
             ORDER BY id DESC
             LIMIT 1",
        )?;

        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_snapshot(row)?)),
            None => Ok(None),
        }
    }

    /// Return the second-most-recent snapshot (used for uptime comparison).
    #[allow(dead_code)] // reserved for uptime delta calculation in future health poller
    pub fn previous(&self) -> Result<Option<ServerHealthSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, cpu_percent, memory_used_mb, memory_total_mb,
                    disk_used_gb, disk_total_gb, uptime_seconds, load_avg_1m, load_avg_5m
             FROM server_health
             ORDER BY id DESC
             LIMIT 1 OFFSET 1",
        )?;

        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_snapshot(row)?)),
            None => Ok(None),
        }
    }

    /// Return up to `limit` snapshots from the last 24 hours, oldest first.
    pub fn history_24h(&self, limit: usize) -> Result<Vec<ServerHealthSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, cpu_percent, memory_used_mb, memory_total_mb,
                    disk_used_gb, disk_total_gb, uptime_seconds, load_avg_1m, load_avg_5m
             FROM server_health
             WHERE timestamp >= datetime('now', '-24 hours')
             ORDER BY id ASC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            row_to_snapshot(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("history_24h query failed: {e}"))
    }

    /// Prune snapshots older than `days` days to prevent unbounded growth.
    pub fn prune_older_than_days(&self, days: u32) -> Result<usize> {
        let deleted = self.conn.execute(
            "DELETE FROM server_health WHERE timestamp < datetime('now', ?1)",
            params![format!("-{days} days")],
        )?;
        Ok(deleted)
    }
}

// ── Row mapper ─────────────────────────────────────────────────────────

fn row_to_snapshot(row: &rusqlite::Row<'_>) -> Result<ServerHealthSnapshot> {
    Ok(ServerHealthSnapshot {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        cpu_percent: row.get(2)?,
        memory_used_mb: row.get(3)?,
        memory_total_mb: row.get(4)?,
        disk_used_gb: row.get(5)?,
        disk_total_gb: row.get(6)?,
        uptime_seconds: row.get(7)?,
        load_avg_1m: row.get(8)?,
        load_avg_5m: row.get(9)?,
    })
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// Build a store backed by a fresh temp file, having run the migration
    /// manually so we don't depend on MessageStore.
    fn temp_store() -> (ServerHealthStore, NamedTempFile) {
        let file = NamedTempFile::new().expect("temp file");
        let conn = Connection::open(file.path()).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS server_health (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                cpu_percent REAL,
                memory_used_mb INTEGER,
                memory_total_mb INTEGER,
                disk_used_gb REAL,
                disk_total_gb REAL,
                uptime_seconds INTEGER,
                load_avg_1m REAL,
                load_avg_5m REAL
            );
            CREATE INDEX IF NOT EXISTS idx_server_health_timestamp
                ON server_health(timestamp);",
        )
        .unwrap();
        drop(conn);
        let store = ServerHealthStore::new(file.path()).unwrap();
        (store, file)
    }

    fn sample_health() -> NewServerHealth {
        NewServerHealth {
            cpu_percent: Some(45.0),
            memory_used_mb: Some(4096),
            memory_total_mb: Some(8192),
            disk_used_gb: Some(120.0),
            disk_total_gb: Some(500.0),
            uptime_seconds: Some(3600),
            load_avg_1m: Some(1.2),
            load_avg_5m: Some(0.9),
        }
    }

    #[test]
    fn insert_and_latest() {
        let (store, _f) = temp_store();
        assert!(store.latest().unwrap().is_none());

        store.insert(&sample_health()).unwrap();

        let snap = store.latest().unwrap().expect("should exist");
        assert_eq!(snap.cpu_percent, Some(45.0));
        assert_eq!(snap.memory_used_mb, Some(4096));
        assert_eq!(snap.uptime_seconds, Some(3600));
    }

    #[test]
    fn previous_returns_second_row() {
        let (store, _f) = temp_store();

        let first = NewServerHealth {
            uptime_seconds: Some(100),
            ..sample_health()
        };
        let second = NewServerHealth {
            uptime_seconds: Some(200),
            ..sample_health()
        };
        store.insert(&first).unwrap();
        store.insert(&second).unwrap();

        let prev = store.previous().unwrap().expect("should exist");
        assert_eq!(prev.uptime_seconds, Some(100));

        let latest = store.latest().unwrap().expect("should exist");
        assert_eq!(latest.uptime_seconds, Some(200));
    }

    #[test]
    fn health_status_classification() {
        let mut snap = ServerHealthSnapshot {
            id: 1,
            timestamp: "2026-01-01 00:00:00".into(),
            cpu_percent: Some(30.0),
            memory_used_mb: Some(2000),
            memory_total_mb: Some(8000),
            disk_used_gb: None,
            disk_total_gb: None,
            uptime_seconds: None,
            load_avg_1m: None,
            load_avg_5m: None,
        };
        assert_eq!(HealthStatus::from_metrics(&snap), HealthStatus::Healthy);

        snap.cpu_percent = Some(75.0);
        assert_eq!(HealthStatus::from_metrics(&snap), HealthStatus::Degraded);

        snap.cpu_percent = Some(92.0);
        assert_eq!(HealthStatus::from_metrics(&snap), HealthStatus::Critical);
    }
}
