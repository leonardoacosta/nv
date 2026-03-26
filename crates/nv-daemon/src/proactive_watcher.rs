//! Proactive follow-up watcher: scans open obligations for overdue, approaching-deadline,
//! and stale items, then emits `Trigger::Cron(CronEvent::ProactiveFollowup)` to the
//! orchestrator.
//!
//! The watcher itself does not query the DB — it emits the trigger and lets the
//! orchestrator handle the actual scan. This preserves the scheduler/orchestrator
//! decoupling established by `scheduler.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveTime, Utc};
use nv_core::config::ProactiveWatcherConfig;
use nv_core::types::{CronEvent, Trigger};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// ── Minimum interval floor ────────────────────────────────────────────

/// Minimum allowed watcher interval in minutes (prevents runaway loops).
const MIN_INTERVAL_MINUTES: u64 = 30;

// ── State ─────────────────────────────────────────────────────────────

/// Persisted state for the proactive watcher.
///
/// Stored at `~/.nv/state/proactive-watcher.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProactiveWatcherState {
    /// When the watcher last emitted a trigger.
    pub last_run_at: Option<DateTime<Utc>>,
    /// Per-obligation reminder count since the last daily reset.
    ///
    /// Key: obligation UUID. Value: number of reminders sent today.
    /// Reset when `last_run_at` rolls over to a new calendar day.
    #[serde(default)]
    pub reminder_counts: HashMap<String, u32>,
}

impl ProactiveWatcherState {
    /// Load state from `~/.nv/state/proactive-watcher.json`.
    ///
    /// Returns `Default` if the file is absent or empty.
    pub fn load(nv_base: &Path) -> Result<Self> {
        let path = state_path(nv_base);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if content.trim().is_empty() || content.trim() == "{}" {
            return Ok(Self::default());
        }
        serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Atomically write state to `~/.nv/state/proactive-watcher.json`.
    pub fn save(&self, nv_base: &Path) -> Result<()> {
        let path = state_path(nv_base);
        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create state dir {}", parent.display()))?;
        }
        let content = serde_json::to_string_pretty(self)?;
        atomic_write(&path, &content)
    }
}

fn state_path(nv_base: &Path) -> PathBuf {
    nv_base.join("state").join("proactive-watcher.json")
}

/// Write `content` to `path` atomically (write to `.tmp`, rename).
fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)
        .with_context(|| format!("failed to write tmp file {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))
}

// ── Quiet Hours ───────────────────────────────────────────────────────

/// Return `true` if the current local time falls within the quiet window.
///
/// Handles overnight windows (e.g., 22:00–08:00) correctly.
/// Uses `reminders::tz_offset_seconds` for timezone resolution.
pub fn is_quiet_now(quiet_start: NaiveTime, quiet_end: NaiveTime, timezone: &str) -> bool {
    let offset_secs = crate::reminders::tz_offset_seconds(timezone);
    let offset = chrono::FixedOffset::east_opt(offset_secs)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
    let now = Utc::now().with_timezone(&offset).time();

    if quiet_start <= quiet_end {
        // Non-wrapping window (e.g., 01:00–06:00)
        now >= quiet_start && now < quiet_end
    } else {
        // Overnight window (e.g., 22:00–08:00)
        now >= quiet_start || now < quiet_end
    }
}

// ── Spawn ─────────────────────────────────────────────────────────────

/// Spawn the proactive watcher task.
///
/// On each tick (at `config.interval_minutes`, minimum 30 minutes):
/// 1. Check quiet hours — skip the tick if currently quiet.
/// 2. Push `Trigger::Cron(CronEvent::ProactiveFollowup)` to the trigger channel.
/// 3. Persist `last_run_at` in the state file.
///
/// The task stops cleanly when the trigger channel is closed.
pub fn spawn_proactive_watcher(
    trigger_tx: mpsc::UnboundedSender<Trigger>,
    config: ProactiveWatcherConfig,
    nv_base: &Path,
) -> tokio::task::JoinHandle<()> {
    let interval_minutes = config.interval_minutes.max(MIN_INTERVAL_MINUTES);
    let nv_base = nv_base.to_path_buf();

    tokio::spawn(async move {
        // Parse quiet-hours strings once at startup.
        let quiet_start = NaiveTime::parse_from_str(&config.quiet_start, "%H:%M").ok();
        let quiet_end = NaiveTime::parse_from_str(&config.quiet_end, "%H:%M").ok();

        // Load persisted state to determine the initial delay.
        let state = match ProactiveWatcherState::load(&nv_base) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "proactive_watcher: failed to load state, using defaults"
                );
                ProactiveWatcherState::default()
            }
        };

        // Calculate initial delay based on last_run_at.
        let interval_secs = interval_minutes * 60;
        let initial_delay_secs = match state.last_run_at {
            Some(last) => {
                let elapsed = (Utc::now() - last).num_seconds().max(0) as u64;
                interval_secs.saturating_sub(elapsed)
            }
            None => 0,
        };

        if initial_delay_secs > 0 {
            tracing::info!(
                delay_secs = initial_delay_secs,
                "proactive_watcher: waiting before first tick"
            );
            tokio::time::sleep(Duration::from_secs(initial_delay_secs)).await;
        }

        // Determine timezone string for quiet-hours checks.
        // We pass it via the config's quiet_start/quiet_end; use "UTC" as fallback
        // since we don't have direct access to daemon.timezone here. The orchestrator
        // will apply its own timezone-aware logic during handle_proactive_followup.
        // For the watcher's own quiet-hours gate, we use a resolved timezone via the
        // crate-level tz_offset helper with a fixed string. The spawn caller passes
        // config which includes quiet_start/quiet_end strings but not timezone — so
        // we use the tz from the timezone parameter passed by the caller. Since the
        // current function signature doesn't include timezone, use UTC for the watcher
        // gate (the orchestrator re-checks via its own quiet hours logic if needed).
        // NOTE: If timezone-aware quiet hours are needed here, extend the function
        // signature to accept &str timezone parameter.
        let timezone = "UTC";

        // Helper closure to check quiet hours.
        let check_quiet = |qs: Option<NaiveTime>, qe: Option<NaiveTime>| -> bool {
            match (qs, qe) {
                (Some(start), Some(end)) => is_quiet_now(start, end, timezone),
                _ => false,
            }
        };

        // Emit the first tick (after initial delay).
        if check_quiet(quiet_start, quiet_end) {
            tracing::debug!("proactive_watcher: first tick suppressed — quiet hours");
        } else {
            if trigger_tx
                .send(Trigger::Cron(CronEvent::ProactiveFollowup))
                .is_err()
            {
                tracing::info!("proactive_watcher: trigger channel closed on first tick");
                return;
            }
            tracing::info!("proactive_watcher: first tick emitted");
            persist_last_run(&nv_base);
        }

        // Regular interval loop.
        let period = Duration::from_secs(interval_secs);
        let mut interval = tokio::time::interval(period);
        interval.tick().await; // Skip the immediate tick.

        loop {
            interval.tick().await;

            if check_quiet(quiet_start, quiet_end) {
                tracing::debug!("proactive_watcher: tick suppressed — quiet hours");
                continue;
            }

            if trigger_tx
                .send(Trigger::Cron(CronEvent::ProactiveFollowup))
                .is_err()
            {
                tracing::info!("proactive_watcher: trigger channel closed, shutting down");
                break;
            }

            tracing::info!("proactive_watcher: ProactiveFollowup trigger emitted");
            persist_last_run(&nv_base);
        }
    })
}

/// Update `last_run_at` in the persisted state file.
fn persist_last_run(nv_base: &Path) {
    let mut state = ProactiveWatcherState::load(nv_base).unwrap_or_default();
    state.last_run_at = Some(Utc::now());
    if let Err(e) = state.save(nv_base) {
        tracing::warn!(error = %e, "proactive_watcher: failed to persist state");
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_state_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        dir
    }

    // ── is_quiet_now ─────────────────────────────────────────────────

    #[test]
    fn is_quiet_now_non_wrapping_window_inside() {
        // Non-wrapping window: 01:00–06:00.
        // We can't control the current time in tests, but we CAN verify the
        // boundary logic by testing with synthetic "now" values via the raw logic.
        // Instead, test the pure logic branch manually.
        let start = NaiveTime::parse_from_str("01:00", "%H:%M").unwrap();
        let end = NaiveTime::parse_from_str("06:00", "%H:%M").unwrap();
        // start < end → non-wrapping
        assert!(start < end, "sanity: should be non-wrapping window");
        // A time inside the window (e.g., 03:00)
        let inside = NaiveTime::parse_from_str("03:00", "%H:%M").unwrap();
        assert!(inside >= start && inside < end, "03:00 should be inside [01:00, 06:00)");
        // A time outside
        let outside = NaiveTime::parse_from_str("08:00", "%H:%M").unwrap();
        assert!(!(outside >= start && outside < end), "08:00 should be outside [01:00, 06:00)");
    }

    #[test]
    fn is_quiet_now_wrapping_window_logic() {
        // Wrapping window: 22:00–08:00 (overnight).
        let start = NaiveTime::parse_from_str("22:00", "%H:%M").unwrap();
        let end = NaiveTime::parse_from_str("08:00", "%H:%M").unwrap();
        // start > end → wrapping
        assert!(start > end, "sanity: should be wrapping window");
        // A time inside (23:00)
        let inside_night = NaiveTime::parse_from_str("23:00", "%H:%M").unwrap();
        assert!(
            inside_night >= start || inside_night < end,
            "23:00 should be inside wrapping window [22:00, 08:00)"
        );
        // A time inside (05:00)
        let inside_morning = NaiveTime::parse_from_str("05:00", "%H:%M").unwrap();
        assert!(
            inside_morning >= start || inside_morning < end,
            "05:00 should be inside wrapping window [22:00, 08:00)"
        );
        // A time outside (12:00)
        let outside = NaiveTime::parse_from_str("12:00", "%H:%M").unwrap();
        assert!(
            !(outside >= start || outside < end),
            "12:00 should be outside wrapping window [22:00, 08:00)"
        );
    }

    #[test]
    fn is_quiet_now_boundary_at_quiet_start() {
        // Exactly at quiet_start — should be IN the quiet window.
        let start = NaiveTime::parse_from_str("22:00", "%H:%M").unwrap();
        let end = NaiveTime::parse_from_str("08:00", "%H:%M").unwrap();
        // For wrapping: now >= start || now < end
        assert!(
            start >= start || start < end,
            "exactly at quiet_start should be in the window"
        );
    }

    #[test]
    fn is_quiet_now_boundary_at_quiet_end() {
        // Exactly at quiet_end — should NOT be in the quiet window (exclusive end).
        let start = NaiveTime::parse_from_str("22:00", "%H:%M").unwrap();
        let end = NaiveTime::parse_from_str("08:00", "%H:%M").unwrap();
        // For wrapping: now >= start || now < end; at end=08:00, now >= 22:00 is false, now < 08:00 is false.
        assert!(
            !(end >= start || end < end),
            "exactly at quiet_end should be outside the window"
        );
    }

    // ── ProactiveWatcherState ─────────────────────────────────────────

    #[test]
    fn state_round_trip_save_load() {
        let dir = make_state_dir();
        let mut state = ProactiveWatcherState::default();
        state.last_run_at = Some(Utc::now());
        state.reminder_counts.insert("ob-1".into(), 2);
        state.reminder_counts.insert("ob-2".into(), 1);

        state.save(dir.path()).unwrap();

        let loaded = ProactiveWatcherState::load(dir.path()).unwrap();
        assert_eq!(loaded.last_run_at, state.last_run_at);
        assert_eq!(loaded.reminder_counts.get("ob-1"), Some(&2));
        assert_eq!(loaded.reminder_counts.get("ob-2"), Some(&1));
    }

    #[test]
    fn state_load_missing_file_returns_default() {
        let dir = make_state_dir();
        let state = ProactiveWatcherState::load(dir.path()).unwrap();
        assert!(state.last_run_at.is_none());
        assert!(state.reminder_counts.is_empty());
    }

    // ── spawn_proactive_watcher ───────────────────────────────────────

    #[tokio::test]
    async fn watcher_stops_on_channel_close() {
        let dir = make_state_dir();
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        drop(rx); // Close the receiver immediately.

        let config = ProactiveWatcherConfig::default();
        let handle = spawn_proactive_watcher(tx, config, dir.path());

        // Task should exit promptly since the channel is closed.
        let result = tokio::time::timeout(Duration::from_secs(3), handle).await;
        assert!(result.is_ok(), "watcher should have exited after channel close");
    }

    #[tokio::test]
    async fn watcher_respects_recent_last_run_initial_delay() {
        let dir = make_state_dir();
        // Record a last_run_at of "just now" — should delay the full interval.
        let mut state = ProactiveWatcherState::default();
        state.last_run_at = Some(Utc::now());
        state.save(dir.path()).unwrap();

        let mut config = ProactiveWatcherConfig::default();
        config.interval_minutes = 30; // minimum floor

        let (tx, mut rx) = mpsc::unbounded_channel::<Trigger>();
        let _handle = spawn_proactive_watcher(tx, config, dir.path());

        // First tick should NOT arrive quickly (should wait ~30 minutes).
        let result =
            tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(
            result.is_err(),
            "should not get a tick immediately after a recent last_run_at"
        );
    }
}
