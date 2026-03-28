//! PgSessionEventWriter — buffered Postgres writer for session events.
//!
//! Buffers events in memory and flushes to the `session_events` table either
//! every 5 seconds or on session end, whichever comes first. Uses
//! fire-and-forget `tokio::spawn` to avoid blocking the orchestrator loop.

use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::pg_pool::PgPool;

/// A single session event to be flushed to Postgres.
#[derive(Debug, Clone)]
pub struct PendingSessionEvent {
    pub session_id: Uuid,
    pub event_type: String,
    pub direction: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Buffered session event writer.
///
/// Thread-safe via `Arc<Mutex<>>` on the internal buffer. The flush loop
/// runs as a background task.
#[derive(Clone)]
pub struct PgSessionEventWriter {
    pool: PgPool,
    buffer: Arc<Mutex<Vec<PendingSessionEvent>>>,
}

impl PgSessionEventWriter {
    /// Create a new writer and spawn the background flush loop.
    pub fn new(pool: PgPool) -> Self {
        let writer = Self {
            pool,
            buffer: Arc::new(Mutex::new(Vec::new())),
        };

        // Spawn background flush loop (fire-and-forget — avoids blocking orchestrator).
        let flush_writer = writer.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Err(e) = flush_writer.flush().await {
                    tracing::warn!(error = %e, "pg_session_events: flush failed");
                }
            }
        });

        writer
    }

    /// Enqueue a session event for batched writing.
    pub async fn push(&self, event: PendingSessionEvent) {
        self.buffer.lock().await.push(event);
    }

    /// Flush all buffered events to Postgres.
    ///
    /// Called by the background timer and on session end.
    pub async fn flush(&self) -> anyhow::Result<()> {
        let events: Vec<PendingSessionEvent> = {
            let mut buf = self.buffer.lock().await;
            if buf.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *buf)
        };

        let guard = match self.pool.client().await {
            Some(g) => g,
            None => {
                // Re-buffer the events so they're retried next flush.
                self.buffer.lock().await.extend(events);
                return Err(anyhow::anyhow!(
                    "pg_session_events: no Postgres connection"
                ));
            }
        };
        let client = guard.get();

        for event in &events {
            let id = Uuid::new_v4();
            if let Err(e) = client
                .execute(
                    "INSERT INTO session_events
                        (id, session_id, event_type, direction, content, metadata, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, NOW())",
                    &[
                        &id,
                        &event.session_id,
                        &event.event_type,
                        &event.direction,
                        &event.content,
                        &event.metadata,
                    ],
                )
                .await
            {
                tracing::warn!(
                    error = %e,
                    event_type = %event.event_type,
                    "pg_session_events: failed to insert event"
                );
            }
        }

        if !events.is_empty() {
            tracing::debug!(count = events.len(), "pg_session_events: flushed");
        }

        Ok(())
    }

    /// Convenience: push an event and immediately flush (used on session end).
    pub async fn push_and_flush(&self, event: PendingSessionEvent) {
        self.buffer.lock().await.push(event);
        if let Err(e) = self.flush().await {
            tracing::warn!(error = %e, "pg_session_events: push_and_flush failed");
        }
    }
}
