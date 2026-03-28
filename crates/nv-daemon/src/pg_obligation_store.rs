//! PgObligationStore — Postgres-backed obligation CRUD.
//!
//! Mirrors the interface of `ObligationStore` (SQLite) but writes to the
//! Postgres `obligations` table via `PgPool`. Used for dual-write during
//! the migration period.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::obligation_store::NewObligation;
use crate::pg_pool::PgPool;

/// Postgres-backed obligation store.
///
/// All methods are async and return `Result<T>`. Callers must handle errors
/// gracefully — PG failures must never crash the daemon.
#[derive(Clone)]
pub struct PgObligationStore {
    pool: PgPool,
}

impl PgObligationStore {
    /// Create a new store backed by the given PgPool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new obligation into the Postgres `obligations` table.
    pub async fn create(&self, new: &NewObligation) -> anyhow::Result<()> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_obligation_store: no Postgres connection"))?;
        let client = guard.get();

        // Parse the UUID string to a native Uuid for the PG uuid column.
        let id: Uuid = new.id.parse()?;
        let status = nv_core::types::ObligationStatus::Open.as_str();
        let owner = new.owner.as_str();

        // deadline: parse RFC 3339 string to NaiveDateTime for the PG timestamp column.
        let deadline: Option<chrono::NaiveDateTime> = new
            .deadline
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.naive_utc());

        client
            .execute(
                "INSERT INTO obligations
                    (id, detected_action, owner, status, priority, project_code,
                     source_channel, source_message, deadline, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())",
                &[
                    &id,
                    &new.detected_action,
                    &owner,
                    &status,
                    &new.priority,
                    &new.project_code,
                    &new.source_channel,
                    &new.source_message,
                    &deadline,
                ],
            )
            .await?;

        tracing::debug!(obligation_id = %new.id, "pg_obligation_store: created");
        Ok(())
    }

    /// Update the status of an obligation.
    pub async fn update_status(
        &self,
        id: &str,
        new_status: &nv_core::types::ObligationStatus,
    ) -> anyhow::Result<bool> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_obligation_store: no Postgres connection"))?;
        let client = guard.get();

        let uuid: Uuid = id.parse()?;
        let rows = client
            .execute(
                "UPDATE obligations SET status = $1, updated_at = NOW() WHERE id = $2",
                &[&new_status.as_str(), &uuid],
            )
            .await?;

        Ok(rows > 0)
    }

    /// Update both status and owner of an obligation.
    pub async fn update_status_and_owner(
        &self,
        id: &str,
        new_status: &nv_core::types::ObligationStatus,
        new_owner: &nv_core::types::ObligationOwner,
    ) -> anyhow::Result<bool> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_obligation_store: no Postgres connection"))?;
        let client = guard.get();

        let uuid: Uuid = id.parse()?;
        let rows = client
            .execute(
                "UPDATE obligations SET status = $1, owner = $2, updated_at = NOW() WHERE id = $3",
                &[&new_status.as_str(), &new_owner.as_str(), &uuid],
            )
            .await?;

        Ok(rows > 0)
    }

    /// Update the detected_action text of an obligation.
    pub async fn update_detected_action(
        &self,
        id: &str,
        new_text: &str,
    ) -> anyhow::Result<bool> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_obligation_store: no Postgres connection"))?;
        let client = guard.get();

        let uuid: Uuid = id.parse()?;
        let rows = client
            .execute(
                "UPDATE obligations SET detected_action = $1, updated_at = NOW() WHERE id = $2",
                &[&new_text, &uuid],
            )
            .await?;

        Ok(rows > 0)
    }

    /// Reset the staleness clock (snooze) by touching updated_at.
    pub async fn snooze(&self, id: &str) -> anyhow::Result<bool> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_obligation_store: no Postgres connection"))?;
        let client = guard.get();

        let uuid: Uuid = id.parse()?;
        let rows = client
            .execute(
                "UPDATE obligations SET updated_at = NOW() WHERE id = $1 AND status = 'open'",
                &[&uuid],
            )
            .await?;

        Ok(rows > 0)
    }

    /// Update the last_attempt_at timestamp for autonomous execution cooldown.
    pub async fn update_last_attempt_at(
        &self,
        id: &str,
        timestamp: &DateTime<Utc>,
    ) -> anyhow::Result<bool> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_obligation_store: no Postgres connection"))?;
        let client = guard.get();

        let uuid: Uuid = id.parse()?;
        let ts = timestamp.naive_utc();
        let rows = client
            .execute(
                "UPDATE obligations SET last_attempt_at = $1, updated_at = NOW() WHERE id = $2",
                &[&ts, &uuid],
            )
            .await?;

        Ok(rows > 0)
    }
}
