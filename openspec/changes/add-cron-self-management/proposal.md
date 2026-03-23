# Proposal: Cron Self-Management

## Change ID
`add-cron-self-management`

## Summary

Four new tools (`list_schedules`, `add_schedule`, `modify_schedule`, `remove_schedule`) that let
the user view and manage Nova's recurring schedules through natural language. Built-in schedules
(digest, memory cleanup) are read-only; user-created schedules are persisted in SQLite and
integrated into the existing scheduler loop. Write operations require PendingAction confirmation
via Telegram inline keyboard.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool registration + dispatch), `crates/nv-daemon/src/scheduler.rs` (cron loop), `crates/nv-daemon/src/main.rs` (SharedDeps, init), `crates/nv-core/src/types.rs` (CronEvent, ActionType)
- Related: Existing `scheduler.rs` runs a single fixed-interval digest loop. `MessageStore` in `messages.rs` demonstrates the SQLite init + query patterns (rusqlite, `Connection::open`, `CREATE TABLE IF NOT EXISTS`). `callbacks.rs` shows the PendingAction approve/reject flow.
- Depends on: `cron` crate (new dependency) for parsing and next-fire-time calculation

## Motivation

Nova's scheduler currently supports only built-in periodic tasks (digest, memory cleanup) with
hardcoded intervals. The user cannot create, inspect, or modify recurring schedules. Common
requests like "run a health check every morning at 8am" or "send me a digest every 6 hours instead
of 4" require code changes. Exposing schedule CRUD through tools lets the user manage recurrence
through natural conversation while keeping built-in schedules immutable and safe.

## Requirements

### Req-1: SQLite Schedules Table

New table in a dedicated `~/.nv/schedules.db` database, managed by a `ScheduleStore` struct
following the same pattern as `MessageStore`:

```sql
CREATE TABLE IF NOT EXISTS schedules (
    id          TEXT PRIMARY KEY,        -- UUID
    name        TEXT NOT NULL UNIQUE,    -- user-facing label, e.g. "morning-health-check"
    cron_expr   TEXT NOT NULL,           -- standard 5-field cron expression
    action      TEXT NOT NULL,           -- one of: digest, health_check, reminder
    channel     TEXT NOT NULL,           -- originating channel: "telegram", "discord", etc.
    enabled     INTEGER NOT NULL DEFAULT 1,  -- 0 = paused, 1 = active
    created_at  TEXT NOT NULL,           -- ISO 8601
    last_run_at TEXT                     -- ISO 8601, NULL if never run
);
```

`ScheduleStore` exposes:
- `new(nv_base: &Path) -> Self` — opens/creates DB, runs CREATE TABLE
- `list() -> Result<Vec<Schedule>>` — all rows
- `get(name: &str) -> Result<Option<Schedule>>` — by name
- `insert(schedule: &Schedule) -> Result<()>` — unique name enforced
- `update_cron(name: &str, cron_expr: &str) -> Result<()>`
- `set_enabled(name: &str, enabled: bool) -> Result<()>`
- `delete(name: &str) -> Result<bool>` — returns whether a row was deleted
- `mark_run(name: &str) -> Result<()>` — sets `last_run_at` to now

The `Schedule` struct mirrors the table columns and derives `Debug, Clone, Serialize, Deserialize`.

### Req-2: `list_schedules` Tool (Read-Only)

Shows all schedules: built-in (hardcoded in the formatter, not in SQLite) plus user-created
(from SQLite). For each schedule, display: name, cron expression (human-readable description),
enabled status, and next fire time.

Built-in schedules to list:
- `digest` — interval from config (`config.agent.digest_interval_minutes`), always enabled
- `memory-cleanup` — if configured, always enabled

Next fire time is calculated from the cron expression using the `cron` crate's `Schedule::upcoming()`.

Returns `ToolResult::Immediate` with formatted text.

Parameters: none.

### Req-3: `add_schedule` Tool (PendingAction)

Creates a new user schedule. Returns `ToolResult::PendingAction` so the user confirms via Telegram
before the row is inserted.

Parameters:
- `name` (string, required) — unique label, validated as lowercase alphanumeric + hyphens
- `cron_expr` (string, required) — standard 5-field cron, validated by the `cron` crate parser
- `action` (string, required) — one of `digest`, `health_check`, `reminder`
- `channel` (string, required) — originating channel name

Validation before returning PendingAction:
- Parse `cron_expr` with `cron::Schedule::from_str()` — reject invalid expressions
- Reject names already in use (check SQLite)
- Reject reserved names: `digest`, `memory-cleanup`

On approval (in `callbacks.rs`): insert the row via `ScheduleStore::insert()`.

New `ActionType::ScheduleAdd` variant needed in `nv_core::types`.

### Req-4: `modify_schedule` Tool (PendingAction)

Updates an existing user schedule. Returns `ToolResult::PendingAction`.

Parameters:
- `name` (string, required) — must exist in SQLite
- `cron_expr` (string, optional) — new cron expression, validated
- `enabled` (boolean, optional) — pause/resume

At least one of `cron_expr` or `enabled` must be provided. Reject modifications to built-in
schedule names.

On approval: call `ScheduleStore::update_cron()` and/or `ScheduleStore::set_enabled()`.

New `ActionType::ScheduleModify` variant.

### Req-5: `remove_schedule` Tool (PendingAction)

Deletes a user schedule by name. Returns `ToolResult::PendingAction`.

Parameters:
- `name` (string, required) — must exist in SQLite, must not be a built-in name

On approval: call `ScheduleStore::delete()`.

New `ActionType::ScheduleRemove` variant.

### Req-6: Scheduler Integration

Extend `scheduler.rs` to check user schedules alongside the built-in digest loop:

1. Add a second polling loop (or merge into the existing one) that runs every 60 seconds
2. On each tick: load all enabled user schedules from SQLite
3. For each schedule: compare `last_run_at` with `cron::Schedule::after()` — if a firing was
   missed, emit `Trigger::Cron(CronEvent::UserSchedule { name, action })` into the trigger channel
4. After emitting, call `ScheduleStore::mark_run()`

Add `CronEvent::UserSchedule { name: String, action: String }` variant to `nv_core::types`.

The orchestrator/worker already handles `Trigger::Cron` — it needs a new match arm for
`UserSchedule` that formats an appropriate prompt based on the action type (digest, health_check,
reminder).

### Req-7: SharedDeps + Init Wiring

- Add `schedule_store: Arc<std::sync::Mutex<ScheduleStore>>` to `SharedDeps`
- In `main.rs`: construct `ScheduleStore::new(&nv_base)`, wrap in `Arc<Mutex<>>`, add to
  `SharedDeps` construction
- Pass `Arc<Mutex<ScheduleStore>>` to `spawn_scheduler()` (signature change)
- Pass `schedule_store` ref to `execute_tool_send()` (signature change) for tool dispatch

## Scope
- **IN**: SQLite CRUD for user schedules, 4 tool definitions + dispatch, PendingAction flow for writes, scheduler polling of user schedules, `cron` crate integration, 3 new ActionType variants, CronEvent::UserSchedule
- **OUT**: Cron expression builder UI, natural language to cron parsing (Claude handles this), modifying built-in schedule intervals via tools, per-schedule timezone support (uses system TZ), web dashboard

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/schedule_tools.rs` | New file: `ScheduleStore`, `Schedule` struct, SQLite CRUD, cron parsing helpers |
| `crates/nv-daemon/src/tools.rs` | 4 new tool definitions in `register_tools()`, 4 new match arms in `execute_tool_send()` |
| `crates/nv-daemon/src/scheduler.rs` | Add user-schedule polling loop, accept `ScheduleStore` param |
| `crates/nv-daemon/src/main.rs` | Init `ScheduleStore`, add to `SharedDeps`, pass to scheduler |
| `crates/nv-daemon/src/worker.rs` | Add `schedule_store` field to `SharedDeps` |
| `crates/nv-daemon/src/callbacks.rs` | Handle `ScheduleAdd`, `ScheduleModify`, `ScheduleRemove` approval execution |
| `crates/nv-core/src/types.rs` | Add `ActionType::{ScheduleAdd, ScheduleModify, ScheduleRemove}`, `CronEvent::UserSchedule` |
| `crates/nv-daemon/Cargo.toml` | Add `cron` dependency |

## Risks
| Risk | Mitigation |
|------|-----------|
| `cron` crate API mismatch with standard 5-field format | The `cron` crate uses 7-field expressions (sec min hr dom mon dow yr) — wrap with a helper that prepends `0` and appends `*` to convert 5-field user input to 7-field. Validate on input. |
| User schedule polling adds SQLite reads every 60s | Schedules table is tiny (single-digit rows expected). `SELECT` on a small table is negligible. No index needed beyond the `name` UNIQUE constraint. |
| Race between scheduler polling and tool CRUD | `ScheduleStore` is behind `Arc<Mutex<>>` — lock contention is minimal since operations are fast. The scheduler holds the lock only for the duration of the SELECT + UPDATE. |
| PendingAction approval delay causes missed fires | The scheduler checks `last_run_at` against cron, so it will catch up on the next poll after the row is inserted. At most one fire is delayed by the approval latency. |
| Name collision between user and built-in schedules | Reject reserved names (`digest`, `memory-cleanup`) at validation time in `add_schedule`. |
