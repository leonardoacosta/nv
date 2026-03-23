use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nv_core::types::{CronEvent, Trigger};
use tokio::sync::mpsc;

use crate::digest::state::DigestStateManager;
use crate::schedule_tools::{validate_cron_expr, ScheduleStore};

/// Minimum allowed digest interval (prevents runaway loops).
const MIN_INTERVAL_MINUTES: u64 = 5;

/// How often to poll user schedules for missed fires (seconds).
const USER_SCHEDULE_POLL_SECS: u64 = 60;

/// Spawn the cron scheduler task.
///
/// Pushes `Trigger::Cron(CronEvent::Digest)` into the trigger channel
/// at the configured interval. Reads `last-digest.json` on startup to
/// calculate initial delay (avoids firing immediately on restart if a
/// digest was sent recently).
///
/// Also polls user-defined schedules every 60 seconds, emitting
/// `Trigger::Cron(CronEvent::UserSchedule { name, action })` for any
/// schedule whose next-fire time has passed since `last_run_at`.
pub fn spawn_scheduler(
    trigger_tx: mpsc::UnboundedSender<Trigger>,
    interval_minutes: u64,
    nv_base: &Path,
    schedule_store: Option<Arc<Mutex<ScheduleStore>>>,
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

        // Start the regular digest interval
        let period = Duration::from_secs(interval_minutes * 60);
        let mut digest_interval = tokio::time::interval(period);
        // Skip the first tick since we already sent one above
        digest_interval.tick().await;

        // User schedule poll interval (60 seconds)
        let mut user_sched_interval =
            tokio::time::interval(Duration::from_secs(USER_SCHEDULE_POLL_SECS));
        // Skip the immediate first tick (allow init to settle)
        user_sched_interval.tick().await;

        loop {
            tokio::select! {
                _ = digest_interval.tick() => {
                    tracing::debug!("scheduler: digest tick");
                    if trigger_tx.send(Trigger::Cron(CronEvent::Digest)).is_err() {
                        tracing::info!("scheduler: trigger channel closed, shutting down");
                        break;
                    }
                }
                _ = user_sched_interval.tick() => {
                    if let Some(ref store_arc) = schedule_store {
                        poll_user_schedules(store_arc, &trigger_tx);
                    }
                }
            }
        }
    })
}

/// Check all enabled user schedules and emit triggers for any that have
/// missed their next fire time since `last_run_at`.
fn poll_user_schedules(
    store_arc: &Arc<Mutex<ScheduleStore>>,
    trigger_tx: &mpsc::UnboundedSender<Trigger>,
) {
    let store = match store_arc.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to lock schedule store in poll");
            return;
        }
    };

    let schedules = match store.list() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to list user schedules");
            return;
        }
    };

    let now = chrono::Utc::now();

    for sched in schedules {
        if !sched.enabled {
            continue;
        }

        // Parse the cron expression (5-field) into a cron::Schedule
        let cron_sched = match validate_cron_expr(&sched.cron_expr) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(name = %sched.name, error = %e, "invalid cron expr, skipping");
                continue;
            }
        };

        // Determine the reference time: last_run_at or the epoch
        let reference = sched
            .last_run_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH);

        // Check if the next fire time after the reference has passed
        let should_fire = cron_sched.after(&reference).next().is_some_and(|t| t <= now);

        if should_fire {
            tracing::info!(name = %sched.name, action = %sched.action, "user schedule firing");
            let trigger = Trigger::Cron(CronEvent::UserSchedule {
                name: sched.name.clone(),
                action: sched.action.clone(),
            });
            if trigger_tx.send(trigger).is_err() {
                tracing::warn!("scheduler: trigger channel closed during user schedule poll");
                return;
            }
            // Mark the schedule as run (update last_run_at)
            if let Err(e) = store.mark_run(&sched.name) {
                tracing::warn!(name = %sched.name, error = %e, "failed to mark user schedule as run");
            }
        }
    }
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
        let _handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path(), None);

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

        let handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path(), None);

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
        let _handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path(), None);

        // First tick should not arrive immediately (should wait ~5 minutes)
        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(result.is_err(), "should not get tick immediately after recent digest");
    }
}
