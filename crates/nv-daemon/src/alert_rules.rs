//! Alert rule system — typed rules that evaluate conditions and produce obligations.
//!
//! Each `AlertRule` has a `rule_type` that maps to an `AlertRuleType` variant.
//! The `RuleEvaluator` trait is the contract every watcher implements: given
//! access to its external service, it returns `Some(NewObligation)` when the
//! rule condition is met and `None` when everything is healthy.
//!
//! The `AlertRuleStore` wraps the `alert_rules` table in `messages.db` for
//! CRUD operations. Watchers call `AlertRuleStore::touch_triggered` after
//! firing to update `last_triggered_at`.

use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::obligation_store::NewObligation;

// ── Rule Type ──────────────────────────────────────────────────────────

/// The category of condition an alert rule monitors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertRuleType {
    /// A Vercel deployment ended in a failed or error state.
    DeployFailure,
    /// Sentry error event count spiked above a configured threshold.
    SentrySpike,
    /// A tracked beads/Linear ticket has been open without activity for too long.
    StaleTicket,
    /// Home Assistant reported an anomalous state for a watched entity.
    HaAnomaly,
}

impl AlertRuleType {
    /// Canonical string representation stored in the DB.
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertRuleType::DeployFailure => "deploy_failure",
            AlertRuleType::SentrySpike => "sentry_spike",
            AlertRuleType::StaleTicket => "stale_ticket",
            AlertRuleType::HaAnomaly => "ha_anomaly",
        }
    }

    /// Parse from the DB string value.
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "deploy_failure" => Ok(AlertRuleType::DeployFailure),
            "sentry_spike" => Ok(AlertRuleType::SentrySpike),
            "stale_ticket" => Ok(AlertRuleType::StaleTicket),
            "ha_anomaly" => Ok(AlertRuleType::HaAnomaly),
            other => Err(anyhow::anyhow!("unknown alert rule type: {other}")),
        }
    }
}

// ── Alert Rule ─────────────────────────────────────────────────────────

/// A persisted alert rule row from the `alert_rules` table.
#[derive(Debug, Clone)]
#[allow(dead_code)] // fields read via pattern matching and tests
pub struct AlertRule {
    pub id: String,
    pub name: String,
    pub rule_type: AlertRuleType,
    /// JSON blob for rule-specific configuration (e.g. threshold, entity_id).
    pub config: Option<String>,
    pub enabled: bool,
    pub last_triggered_at: Option<String>,
    pub created_at: String,
}

// ── Evaluator Trait ────────────────────────────────────────────────────

/// Contract that every watcher implements.
///
/// Implementations are `async` but the trait itself is synchronous so that
/// watchers can be constructed without an async runtime. The `evaluate` method
/// is called from async watcher tasks.
pub trait RuleEvaluator {
    /// Evaluate the rule condition. Returns `Some(NewObligation)` if the rule
    /// fires, `None` if the service is healthy or cannot be reached.
    ///
    /// Implementations MUST be non-blocking (spawn blocking work on a thread
    /// pool if needed) and MUST NOT panic — log warnings for transient failures.
    fn evaluate(&self, rule: &AlertRule) -> impl std::future::Future<Output = Option<NewObligation>> + Send;
}

// ── Alert Rule Store ───────────────────────────────────────────────────

/// SQLite-backed store for alert rules.
///
/// Shares `messages.db` with `MessageStore` and `ObligationStore`. The schema
/// migration v3 that creates the `alert_rules` table is run by `MessageStore::init`.
/// `AlertRuleStore::new` mirrors the migration so it can open the DB independently.
pub struct AlertRuleStore {
    pub(crate) conn: Connection,
}

impl AlertRuleStore {
    /// Open the SQLite database.
    ///
    /// Schema migrations are managed exclusively by `MessageStore`. Callers must
    /// ensure `MessageStore::init` has been called before constructing an
    /// `AlertRuleStore` against the same `messages.db` path.
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        Ok(Self { conn })
    }

    /// Insert a new alert rule. Returns the created `AlertRule`.
    pub fn create(
        &self,
        id: &str,
        name: &str,
        rule_type: AlertRuleType,
        config: Option<&str>,
        enabled: bool,
    ) -> Result<AlertRule> {
        self.conn.execute(
            "INSERT INTO alert_rules (id, name, rule_type, config, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            params![id, name, rule_type.as_str(), config, enabled as i64],
        )?;

        self.get_by_name(name)?
            .ok_or_else(|| anyhow::anyhow!("alert rule not found after insert: {name}"))
    }

    /// Retrieve an alert rule by its unique name.
    pub fn get_by_name(&self, name: &str) -> Result<Option<AlertRule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, rule_type, config, enabled, last_triggered_at, created_at
             FROM alert_rules
             WHERE name = ?1",
        )?;

        let mut rows = stmt.query(params![name])?;

        match rows.next()? {
            Some(row) => Ok(Some(row_to_alert_rule(row)?)),
            None => Ok(None),
        }
    }

    /// List all enabled alert rules.
    pub fn list_enabled(&self) -> Result<Vec<AlertRule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, rule_type, config, enabled, last_triggered_at, created_at
             FROM alert_rules
             WHERE enabled = 1
             ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            row_to_alert_rule(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("list_enabled query failed: {e}"))
    }

    /// Update `last_triggered_at` for a rule to the current UTC time.
    ///
    /// Returns `true` if a row was updated, `false` if the name was not found.
    pub fn touch_triggered(&self, name: &str) -> Result<bool> {
        let rows_changed = self.conn.execute(
            "UPDATE alert_rules
             SET last_triggered_at = datetime('now')
             WHERE name = ?1",
            params![name],
        )?;

        Ok(rows_changed > 0)
    }

    /// Enable or disable a rule by name.
    ///
    /// Returns `true` if a row was updated.
    #[allow(dead_code)] // reserved for future API/CLI use
    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<bool> {
        let rows_changed = self.conn.execute(
            "UPDATE alert_rules SET enabled = ?1 WHERE name = ?2",
            params![enabled as i64, name],
        )?;

        Ok(rows_changed > 0)
    }
}

// ── Row Mapper ─────────────────────────────────────────────────────────

fn row_to_alert_rule(row: &rusqlite::Row<'_>) -> Result<AlertRule> {
    let rule_type_str: String = row.get(2)?;
    let enabled_int: i64 = row.get(4)?;

    Ok(AlertRule {
        id: row.get(0)?,
        name: row.get(1)?,
        rule_type: AlertRuleType::from_str(&rule_type_str)?,
        config: row.get(3)?,
        enabled: enabled_int != 0,
        last_triggered_at: row.get(5)?,
        created_at: row.get(6)?,
    })
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn temp_store() -> (AlertRuleStore, NamedTempFile) {
        let file = NamedTempFile::new().expect("temp file");
        let store = AlertRuleStore::new(file.path()).expect("store init");

        // Apply the alert_rules schema directly — MessageStore owns migrations in
        // production, but tests use a fresh temp DB with no MessageStore.
        store.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS alert_rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                rule_type TEXT NOT NULL,
                config TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                last_triggered_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_alert_rules_name ON alert_rules(name);
            CREATE INDEX IF NOT EXISTS idx_alert_rules_enabled ON alert_rules(enabled);"
        ).expect("test schema setup");

        (store, file)
    }

    #[test]
    fn create_and_get_by_name() {
        let (store, _f) = temp_store();

        let rule = store
            .create("id-1", "deploy_failure", AlertRuleType::DeployFailure, None, true)
            .unwrap();

        assert_eq!(rule.name, "deploy_failure");
        assert_eq!(rule.rule_type, AlertRuleType::DeployFailure);
        assert!(rule.enabled);
        assert!(rule.last_triggered_at.is_none());

        let fetched = store.get_by_name("deploy_failure").unwrap().expect("should exist");
        assert_eq!(fetched.id, rule.id);
    }

    #[test]
    fn list_enabled_excludes_disabled() {
        let (store, _f) = temp_store();

        store
            .create("id-1", "deploy_failure", AlertRuleType::DeployFailure, None, true)
            .unwrap();
        store
            .create("id-2", "sentry_spike", AlertRuleType::SentrySpike, None, false)
            .unwrap();

        let enabled = store.list_enabled().unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "deploy_failure");
    }

    #[test]
    fn touch_triggered_updates_timestamp() {
        let (store, _f) = temp_store();

        store
            .create("id-1", "ha_anomaly", AlertRuleType::HaAnomaly, None, true)
            .unwrap();

        let updated = store.touch_triggered("ha_anomaly").unwrap();
        assert!(updated);

        let rule = store.get_by_name("ha_anomaly").unwrap().unwrap();
        assert!(rule.last_triggered_at.is_some());
    }

    #[test]
    fn set_enabled_toggles_rule() {
        let (store, _f) = temp_store();

        store
            .create("id-1", "stale_ticket", AlertRuleType::StaleTicket, None, true)
            .unwrap();

        let toggled = store.set_enabled("stale_ticket", false).unwrap();
        assert!(toggled);

        let enabled = store.list_enabled().unwrap();
        assert_eq!(enabled.len(), 0);
    }

    #[test]
    fn alert_rule_type_roundtrip() {
        for rt in &[
            AlertRuleType::DeployFailure,
            AlertRuleType::SentrySpike,
            AlertRuleType::StaleTicket,
            AlertRuleType::HaAnomaly,
        ] {
            let s = rt.as_str();
            let parsed = AlertRuleType::from_str(s).expect("roundtrip failed");
            assert_eq!(*rt, parsed);
        }
    }
}
