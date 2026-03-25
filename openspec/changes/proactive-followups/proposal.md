# proactive-followups

## Summary

Add a `ProactiveWatcher` that runs on a configurable schedule (default: every 2 hours during 8am–10pm) and scans the obligation store for stale, overdue, and approaching-deadline obligations. For each match, Nova sends a gentle Telegram reminder with context and action buttons. Respects quiet hours. Thresholds are configurable in `nv.toml`.

## Motivation

Nova already detects and stores obligations from inbound messages. But detection alone does not close the loop — obligations sit open indefinitely unless the user happens to ask about them. The existing scheduler fires digests and morning briefings on fixed intervals, but neither targets individual obligations proactively. The result is that commitments made via Telegram or other channels silently age out without follow-up.

The `ProactiveWatcher` fills this gap: it is the autonomous follow-up mechanism that makes Nova act on what it knows rather than waiting to be asked.

## Current State

`obligation_store.rs` — `ObligationStore` with full CRUD: `list_by_status`, `list_all`, `count_open_by_priority`. The `obligations` table has `status`, `priority`, `created_at`, `updated_at`, `detected_action`, `source_channel`, `deadline` (not yet present — see Design).

`scheduler.rs` — `spawn_scheduler` manages three tokio intervals: digest (configurable), user schedule poll (60s), morning briefing (7am daily). All emit `Trigger::Cron(CronEvent::*)` into the shared `mpsc::UnboundedSender<Trigger>`.

`nv-core/src/types.rs` — `CronEvent` enum has `Digest`, `MemoryCleanup`, `MorningBriefing`, `UserSchedule`. No `ProactiveFollowup` variant yet.

`config/nv.toml` — `[agent]` section has `digest_interval_minutes`. No proactive watcher config block yet.

`watchers/mod.rs` and siblings — alert-rule watchers (deploy, sentry, stale ticket, HA) run via `spawn_watchers` on a separate poll cycle. They create obligations. The `ProactiveWatcher` is distinct: it reads existing obligations and sends reminders, it does not create new ones.

`channels/telegram/` — Telegram client sends `OutboundMessage` with optional `InlineKeyboard`. The `InlineKeyboard::confirm_action` and `InlineKeyboard::session_error` patterns show the established button API. The `telegram-reminder-ux` spec (Wave 4, bead `nv-myw8`) defines button patterns for reminder notifications — this spec reuses those patterns.

## Design

### 1. Schema: Add `deadline` Column to `obligations`

The proactive watcher needs a per-obligation deadline to detect "approaching deadline" cases. Add a `deadline TEXT` (nullable, RFC 3339) column to the `obligations` table via a new migration in `MessageStore`.

```
obligations
  + deadline TEXT  -- nullable, RFC 3339 UTC; NULL means no explicit deadline
```

The `Obligation` struct in `nv-core/src/types.rs` gains an `pub deadline: Option<String>` field. `ObligationStore::create` and `NewObligation` gain an optional `deadline` field. Existing callers pass `None`.

### 2. Config: `[proactive_watcher]` in `nv.toml`

```toml
[proactive_watcher]
enabled = true
# How often to scan obligations (minutes). Default: 120.
interval_minutes = 120
# Quiet hours: no reminders sent during this window (local time).
quiet_start = "22:00"
quiet_end = "08:00"
# Item is "overdue" when past its deadline.
# Item is "stale" when updated_at is older than this threshold (hours).
stale_threshold_hours = 48
# Item is "approaching deadline" when deadline is within this window (hours).
approaching_deadline_hours = 24
# Maximum reminders per obligation per interval (dedup guard).
max_reminders_per_interval = 1
```

`NvConfig` in `crates/nv-core/src/config.rs` (or wherever config is parsed) gains a `proactive_watcher: Option<ProactiveWatcherConfig>` field with the above defaults applied when absent.

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ProactiveWatcherConfig {
    #[serde(default = "default_pw_enabled")]
    pub enabled: bool,
    #[serde(default = "default_pw_interval")]
    pub interval_minutes: u64,
    #[serde(default = "default_pw_quiet_start")]
    pub quiet_start: String,      // "22:00"
    #[serde(default = "default_pw_quiet_end")]
    pub quiet_end: String,        // "08:00"
    #[serde(default = "default_pw_stale_hours")]
    pub stale_threshold_hours: u64,
    #[serde(default = "default_pw_approaching_hours")]
    pub approaching_deadline_hours: u64,
    #[serde(default = "default_pw_max_reminders")]
    pub max_reminders_per_interval: u32,
}
```

### 3. `CronEvent::ProactiveFollowup` Variant

Add to `nv-core/src/types.rs`:

```rust
pub enum CronEvent {
    Digest,
    MemoryCleanup,
    MorningBriefing,
    UserSchedule { name: String, action: String },
    ProactiveFollowup,   // <-- new
}
```

### 4. `ProactiveWatcher` Module — `crates/nv-daemon/src/proactive_watcher.rs`

New file. A single `spawn_proactive_watcher` function mirrors the pattern of `spawn_scheduler`.

```rust
pub fn spawn_proactive_watcher(
    trigger_tx: mpsc::UnboundedSender<Trigger>,
    config: ProactiveWatcherConfig,
    nv_base: &Path,
) -> tokio::task::JoinHandle<()>
```

The spawned task:

1. Reads `~/.nv/state/proactive-watcher.json` on startup to recover `last_run_at` and per-obligation reminder counts.
2. Calculates initial delay: if `last_run_at` is within `interval_minutes`, waits the remainder.
3. On each tick:
   a. Checks quiet hours (local time) — if currently in the quiet window, skips and logs.
   b. Pushes `Trigger::Cron(CronEvent::ProactiveFollowup)` to the trigger channel.
   c. Updates `last_run_at` in state.

The watcher itself does not query the DB — it emits the trigger and lets the orchestrator handle the scan. This preserves the existing pattern where the scheduler and orchestrator are decoupled.

### 5. State File: `~/.nv/state/proactive-watcher.json`

```json
{
  "last_run_at": "2026-03-25T10:00:00Z",
  "reminder_counts": {
    "obligation-uuid-1": 2,
    "obligation-uuid-2": 1
  }
}
```

`ProactiveWatcherState` struct with `last_run_at: Option<DateTime<Utc>>` and `reminder_counts: HashMap<String, u32>`. Read/write with atomic JSON write (same pattern as `DigestStateManager`).

`reminder_counts` is used to enforce `max_reminders_per_interval`: if an obligation has already received `>= max_reminders_per_interval` reminders since the last reset, it is skipped. Counts reset when `last_run_at` rolls over to a new day.

### 6. Orchestrator Handling of `CronEvent::ProactiveFollowup`

In `orchestrator.rs`, the existing `Trigger::Cron` arm routes to the digest pipeline. Add a new branch for `CronEvent::ProactiveFollowup`:

```rust
CronEvent::ProactiveFollowup => {
    self.handle_proactive_followup().await;
}
```

`handle_proactive_followup` (new private method on `Orchestrator`):

1. Locks `ObligationStore` and queries open obligations via `list_by_status(ObligationStatus::Open)`.
2. For each open obligation, applies the scan:
   - **Overdue**: `deadline` is set and `deadline < now`. Priority label: "overdue".
   - **Approaching deadline**: `deadline` is set and `now < deadline < now + approaching_deadline_hours`. Priority label: "due soon".
   - **Stale**: `updated_at < now - stale_threshold_hours` and no deadline set (or deadline is in the future). Priority label: "no update in Xh".
3. Deduplicates using `reminder_counts` from `ProactiveWatcherState`.
4. For each matched obligation (up to a cap of 5 per run to avoid flooding), sends a Telegram message with context and action buttons.
5. Increments `reminder_counts` for each reminded obligation and saves state.

Cap of 5 reminders per run prevents a flood if many obligations are stale simultaneously. Obligations are ordered by: overdue first, then approaching deadline, then stale. Within each group, ordered by priority ASC (0 = most urgent).

### 7. Telegram Reminder Format

Each reminder is a separate `OutboundMessage` to `config.telegram.chat_id`. Format (plain text, no markdown beyond what Telegram parses):

```
Follow-up: {detected_action}

Status: {overdue | due soon | no update in 48h}
Channel: {source_channel}
Priority: P{priority}

What would you like to do?
```

Inline keyboard (reusing `telegram-reminder-ux` button patterns):

```
[Mark Done]  [Snooze 24h]  [Dismiss]
```

Callback data:
- `followup:done:{obligation_id}` — calls `ObligationStore::update_status(id, Done)`
- `followup:snooze:{obligation_id}` — updates `updated_at` to now (resets staleness clock)
- `followup:dismiss:{obligation_id}` — calls `ObligationStore::update_status(id, Dismissed)`

### 8. Callback Routing

In the Telegram callback handler (currently in `orchestrator.rs` or `callbacks.rs`), add a `followup:` prefix route:

```rust
"followup" => handle_followup_callback(action, obligation_id, &self.deps).await,
```

`handle_followup_callback` executes the corresponding `ObligationStore` operation and sends a one-line confirmation to Telegram.

### 9. Quiet Hours Logic

Quiet hours use the local timezone from `config.agent.timezone` (defaulting to "UTC" if unset). The check compares the current local `NaiveTime` against `quiet_start` and `quiet_end`. The window wraps midnight (e.g. 22:00–08:00 means the quiet period spans two calendar days).

```rust
fn is_quiet_now(quiet_start: NaiveTime, quiet_end: NaiveTime) -> bool {
    let now = local_now(timezone).time();
    if quiet_start < quiet_end {
        // Window does not wrap midnight (e.g. 01:00–06:00)
        now >= quiet_start && now < quiet_end
    } else {
        // Window wraps midnight (e.g. 22:00–08:00)
        now >= quiet_start || now < quiet_end
    }
}
```

`tz_offset_seconds` from `reminders.rs` is reused for timezone lookup.

### 10. Daemon Wiring

In `main.rs`, after constructing `scheduler` and `obligation_store`:

```rust
if config.proactive_watcher.as_ref().map(|c| c.enabled).unwrap_or(true) {
    proactive_watcher::spawn_proactive_watcher(
        trigger_tx.clone(),
        config.proactive_watcher.clone().unwrap_or_default(),
        &nv_base,
    );
}
```

## Files Changed

| File | Change |
|------|--------|
| `crates/nv-core/src/types.rs` | Add `CronEvent::ProactiveFollowup` variant; add `deadline: Option<String>` to `Obligation` |
| `crates/nv-core/src/config.rs` | Add `ProactiveWatcherConfig` struct; add `proactive_watcher` field to `NvConfig` |
| `crates/nv-daemon/src/messages.rs` | Add migration v(N+1): `ALTER TABLE obligations ADD COLUMN deadline TEXT` |
| `crates/nv-daemon/src/obligation_store.rs` | Add `deadline` to `NewObligation`, `create`, `row_to_obligation`; add `snooze(id)` method (touches `updated_at`) |
| `crates/nv-daemon/src/proactive_watcher.rs` | New — `ProactiveWatcherState`, `spawn_proactive_watcher`, quiet-hours logic |
| `crates/nv-daemon/src/orchestrator.rs` | Add `CronEvent::ProactiveFollowup` arm; add `handle_proactive_followup` method; add `followup:` callback routing |
| `crates/nv-daemon/src/lib.rs` | `pub mod proactive_watcher;` |
| `config/nv.toml` | Add `[proactive_watcher]` example block (commented, showing defaults) |

## Dependencies

- `telegram-reminder-ux` (Wave 4, bead `nv-myw8`) — button patterns for `[Mark Done] [Snooze 24h] [Dismiss]`. This spec's callback format mirrors the reminder UX established there. Can be implemented before `telegram-reminder-ux` is applied if the button API is treated as a local convention in `proactive_watcher.rs`; the two specs should be applied in order for consistency.
- `callback-handler-completion` (Wave 4, bead `nv-ags8`) — full callback routing infrastructure. The `followup:` prefix routing defined here depends on the callback handler being able to route arbitrary prefixes.

## Out of Scope

- Natural-language deadline extraction from message content (the `deadline` field is set manually or by future NLP tools — not extracted in this spec)
- Push notifications to channels other than Telegram
- Per-obligation snooze duration configuration (fixed at 24h for v1)
- Obligation history / audit log for reminder events
- Dashboard UI for proactive watcher status
- Adaptive interval (backing off when no matches found)
