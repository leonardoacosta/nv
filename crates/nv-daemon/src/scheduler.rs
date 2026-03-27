use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Datelike, Timelike};
use nv_core::types::{CronEvent, Trigger};
use tokio::sync::mpsc;

use crate::digest::state::DigestStateManager;
use crate::messages::MessageStore;
use crate::tools::schedule::{validate_cron_expr, ScheduleStore};

// ── Cached briefing_hour from Postgres settings table ──────────────

/// How long to cache the briefing_hour value before re-querying Postgres.
const BRIEFING_HOUR_CACHE_SECS: u64 = 60;

/// Read `briefing_hour` from the Postgres `settings` table.
/// Caches the result for 60 seconds. Falls back to `MORNING_BRIEFING_HOUR` on error.
async fn read_briefing_hour(
    cache: &tokio::sync::Mutex<(Option<u32>, std::time::Instant)>,
) -> u32 {
    let mut guard = cache.lock().await;
    let (ref cached_val, ref last_fetch) = *guard;

    // Return cached value if still fresh
    if let Some(val) = cached_val {
        if last_fetch.elapsed() < Duration::from_secs(BRIEFING_HOUR_CACHE_SECS) {
            return *val;
        }
    }

    // Try to read from Postgres
    let result = read_briefing_hour_from_db().await;
    let hour = result.unwrap_or(MORNING_BRIEFING_HOUR);
    *guard = (Some(hour), std::time::Instant::now());
    hour
}

/// Query Postgres settings table for the "briefing_hour" key.
async fn read_briefing_hour_from_db() -> Result<u32, Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")?;

    let query = "SELECT value FROM settings WHERE key = 'briefing_hour'";

    // Connect with TLS if the URL indicates a cloud provider, otherwise no TLS.
    // Branches are separate because the connection types differ.
    if url.contains("sslmode=require") || url.contains("neon.tech") {
        let tls_config = rustls::ClientConfig::builder()
            .with_root_certificates({
                let mut roots = rustls::RootCertStore::empty();
                roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                roots
            })
            .with_no_client_auth();
        let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);

        let (client, connection) = tokio::time::timeout(
            Duration::from_secs(5),
            tokio_postgres::connect(&url, tls),
        )
        .await??;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::warn!(error = %e, "briefing_hour: postgres connection error (tls)");
            }
        });

        parse_briefing_hour_row(client.query_opt(query, &[]).await?)
    } else {
        let (client, connection) = tokio::time::timeout(
            Duration::from_secs(5),
            tokio_postgres::connect(&url, tokio_postgres::NoTls),
        )
        .await??;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::warn!(error = %e, "briefing_hour: postgres connection error (no-tls)");
            }
        });

        parse_briefing_hour_row(client.query_opt(query, &[]).await?)
    }
}

/// Parse the optional row from the settings query into a u32 hour.
fn parse_briefing_hour_row(
    row: Option<tokio_postgres::Row>,
) -> Result<u32, Box<dyn std::error::Error>> {
    match row {
        Some(r) => {
            let val: &str = r.get::<_, &str>(0);
            let hour: u32 = val.parse()?;
            Ok(hour.min(23)) // Clamp to valid hour range
        }
        None => Ok(MORNING_BRIEFING_HOUR),
    }
}

/// Minimum allowed digest interval (prevents runaway loops).
const MIN_INTERVAL_MINUTES: u64 = 5;

/// How often to poll user schedules for missed fires (seconds).
const USER_SCHEDULE_POLL_SECS: u64 = 60;

/// How often to poll for morning briefing fires (seconds).
/// We check every 60 seconds — same cadence as user schedules.
const MORNING_BRIEFING_POLL_SECS: u64 = 60;

/// How often to poll for weekly self-assessment fires (seconds).
const SELF_ASSESSMENT_POLL_SECS: u64 = 60;

/// Day-of-week for weekly self-assessment (0 = Sunday in chrono).
const SELF_ASSESSMENT_WEEKDAY: u32 = 0;

/// Hour range [start, end) during which the self-assessment may fire (local time).
const SELF_ASSESSMENT_HOUR_START: u32 = 0;
const SELF_ASSESSMENT_HOUR_END: u32 = 1;

/// Hour (24-hour, local time) at which the morning briefing fires.
const MORNING_BRIEFING_HOUR: u32 = 7;

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
    message_store: Option<Arc<Mutex<MessageStore>>>,
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

        // Digest is now merged into the morning briefing (fires once daily at 7am).
        // The standalone digest timer is removed. The interval_minutes param is
        // kept in the function signature for backward compatibility but ignored.
        let _ = interval_minutes;

        // User schedule poll interval (60 seconds)
        let mut user_sched_interval =
            tokio::time::interval(Duration::from_secs(USER_SCHEDULE_POLL_SECS));
        // Skip the immediate first tick (allow init to settle)
        user_sched_interval.tick().await;

        // Morning briefing poll interval (60 seconds)
        let mut morning_briefing_interval =
            tokio::time::interval(Duration::from_secs(MORNING_BRIEFING_POLL_SECS));
        morning_briefing_interval.tick().await;

        // Weekly self-assessment poll interval (60 seconds)
        let mut self_assessment_interval =
            tokio::time::interval(Duration::from_secs(SELF_ASSESSMENT_POLL_SECS));
        self_assessment_interval.tick().await;

        // Track the last date we sent a morning briefing to prevent duplicate fires.
        let mut last_briefing_date: Option<chrono::NaiveDate> = None;
        // Cache for briefing_hour read from Postgres settings table.
        let briefing_hour_cache: tokio::sync::Mutex<(Option<u32>, std::time::Instant)> =
            tokio::sync::Mutex::new((None, std::time::Instant::now()));
        // Track the last date we pruned latency_spans to fire once per day.
        let mut last_latency_prune_date: Option<chrono::NaiveDate> = None;
        // Track the last date we fired the weekly self-assessment.
        let mut last_assessment_date: Option<chrono::NaiveDate> = None;

        loop {
            tokio::select! {
                // Digest removed from standalone timer — now fires with morning briefing at 7am.
                _ = user_sched_interval.tick() => {
                    if let Some(ref store_arc) = schedule_store {
                        poll_user_schedules(store_arc, &trigger_tx);
                    }
                }
                _ = morning_briefing_interval.tick() => {
                    let now = chrono::Local::now();
                    let today = now.date_naive();
                    let current_hour = now.hour();

                    // Read configured briefing hour from Postgres (cached 60s,
                    // falls back to MORNING_BRIEFING_HOUR on error).
                    let configured_hour = read_briefing_hour(&briefing_hour_cache).await;

                    // Fire once per day at exactly the configured briefing hour.
                    // Using == instead of >= prevents spurious fires when the
                    // daemon restarts with a stale last_briefing_date at any
                    // hour after the configured hour (e.g. a 9pm restart would
                    // not re-trigger).
                    if current_hour == configured_hour
                        && last_briefing_date.is_none_or(|d| d < today)
                    {
                        last_briefing_date = Some(today);
                        tracing::info!(hour = current_hour, "scheduler: morning briefing + digest tick");
                        // Fire digest first (gathers data), then briefing (synthesizes it)
                        let _ = trigger_tx.send(Trigger::Cron(CronEvent::Digest));
                        if trigger_tx.send(Trigger::Cron(CronEvent::MorningBriefing)).is_err() {
                            tracing::info!("scheduler: trigger channel closed on morning briefing tick");
                            break;
                        }
                    }

                    // Nightly latency_spans pruning: delete rows older than 30 days.
                    // Fires once per day at any hour (not strictly at midnight) to
                    // keep the table bounded without requiring a dedicated cron event.
                    if last_latency_prune_date.is_none_or(|d| d < today) {
                        last_latency_prune_date = Some(today);
                        if let Some(ref ms_arc) = message_store {
                            match ms_arc.lock() {
                                Ok(store) => match store.prune_latency_spans() {
                                    Ok(n) => {
                                        if n > 0 {
                                            tracing::info!(
                                                rows_deleted = n,
                                                "scheduler: pruned old latency_spans rows"
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            error = %e,
                                            "scheduler: failed to prune latency_spans"
                                        );
                                    }
                                },
                                Err(e) => {
                                    tracing::warn!(
                                        error = %e,
                                        "scheduler: failed to lock message_store for latency prune"
                                    );
                                }
                            }
                        }
                    }
                }
                _ = self_assessment_interval.tick() => {
                    let now = chrono::Local::now();
                    let today = now.date_naive();
                    let current_hour = now.hour();
                    // weekday_from_monday(): Mon=0 .. Sun=6.
                    // We want Sunday, which is weekday index 6 in chrono.
                    let is_sunday = now.weekday().num_days_from_sunday() == SELF_ASSESSMENT_WEEKDAY;

                    // Fire once per week on Sunday in the 00:00–01:00 window.
                    if is_sunday
                        && (SELF_ASSESSMENT_HOUR_START..SELF_ASSESSMENT_HOUR_END).contains(&current_hour)
                        && last_assessment_date.is_none_or(|d| d < today)
                    {
                        last_assessment_date = Some(today);
                        tracing::info!(
                            hour = current_hour,
                            date = %today,
                            "scheduler: weekly self-assessment tick"
                        );
                        if trigger_tx
                            .send(Trigger::Cron(CronEvent::WeeklySelfAssessment))
                            .is_err()
                        {
                            tracing::info!("scheduler: trigger channel closed on self-assessment tick");
                            break;
                        }
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
        let _handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path(), None, None);

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

        let handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path(), None, None);

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
        let _handle = spawn_scheduler(tx, MIN_INTERVAL_MINUTES, dir.path(), None, None);

        // First tick should not arrive immediately (should wait ~5 minutes)
        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(result.is_err(), "should not get tick immediately after recent digest");
    }

    #[test]
    fn morning_briefing_does_not_fire_after_briefing_hour() {
        // With == check, only MORNING_BRIEFING_HOUR == 7 fires the briefing.
        // Hour 8 (MORNING_BRIEFING_HOUR + 1) must NOT match, even with stale last_briefing_date.
        let current_hour: u32 = MORNING_BRIEFING_HOUR + 1; // 8am
        // Simulate stale last_briefing_date from yesterday
        let last_briefing_date: Option<chrono::NaiveDate> = Some(
            chrono::Local::now().date_naive() - chrono::Duration::days(1)
        );
        let today = chrono::Local::now().date_naive();

        // With >=: this would fire (8 >= 7 is true, yesterday < today is true)
        // With ==: this must NOT fire (8 == 7 is false)
        let would_fire_with_gte = current_hour >= MORNING_BRIEFING_HOUR
            && last_briefing_date.is_none_or(|d| d < today);

        let would_fire_with_eq = current_hour == MORNING_BRIEFING_HOUR
            && last_briefing_date.is_none_or(|d| d < today);

        assert!(
            would_fire_with_gte,
            "Old >= check would fire at hour 8 with stale date (this is the bug we fixed)"
        );
        assert!(
            !would_fire_with_eq,
            "New == check must NOT fire at hour 8 — only fires at exactly hour 7"
        );
    }

    // ── Weekly self-assessment fire-condition unit tests ───────────────

    /// Simulate the fire-condition logic for WeeklySelfAssessment.
    fn should_fire_assessment(
        is_sunday: bool,
        current_hour: u32,
        last_assessment_date: Option<chrono::NaiveDate>,
        today: chrono::NaiveDate,
    ) -> bool {
        is_sunday
            && (SELF_ASSESSMENT_HOUR_START..SELF_ASSESSMENT_HOUR_END).contains(&current_hour)
            && last_assessment_date.is_none_or(|d| d < today)
    }

    #[test]
    fn assessment_fires_on_sunday_hour_zero_no_prior() {
        let today = chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(); // Sunday
        assert!(
            should_fire_assessment(true, 0, None, today),
            "must fire on Sunday hour 0 with no prior date"
        );
    }

    #[test]
    fn assessment_does_not_fire_on_non_sunday() {
        let today = chrono::NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday
        assert!(
            !should_fire_assessment(false, 0, None, today),
            "must NOT fire on a non-Sunday"
        );
    }

    #[test]
    fn assessment_does_not_fire_outside_hour_window() {
        let today = chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(); // Sunday
        // Hour 1 is outside the [0, 1) window.
        assert!(
            !should_fire_assessment(true, 1, None, today),
            "must NOT fire at hour 1 (outside window)"
        );
        // Hour 23 also outside window.
        assert!(
            !should_fire_assessment(true, 23, None, today),
            "must NOT fire at hour 23"
        );
    }

    #[test]
    fn assessment_does_not_fire_twice_same_day() {
        let today = chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(); // Sunday
        // Already ran today.
        assert!(
            !should_fire_assessment(true, 0, Some(today), today),
            "must NOT fire twice on the same day"
        );
    }

    #[test]
    fn assessment_fires_again_next_sunday() {
        let last_sunday = chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap();
        let this_sunday = chrono::NaiveDate::from_ymd_opt(2025, 1, 12).unwrap();
        assert!(
            should_fire_assessment(true, 0, Some(last_sunday), this_sunday),
            "must fire on a new Sunday when last_assessment_date is a prior week"
        );
    }
}
