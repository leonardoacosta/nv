# Implementation Tasks

## Phase 1: Schema and Core Types

- [x] [1.1] Add `CronEvent::ProactiveFollowup` variant to `crates/nv-core/src/types.rs` — extend the existing `CronEvent` enum; update any exhaustive match sites in `orchestrator.rs` and `scheduler.rs` with a no-op or `unreachable!` arm as appropriate [owner:api-engineer]
- [x] [1.2] Add `deadline: Option<String>` field to `Obligation` struct in `crates/nv-core/src/types.rs`; add `deadline: Option<String>` to `NewObligation` in `crates/nv-daemon/src/obligation_store.rs`; update `create`, `row_to_obligation`, all existing `NewObligation` construction sites to pass `deadline: None` (depends: none) [owner:api-engineer]
- [x] [1.3] Add migration in `crates/nv-daemon/src/messages.rs` (version N+1 in `rusqlite_migration` chain): `ALTER TABLE obligations ADD COLUMN deadline TEXT` — ensure the migration is appended after the current last migration, not inserted (depends: 1.2) [owner:db-engineer]

## Phase 2: Config

- [x] [2.1] Add `ProactiveWatcherConfig` struct to `crates/nv-core/src/config.rs` (or wherever `NvConfig` is defined) with fields: `enabled: bool` (default true), `interval_minutes: u64` (default 120), `quiet_start: String` (default "22:00"), `quiet_end: String` (default "08:00"), `stale_threshold_hours: u64` (default 48), `approaching_deadline_hours: u64` (default 24), `max_reminders_per_interval: u32` (default 1). Implement `Default`. Add `proactive_watcher: Option<ProactiveWatcherConfig>` field to `NvConfig`. Add unit tests for config deserialization with and without the section present (depends: none) [owner:api-engineer]
- [x] [2.2] Add `[proactive_watcher]` commented example block to `config/nv.toml` showing all fields with their defaults and a one-line comment per field (depends: 2.1) [owner:api-engineer]

## Phase 3: ObligationStore Extensions

- [x] [3.1] Add `snooze(id: &str) -> Result<bool>` method to `ObligationStore` in `crates/nv-daemon/src/obligation_store.rs` — executes `UPDATE obligations SET updated_at = datetime('now') WHERE id = ?1 AND status = 'open'`, returns true if a row was updated. Add unit test: create obligation, call snooze, verify `updated_at` is refreshed (depends: 1.3) [owner:db-engineer]
- [x] [3.2] Add `list_open_with_deadline_before(cutoff: &DateTime<Utc>) -> Result<Vec<Obligation>>` query to `ObligationStore` — returns open obligations where `deadline IS NOT NULL AND deadline <= cutoff`, ordered by priority ASC, deadline ASC. Add unit test (depends: 1.3) [owner:db-engineer]
- [x] [3.3] Add `list_stale_open(since: &DateTime<Utc>) -> Result<Vec<Obligation>>` query — returns open obligations where `updated_at <= since` (no deadline filter), ordered by priority ASC, updated_at ASC. Add unit test (depends: 1.3) [owner:db-engineer]

## Phase 4: ProactiveWatcher Module

- [x] [4.1] Create `crates/nv-daemon/src/proactive_watcher.rs` — define `ProactiveWatcherState` struct (`last_run_at: Option<DateTime<Utc>>`, `reminder_counts: HashMap<String, u32>`) with `load(nv_base: &Path) -> Result<Self>` and `save(&self, nv_base: &Path) -> Result<()>` using atomic JSON write (pattern from `DigestStateManager`). State file: `~/.nv/state/proactive-watcher.json` (depends: 2.1) [owner:api-engineer]
- [x] [4.2] Implement `is_quiet_now(quiet_start: NaiveTime, quiet_end: NaiveTime, timezone: &str) -> bool` in `proactive_watcher.rs` — handles midnight-spanning windows using the `tz_offset_seconds` helper from `reminders.rs`. Add unit tests for: non-wrapping window (01:00–06:00), wrapping window (22:00–08:00), boundary conditions (exactly at quiet_start, exactly at quiet_end), UTC case (depends: 4.1) [owner:api-engineer]
- [x] [4.3] Implement `spawn_proactive_watcher(trigger_tx: mpsc::UnboundedSender<Trigger>, config: ProactiveWatcherConfig, nv_base: &Path) -> tokio::task::JoinHandle<()>` — tokio task that loads state on startup, calculates initial delay from `last_run_at`, ticks at `interval_minutes`, checks quiet hours on each tick, pushes `Trigger::Cron(CronEvent::ProactiveFollowup)` when not quiet, updates `last_run_at` in state after each push, shuts down cleanly when channel closes. Minimum interval floor: 30 minutes (prevent runaway). Add unit tests: initial delay respects recent last_run_at; channel close stops the task; quiet hours suppresses trigger push (depends: 4.2, 1.1) [owner:api-engineer]
- [x] [4.4] Add `pub mod proactive_watcher;` to `crates/nv-daemon/src/lib.rs` (depends: 4.3) [owner:api-engineer]

## Phase 5: Orchestrator Integration

- [x] [5.1] Add `CronEvent::ProactiveFollowup` match arm to the `Trigger::Cron` handler in `crates/nv-daemon/src/orchestrator.rs` — routes to new `handle_proactive_followup(&mut self)` async method. Ensure existing `Digest`, `MorningBriefing`, `UserSchedule`, `MemoryCleanup` arms are unaffected (depends: 4.3, 3.1, 3.2, 3.3) [owner:api-engineer]
- [x] [5.2] Implement `handle_proactive_followup` on `Orchestrator` — loads `ProactiveWatcherState`; queries open obligations and applies three scans in order: (1) overdue (`deadline IS NOT NULL AND deadline < now`), (2) approaching deadline (`deadline IS NOT NULL AND deadline BETWEEN now AND now + approaching_deadline_hours`), (3) stale (`updated_at < now - stale_threshold_hours`); deduplicates via `reminder_counts`; caps at 5 reminders per run; sends one `OutboundMessage` per matched obligation with content and inline keyboard; increments `reminder_counts` and saves state (depends: 5.1) [owner:api-engineer]
- [x] [5.3] Implement Telegram reminder message formatting in `handle_proactive_followup` — plain text body: detected action, status label (overdue/due soon/no update in Xh), source channel, priority; inline keyboard with three buttons: `[Mark Done]` (`followup:done:{id}`), `[Snooze 24h]` (`followup:snooze:{id}`), `[Dismiss]` (`followup:dismiss:{id}`) (depends: 5.2) [owner:api-engineer]

## Phase 6: Callback Routing

- [x] [6.1] Add `followup:` prefix routing in the Telegram callback handler (in `orchestrator.rs` or `callbacks.rs`) — parse `followup:{action}:{obligation_id}` and route to `handle_followup_callback(action, id)` async fn. Actions: `done` → `ObligationStore::update_status(id, Done)`, `snooze` → `ObligationStore::snooze(id)`, `dismiss` → `ObligationStore::update_status(id, Dismissed)`. After each, send a one-line confirmation to the originating chat. Log unknown actions as warnings (depends: 5.3, 3.1) [owner:api-engineer]
- [x] [6.2] Add unit tests for `handle_followup_callback` — done/snooze/dismiss branches; unknown action logs warning and does not crash (depends: 6.1) [owner:api-engineer]

## Phase 7: Daemon Wiring

- [x] [7.1] In `crates/nv-daemon/src/main.rs`, after the scheduler spawn: conditionally spawn `proactive_watcher::spawn_proactive_watcher(trigger_tx.clone(), config.proactive_watcher.clone().unwrap_or_default(), &nv_base)` gated on `config.proactive_watcher.as_ref().map(|c| c.enabled).unwrap_or(true)`. Log a startup info line when the watcher starts (depends: 4.3, 2.1) [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Schema/Types | `cargo build -p nv-core -p nv-daemon` — new enum variant and `deadline` field compile cleanly; migration appended without altering existing migration order |
| 2 Config | `cargo test -p nv-core` — `ProactiveWatcherConfig` deserializes with all defaults and with explicit values; `NvConfig` still deserializes existing `nv.toml` without the new section |
| 3 ObligationStore | `cargo test -p nv-daemon` — `snooze`, `list_open_with_deadline_before`, `list_stale_open` unit tests pass |
| 4 Watcher | `cargo test -p nv-daemon` — `is_quiet_now` unit tests pass for wrapping/non-wrapping windows; `spawn_proactive_watcher` stops on channel close; initial delay respects recent `last_run_at` |
| 5 Orchestrator | `cargo build -p nv-daemon` — `handle_proactive_followup` compiles; no regressions in existing `Cron` arm handling |
| 6 Callbacks | `cargo test -p nv-daemon` — `handle_followup_callback` tests pass for all three actions |
| 7 Integration | Manual: restart daemon; wait one interval or send `Trigger::Cron(CronEvent::ProactiveFollowup)` via CLI; verify Telegram reminder arrives for a known stale obligation with correct buttons; tap each button and verify correct state transition |
