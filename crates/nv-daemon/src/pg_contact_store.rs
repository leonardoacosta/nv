//! PgContactStore — Postgres-backed contact CRUD.
//!
//! Mirrors the interface of `ContactStore` (SQLite) but writes to the
//! Postgres `contacts` table via `PgPool`. Used for dual-write during
//! the migration period.

use uuid::Uuid;

use crate::pg_pool::PgPool;

/// Postgres-backed contact store.
///
/// All methods are async and return `Result<T>`. Callers must handle errors
/// gracefully — PG failures must never crash the daemon.
#[derive(Clone)]
pub struct PgContactStore {
    pool: PgPool,
}

impl PgContactStore {
    /// Create a new store backed by the given PgPool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new contact into the Postgres `contacts` table.
    ///
    /// `channel_ids` is stored as JSONB (via `serde_json::Value`).
    pub async fn create(
        &self,
        name: &str,
        channel_ids: &serde_json::Value,
        relationship_type: &str,
        notes: Option<&str>,
    ) -> anyhow::Result<String> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_contact_store: no Postgres connection"))?;
        let client = guard.get();

        let id = Uuid::new_v4();

        // Serialize channel_ids to a serde_json::Value for the JSONB param.
        // tokio-postgres accepts serde_json::Value for JSONB columns natively
        // when the `with-serde_json-1` feature is enabled.
        client
            .execute(
                "INSERT INTO contacts (id, name, channel_ids, relationship_type, notes, created_at)
                 VALUES ($1, $2, $3, $4, $5, NOW())",
                &[&id, &name, channel_ids, &relationship_type, &notes],
            )
            .await?;

        tracing::debug!(contact_id = %id, name, "pg_contact_store: created");
        Ok(id.to_string())
    }

    /// Update fields on an existing contact. Pass `None` to leave unchanged.
    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        channel_ids: Option<&serde_json::Value>,
        relationship_type: Option<&str>,
        notes: Option<&str>,
    ) -> anyhow::Result<bool> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_contact_store: no Postgres connection"))?;
        let client = guard.get();

        let uuid: Uuid = id.parse()?;
        let mut updated = false;

        if let Some(n) = name {
            client
                .execute(
                    "UPDATE contacts SET name = $1 WHERE id = $2",
                    &[&n, &uuid],
                )
                .await?;
            updated = true;
        }
        if let Some(cids) = channel_ids {
            client
                .execute(
                    "UPDATE contacts SET channel_ids = $1 WHERE id = $2",
                    &[cids, &uuid],
                )
                .await?;
            updated = true;
        }
        if let Some(rt) = relationship_type {
            client
                .execute(
                    "UPDATE contacts SET relationship_type = $1 WHERE id = $2",
                    &[&rt, &uuid],
                )
                .await?;
            updated = true;
        }
        if let Some(n) = notes {
            client
                .execute(
                    "UPDATE contacts SET notes = $1 WHERE id = $2",
                    &[&n, &uuid],
                )
                .await?;
            updated = true;
        }

        Ok(updated)
    }

    /// Delete a contact by ID.
    pub async fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let guard = self
            .pool
            .client()
            .await
            .ok_or_else(|| anyhow::anyhow!("pg_contact_store: no Postgres connection"))?;
        let client = guard.get();

        let uuid: Uuid = id.parse()?;
        let rows = client
            .execute("DELETE FROM contacts WHERE id = $1", &[&uuid])
            .await?;

        Ok(rows > 0)
    }
}
