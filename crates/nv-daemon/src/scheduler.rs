use std::path::Path;
use std::time::Duration;

use nv_core::types::{CronEvent, Trigger};
use tokio::sync::mpsc;

use crate::digest::state::DigestStateManager;

/// Minimum allowed digest interval (prevents runaway loops).
const MIN_INTERVAL_MINUTES: u64 = 5;

/// Spawn the cron scheduler task.
///
/// Pushes `Trigger::Cron(CronEvent::Digest)` into the trigger channel
/// at the configured interval. Reads `last-digest.json` on startup to
/// calculate initial delay (avoids firing immediately on restart if a
/// digest was sent recently).
pub fn spawn_scheduler(
    trigger_tx: mpsc::UnboundedSender<Trigger>,
    interval_minutes: u64,
    nv_base: &Path,
) -> tokio::task::JoinHandle<()> {
    let interval_minutes = interval_minutes.max(MIN_INTERVAL_MINUTES);
    let nv_base = nv_base.to_path_buf();

    tokio::spawn(async move {
        let state_mgr = DigestStateManager::new(&nv_base);

        // Calculate initial delay based on last digest time
        let initial_delay = match state_mgr.seconds_until_next(interval_minutes) {
            Ok(secs) => Duration::from_secs(secs),
            Err(e) => {
                tracing::warn!(error = %e, "failed to read last digest state, using full interval");
                Duration::from_secs(interval_minutes * 60)
            }
        };

        if !initial_delay.is_zero() {
            tracing::info!(
                delay_secs = initial_delay.as_secs(),
                "scheduler: waiting before first digest tick"
            );
            tokio::time::sleep(initial_delay).await;
        }

        // Push the first tick (after initial delay)
        if trigger_tx.send(Trigger::Cron(CronEvent::Digest)).is_err() {
            tracing::error!("scheduler: trigger channel closed on first tick");
            return;
        }
        tracing::info!("scheduler: first digest tick sent");

        // Start the regular interval
        let period = Duration::from_secs(interval_minutes * 60);
        let mut interval = tokio::time::interval(period);
        // Skip the first tick since we already sent one above
        interval.tick().await;

        loop {
            interval.tick().await;

            tracing::debug!("scheduler: digest tick");
            if trigger_tx.send(Trigger::Cron(CronEvent::Digest)).is_err() {
                tracing::info!("scheduler: trigger channel closed, shutting down");
                break;
            }
        }
    })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_state_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let state_dir = dir.path().join("state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("last-digest.json"), "{}").unwrap();
        dir
    }

    #[test]
    fn min_interval_floor() {
        // Interval below minimum should be clamped
        let clamped = 3u64.max(MIN_INTERVAL_MINUTES);
        assert_eq!(clamped, 5);
    }

    #[tokio::test]
    async fn scheduler_sends_initial_tick() {
        let dir = setup_state_dir();
        let (tx, mut rx) = mpsc::unbounded_channel::<Trigger>();

        // State has no last_sent_at, so initial delay should be 0
        let _handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path());

        // Should receive the first tick quickly (no delay)
        let trigger = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for first tick")
            .expect("channel closed");

        match trigger {
            Trigger::Cron(CronEvent::Digest) => {} // Expected
            other => panic!("unexpected trigger: {other:?}"),
        }
    }

    #[tokio::test]
    async fn scheduler_stops_on_channel_close() {
        let dir = setup_state_dir();
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        drop(rx); // Close receiver immediately

        let handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path());

        // Scheduler should exit without hanging
        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "scheduler should have exited");
    }

    #[tokio::test]
    async fn scheduler_respects_recent_digest() {
        let dir = setup_state_dir();
        let mgr = DigestStateManager::new(dir.path());

        // Record a digest sent just now
        mgr.record_sent("sha256:test", vec![], std::collections::HashMap::new())
            .unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel::<Trigger>();
        let _handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path());

        // First tick should not arrive immediately (should wait ~5 minutes)
        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(result.is_err(), "should not get tick immediately after recent digest");
    }
}
