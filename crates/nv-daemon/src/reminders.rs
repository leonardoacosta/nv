//! Reminders system: SQLite-backed one-shot timers with background polling.
//!
//! Claude calls `set_reminder`, `list_reminders`, and `cancel_reminder` tools.
//! A background tokio task polls every 30s for due reminders and fires them
//! to the originating channel.
//!
//! All storage uses UTC. Display converts to the user's configured timezone
//! via a UTC offset lookup (common US timezones supported; defaults to UTC).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, NaiveDate, NaiveDateTime, NaiveTime, Utc, Weekday};
use nv_core::channel::Channel;
use nv_core::types::OutboundMessage;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use tracing::{error, info, warn};

/// Polling interval for the reminder scheduler.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

// ── Timezone Offset Helper ───────────────────────────────────────────

/// Return the UTC offset in seconds for a named IANA timezone.
///
/// Supports common US timezones. Defaults to 0 (UTC) for unknown zones.
/// DST detection: picks standard/daylight offset based on current month.
fn tz_offset_seconds(tz_name: &str) -> i32 {
    let now = Utc::now();
    let month = now.month();
    // Simple DST approximation: Mar–Nov is daylight time in most US zones
    let is_dst = (3..=11).contains(&month);

    match tz_name {
        "America/New_York" | "US/Eastern" => {
            if is_dst { -4 * 3600 } else { -5 * 3600 }
        }
        "America/Chicago" | "US/Central" => {
            if is_dst { -5 * 3600 } else { -6 * 3600 }
        }
        "America/Denver" | "US/Mountain" => {
            if is_dst { -6 * 3600 } else { -7 * 3600 }
        }
        "America/Los_Angeles" | "US/Pacific" => {
            if is_dst { -7 * 3600 } else { -8 * 3600 }
        }
        "America/Phoenix" => -7 * 3600,     // no DST
        "America/Anchorage" => {
            if is_dst { -8 * 3600 } else { -9 * 3600 }
        }
        "Pacific/Honolulu" => -10 * 3600,   // no DST
        "UTC" | "Etc/UTC" => 0,
        _ => 0,
    }
}

/// Convert UTC datetime to a display string in the given timezone.
fn to_local_display(utc: &DateTime<Utc>, tz_name: &str) -> String {
    let offset_secs = tz_offset_seconds(tz_name);
    let local = *utc + ChronoDuration::seconds(offset_secs as i64);
    local.format("%Y-%m-%d %H:%M").to_string()
}

/// Get the current local NaiveDateTime for the given timezone.
fn local_now(tz_name: &str) -> NaiveDateTime {
    let offset_secs = tz_offset_seconds(tz_name);
    let utc_now = Utc::now();
    let local = utc_now + ChronoDuration::seconds(offset_secs as i64);
    local.naive_utc()
}

// ── Time Parsing ─────────────────────────────────────────────────────

/// Parse a relative or absolute time expression into a UTC `DateTime<Utc>`.
///
/// Supported formats:
/// - `2h` / `2 hours` — add hours to now
/// - `30m` / `30min` / `30 minutes` — add minutes
/// - `1d` / `1 day` / `1 days` — add days
/// - `tomorrow` — next day at 09:00 local
/// - `tomorrow 9am` / `tomorrow 14:00` — next day at specified time
/// - `next Monday` — next occurrence of weekday at 09:00
/// - `next Monday 2pm` / `next Monday 14:00` — weekday + time
/// - ISO 8601 datetime string — parsed directly
pub fn parse_relative_time(input: &str, timezone: &str) -> Result<DateTime<Utc>> {
    let s = input.trim().to_lowercase();

    // ── Duration short-form: "2h", "30m", "1d" ───────────────────
    if let Some(rest) = s.strip_suffix('h') {
        if let Ok(n) = rest.trim().parse::<i64>() {
            return Ok(Utc::now() + ChronoDuration::hours(n));
        }
    }
    if s.ends_with('m') && !s.contains("min") && !s.contains("month") {
        let rest = &s[..s.len() - 1];
        if let Ok(n) = rest.trim().parse::<i64>() {
            return Ok(Utc::now() + ChronoDuration::minutes(n));
        }
    }
    if let Some(rest) = s.strip_suffix('d') {
        if let Ok(n) = rest.trim().parse::<i64>() {
            return Ok(Utc::now() + ChronoDuration::days(n));
        }
    }

    // ── Duration long-form: "2 hours", "30 minutes", "30 min", "1 day" ──
    let parts: Vec<&str> = s.splitn(3, ' ').collect();
    if parts.len() >= 2 {
        if let Ok(n) = parts[0].parse::<i64>() {
            match parts[1] {
                "h" | "hr" | "hrs" | "hour" | "hours" => {
                    return Ok(Utc::now() + ChronoDuration::hours(n));
                }
                "m" | "min" | "mins" | "minute" | "minutes" => {
                    return Ok(Utc::now() + ChronoDuration::minutes(n));
                }
                "d" | "day" | "days" => {
                    return Ok(Utc::now() + ChronoDuration::days(n));
                }
                _ => {}
            }
        }
    }

    // ── Natural date: "tomorrow", "tomorrow 9am", etc. ────────────
    if s.starts_with("tomorrow") {
        let local = local_now(timezone);
        let tomorrow = local.date() + ChronoDuration::days(1);
        let time_part = s.strip_prefix("tomorrow").unwrap_or("").trim();
        let time = if time_part.is_empty() {
            NaiveTime::from_hms_opt(9, 0, 0).unwrap()
        } else {
            parse_time_of_day(time_part)?
        };
        let local_dt = tomorrow.and_time(time);
        return local_naive_to_utc(local_dt, timezone);
    }

    // ── Natural date: "next Monday", "next Monday 2pm" ────────────
    if let Some(rest) = s.strip_prefix("next ") {
        let weekday_and_time: Vec<&str> = rest.splitn(2, ' ').collect();
        let weekday_str = weekday_and_time[0];
        let time_str = weekday_and_time.get(1).copied().unwrap_or("");

        if let Some(target_weekday) = parse_weekday(weekday_str) {
            let local = local_now(timezone);
            let today = local.date();
            let days_until = days_until_weekday(today.weekday(), target_weekday);
            // "next X" should be at least 1 day away
            let days_to_add = if days_until == 0 { 7 } else { days_until };
            let target_date = today + ChronoDuration::days(days_to_add as i64);
            let time = if time_str.is_empty() {
                NaiveTime::from_hms_opt(9, 0, 0).unwrap()
            } else {
                parse_time_of_day(time_str)?
            };
            let local_dt = target_date.and_time(time);
            return local_naive_to_utc(local_dt, timezone);
        }
    }

    // ── ISO 8601 passthrough ──────────────────────────────────────
    // Try RFC 3339 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(input.trim()) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Try naive datetime
    for fmt in &["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M"] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(input.trim(), fmt) {
            return local_naive_to_utc(ndt, timezone);
        }
    }
    // Try date-only
    if let Ok(nd) = NaiveDate::parse_from_str(input.trim(), "%Y-%m-%d") {
        let ndt = nd.and_hms_opt(9, 0, 0).unwrap();
        return local_naive_to_utc(ndt, timezone);
    }

    Err(anyhow!(
        "Could not parse time expression '{}'. \
        Try formats like '2h', '30m', '1d', 'tomorrow', 'tomorrow 9am', \
        'next Monday', 'next Monday 2pm', or an ISO 8601 datetime.",
        input
    ))
}

/// Parse a time-of-day string like "9am", "2pm", "14:00", "09:30".
fn parse_time_of_day(s: &str) -> Result<NaiveTime> {
    let s = s.trim();

    // Try 12h with am/pm
    if let Some(rest) = s.strip_suffix("am") {
        let hour: u32 = rest.trim().parse().context("invalid hour")?;
        let hour = if hour == 12 { 0 } else { hour };
        return NaiveTime::from_hms_opt(hour, 0, 0)
            .ok_or_else(|| anyhow!("invalid time: {s}"));
    }
    if let Some(rest) = s.strip_suffix("pm") {
        let hour: u32 = rest.trim().parse().context("invalid hour")?;
        let hour = if hour == 12 { 12 } else { hour + 12 };
        return NaiveTime::from_hms_opt(hour, 0, 0)
            .ok_or_else(|| anyhow!("invalid time: {s}"));
    }

    // Try HH:MM
    if s.contains(':') {
        return NaiveTime::parse_from_str(s, "%H:%M")
            .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M:%S"))
            .context(format!("invalid time format: {s}"));
    }

    // Bare hour like "9" or "14"
    if let Ok(h) = s.parse::<u32>() {
        return NaiveTime::from_hms_opt(h, 0, 0)
            .ok_or_else(|| anyhow!("invalid hour: {h}"));
    }

    Err(anyhow!("could not parse time: '{s}'"))
}

/// Parse a weekday name (case-insensitive, full or abbreviated).
fn parse_weekday(s: &str) -> Option<Weekday> {
    match s.to_lowercase().as_str() {
        "mon" | "monday" => Some(Weekday::Mon),
        "tue" | "tuesday" => Some(Weekday::Tue),
        "wed" | "wednesday" => Some(Weekday::Wed),
        "thu" | "thursday" => Some(Weekday::Thu),
        "fri" | "friday" => Some(Weekday::Fri),
        "sat" | "saturday" => Some(Weekday::Sat),
        "sun" | "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

/// Number of days from `from` until `target` (0 if same day, 1–6 otherwise).
fn days_until_weekday(from: Weekday, target: Weekday) -> u32 {
    let from_n = from.num_days_from_monday();
    let target_n = target.num_days_from_monday();
    (target_n + 7 - from_n) % 7
}

/// Convert a local NaiveDateTime to UTC using the given timezone name.
fn local_naive_to_utc(ndt: NaiveDateTime, tz_name: &str) -> Result<DateTime<Utc>> {
    let offset_secs = tz_offset_seconds(tz_name);
    // local = utc + offset  =>  utc = local - offset
    let utc_ndt = ndt - ChronoDuration::seconds(offset_secs as i64);
    Ok(DateTime::from_naive_utc_and_offset(utc_ndt, Utc))
}

// ── Reminder Struct ──────────────────────────────────────────────────

/// A reminder row from the database.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Reminder {
    pub id: i64,
    pub message: String,
    pub due_at: String,   // ISO 8601 UTC
    pub channel: String,
    pub created_at: String,
    pub delivered_at: Option<String>,
    pub cancelled: bool,
}

/// Versioned migrations for reminders.db (schedules.db also uses reminders).
///
/// Version 1 is the initial schema, converting CREATE TABLE IF NOT EXISTS to a
/// migration so future ALTER TABLE changes are safe.
fn reminders_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE IF NOT EXISTS reminders (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                message      TEXT NOT NULL,
                due_at       TEXT NOT NULL,
                channel      TEXT NOT NULL,
                created_at   TEXT NOT NULL,
                delivered_at TEXT,
                cancelled    INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_reminders_due_at ON reminders(due_at);

            CREATE INDEX IF NOT EXISTS idx_reminders_active ON reminders(cancelled, delivered_at);",
        ),
    ])
}

// ── ReminderStore ────────────────────────────────────────────────────

/// SQLite-backed reminder store (owns its own connection to avoid !Send issues
/// with shared connections).
pub struct ReminderStore {
    conn: Connection,
}

impl ReminderStore {
    /// Open the database and run versioned migrations to ensure the schema is current.
    ///
    /// Uses PRAGMA user_version to track schema version. Safe for ALTER TABLE
    /// changes in future migration versions.
    pub fn new(db_path: &Path) -> Result<Self> {
        let mut conn = Connection::open(db_path)
            .context("failed to open reminders database")?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .context("failed to set WAL mode on reminders database")?;

        reminders_migrations()
            .to_latest(&mut conn)
            .map_err(|e| anyhow!("failed to run reminders.db migrations: {e}"))?;

        Ok(Self { conn })
    }

    /// Insert a new reminder and return its auto-increment ID.
    pub fn create_reminder(
        &self,
        message: &str,
        due_at: &DateTime<Utc>,
        channel: &str,
    ) -> Result<i64> {
        let due_str = due_at.to_rfc3339();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO reminders (message, due_at, channel, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![message, due_str, channel, now],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Return all active (not cancelled, not delivered) reminders ordered by due_at.
    pub fn list_active_reminders(&self) -> Result<Vec<Reminder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message, due_at, channel, created_at, delivered_at, cancelled
             FROM reminders
             WHERE cancelled = 0 AND delivered_at IS NULL
             ORDER BY due_at ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                message: row.get(1)?,
                due_at: row.get(2)?,
                channel: row.get(3)?,
                created_at: row.get(4)?,
                delivered_at: row.get(5)?,
                cancelled: row.get::<_, i64>(6)? != 0,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to list active reminders")
    }

    /// Cancel a reminder by ID. Returns true if the row existed.
    pub fn cancel_reminder(&self, id: i64) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE reminders SET cancelled = 1 WHERE id = ?1 AND cancelled = 0 AND delivered_at IS NULL",
            params![id],
        )?;
        Ok(affected > 0)
    }

    /// Return all due reminders (due_at <= now, not cancelled, not delivered).
    pub fn get_due_reminders(&self) -> Result<Vec<Reminder>> {
        let now = Utc::now().to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT id, message, due_at, channel, created_at, delivered_at, cancelled
             FROM reminders
             WHERE due_at <= ?1
               AND cancelled = 0
               AND delivered_at IS NULL
             ORDER BY due_at ASC",
        )?;

        let rows = stmt.query_map(params![now], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                message: row.get(1)?,
                due_at: row.get(2)?,
                channel: row.get(3)?,
                created_at: row.get(4)?,
                delivered_at: row.get(5)?,
                cancelled: row.get::<_, i64>(6)? != 0,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to query due reminders")
    }

    /// Mark a reminder as delivered with the current UTC time.
    pub fn mark_delivered(&self, id: i64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE reminders SET delivered_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }
}

// ── Scheduler ────────────────────────────────────────────────────────

/// Spawn the reminder polling background task.
///
/// Polls SQLite every 30 seconds. For each due reminder:
/// 1. Looks up the channel in the registry.
/// 2. Sends "Reminder: {message}" to that channel.
/// 3. Marks delivered on success; logs error and leaves undelivered on failure.
pub fn spawn_reminder_scheduler(
    reminder_store: Arc<std::sync::Mutex<ReminderStore>>,
    channels: HashMap<String, Arc<dyn Channel>>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);

        loop {
            interval.tick().await;

            // Fetch due reminders (sync, must not block)
            let due = {
                match reminder_store.lock() {
                    Ok(store) => match store.get_due_reminders() {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(error = %e, "reminder scheduler: failed to query due reminders");
                            continue;
                        }
                    },
                    Err(e) => {
                        error!(error = %e, "reminder scheduler: lock poisoned");
                        continue;
                    }
                }
            };

            if due.is_empty() {
                continue;
            }

            for reminder in due {
                let Some(channel) = channels.get(&reminder.channel) else {
                    warn!(
                        id = reminder.id,
                        channel = %reminder.channel,
                        "reminder due but channel not found — will retry next cycle"
                    );
                    continue;
                };

                let text = format!("Reminder: {}", reminder.message);
                let msg = OutboundMessage {
                    channel: reminder.channel.clone(),
                    content: text,
                    reply_to: None,
                    keyboard: None,
                };

                match channel.send_message(msg).await {
                    Ok(()) => {
                        info!(id = reminder.id, channel = %reminder.channel, "reminder delivered");

                        if let Ok(store) = reminder_store.lock() {
                            if let Err(e) = store.mark_delivered(reminder.id) {
                                warn!(id = reminder.id, error = %e, "failed to mark reminder delivered");
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            id = reminder.id,
                            channel = %reminder.channel,
                            error = %e,
                            "failed to deliver reminder — will retry next cycle"
                        );
                    }
                }
            }
        }
    });
}

// ── Tool Impl Helpers ─────────────────────────────────────────────────

/// Format a list of active reminders for display.
pub fn format_reminders_list(reminders: &[Reminder], timezone: &str) -> String {
    if reminders.is_empty() {
        return "No active reminders.".to_string();
    }

    let mut lines = vec![format!("Active reminders ({}):", reminders.len())];
    for r in reminders {
        let due_display = DateTime::parse_from_rfc3339(&r.due_at)
            .ok()
            .map(|dt| to_local_display(&dt.with_timezone(&Utc), timezone))
            .unwrap_or_else(|| r.due_at.clone());

        lines.push(format!("  [{}] {} — due {}", r.id, r.message, due_display));
    }
    lines.join("\n")
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tz() -> &'static str {
        "America/Chicago"
    }

    #[test]
    fn parse_short_durations() {
        let before = Utc::now();

        let t = parse_relative_time("2h", test_tz()).unwrap();
        let delta = t - before;
        assert!(delta >= ChronoDuration::hours(2));
        assert!(delta < ChronoDuration::hours(3));

        let t = parse_relative_time("30m", test_tz()).unwrap();
        let delta = t - before;
        assert!(delta >= ChronoDuration::minutes(30));
        assert!(delta < ChronoDuration::hours(1));

        let t = parse_relative_time("1d", test_tz()).unwrap();
        let delta = t - before;
        assert!(delta >= ChronoDuration::days(1));
        assert!(delta < ChronoDuration::days(2));
    }

    #[test]
    fn parse_long_form_durations() {
        let before = Utc::now();

        let t = parse_relative_time("2 hours", test_tz()).unwrap();
        assert!(t - before >= ChronoDuration::hours(2));

        let t = parse_relative_time("30 minutes", test_tz()).unwrap();
        assert!(t - before >= ChronoDuration::minutes(30));

        let t = parse_relative_time("1 day", test_tz()).unwrap();
        assert!(t - before >= ChronoDuration::days(1));
    }

    #[test]
    fn parse_tomorrow() {
        let t = parse_relative_time("tomorrow", test_tz()).unwrap();
        let delta = t - Utc::now();
        // Should be between ~12 and ~36 hours from now
        assert!(delta > ChronoDuration::hours(12));
        assert!(delta < ChronoDuration::hours(36));
    }

    #[test]
    fn parse_tomorrow_with_time() {
        let t = parse_relative_time("tomorrow 9am", test_tz()).unwrap();
        let delta = t - Utc::now();
        // Must be in the future
        assert!(delta > ChronoDuration::zero());
    }

    #[test]
    fn parse_next_monday() {
        let t = parse_relative_time("next Monday", test_tz()).unwrap();
        let delta = t - Utc::now();
        // Should be 1–7 days away
        assert!(delta >= ChronoDuration::hours(1));
        assert!(delta <= ChronoDuration::days(8));
    }

    #[test]
    fn parse_next_monday_with_time() {
        let t = parse_relative_time("next Monday 2pm", test_tz()).unwrap();
        let delta = t - Utc::now();
        assert!(delta > ChronoDuration::zero());
    }

    #[test]
    fn parse_iso_passthrough() {
        let t = parse_relative_time("2030-01-15T10:00:00Z", test_tz()).unwrap();
        assert_eq!(t.year(), 2030);
    }

    #[test]
    fn parse_invalid_returns_error() {
        assert!(parse_relative_time("garbage nonsense", test_tz()).is_err());
    }

    #[test]
    fn reminder_store_crud() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = ReminderStore::new(&db_path).unwrap();

        // Create
        let due = Utc::now() + ChronoDuration::hours(1);
        let id1 = store.create_reminder("check deploy", &due, "telegram").unwrap();
        let id2 = store.create_reminder("meeting prep", &due, "discord").unwrap();
        assert!(id2 > id1);

        // List active
        let active = store.list_active_reminders().unwrap();
        assert_eq!(active.len(), 2);

        // Cancel
        assert!(store.cancel_reminder(id1).unwrap());
        assert!(!store.cancel_reminder(id1).unwrap()); // already cancelled

        let active = store.list_active_reminders().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, id2);

        // Mark delivered
        store.mark_delivered(id2).unwrap();
        let active = store.list_active_reminders().unwrap();
        assert!(active.is_empty());
    }

    #[test]
    fn get_due_reminders_filters_by_time() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = ReminderStore::new(&db_path).unwrap();

        // Past (due)
        let past = Utc::now() - ChronoDuration::minutes(5);
        let id_past = store.create_reminder("past", &past, "telegram").unwrap();

        // Future (not due)
        let future = Utc::now() + ChronoDuration::hours(2);
        let _id_future = store.create_reminder("future", &future, "telegram").unwrap();

        let due = store.get_due_reminders().unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, id_past);
    }

    #[test]
    fn migrations_set_user_version() {
        // Run migrations against an in-memory database and verify that
        // rusqlite_migration set PRAGMA user_version = 1 (one migration version).
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        reminders_migrations().to_latest(&mut conn).unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1, "user_version should be 1 after v1 migration");

        // Verify the reminders table and its indexes exist.
        let table_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='reminders'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_exists, 1, "reminders table should exist after migration");

        for index in &["idx_reminders_due_at", "idx_reminders_active"] {
            let idx_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    rusqlite::params![index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(idx_exists, 1, "index '{index}' should exist after migration");
        }
    }
}
