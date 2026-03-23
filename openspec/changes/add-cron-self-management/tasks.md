# Implementation Tasks

<!-- beads:epic:TBD -->

## Core Types

- [x] [1.1] [P-1] Add `ActionType::{ScheduleAdd, ScheduleModify, ScheduleRemove}` variants to `crates/nv-core/src/types.rs` [owner:api-engineer]
- [x] [1.2] [P-1] Add `CronEvent::UserSchedule { name: String, action: String }` variant to `crates/nv-core/src/types.rs` [owner:api-engineer]

## ScheduleStore (SQLite)

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/schedule_tools.rs` — `Schedule` struct (id, name, cron_expr, action, channel, enabled, created_at, last_run_at), derive Debug/Clone/Serialize/Deserialize [owner:api-engineer]
- [x] [2.2] [P-1] Implement `ScheduleStore::new(nv_base: &Path)` — open `schedules.db`, run CREATE TABLE IF NOT EXISTS, return Self [owner:api-engineer]
- [x] [2.3] [P-1] Implement `ScheduleStore::{list, get, insert, update_cron, set_enabled, delete, mark_run}` — rusqlite CRUD matching the schema in proposal.md [owner:api-engineer]
- [x] [2.4] [P-1] Add `cron` crate dependency to `crates/nv-daemon/Cargo.toml` [owner:api-engineer]
- [x] [2.5] [P-1] Implement `validate_cron_expr(expr: &str) -> Result<cron::Schedule>` helper — convert 5-field user input to 7-field cron crate format (prepend `0`, append `*`), parse, return error on invalid [owner:api-engineer]
- [x] [2.6] [P-1] Implement `next_fire_time(cron_expr: &str) -> Result<Option<DateTime<Utc>>>` helper — parse expression and return next upcoming time [owner:api-engineer]
- [x] [2.7] [P-2] Add `mod schedule_tools;` to `main.rs` module declarations [owner:api-engineer]

## Tool Definitions

- [x] [3.1] [P-1] Add `list_schedules` tool definition to `register_tools()` in `tools.rs` — no parameters, description: "List all recurring schedules (built-in and user-created) with next fire time" [owner:api-engineer]
- [x] [3.2] [P-1] Add `add_schedule` tool definition to `register_tools()` — params: name (string), cron_expr (string), action (string, enum: digest/health_check/reminder), channel (string); all required [owner:api-engineer]
- [x] [3.3] [P-1] Add `modify_schedule` tool definition to `register_tools()` — params: name (string, required), cron_expr (string, optional), enabled (boolean, optional) [owner:api-engineer]
- [x] [3.4] [P-1] Add `remove_schedule` tool definition to `register_tools()` — params: name (string, required) [owner:api-engineer]

## Tool Dispatch

- [x] [4.1] [P-1] Add `list_schedules` match arm in `execute_schedule_tool()` — load from ScheduleStore, prepend hardcoded built-in entries, format with next fire times, return `ToolResult::Immediate` [owner:api-engineer]
- [x] [4.2] [P-1] Add `add_schedule` match arm — validate cron_expr, reject reserved/duplicate names, return `ToolResult::PendingAction` with `ActionType::ScheduleAdd` [owner:api-engineer]
- [x] [4.3] [P-1] Add `modify_schedule` match arm — validate name exists, reject built-in names, validate cron_expr if provided, return `ToolResult::PendingAction` with `ActionType::ScheduleModify` [owner:api-engineer]
- [x] [4.4] [P-1] Add `remove_schedule` match arm — validate name exists, reject built-in names, return `ToolResult::PendingAction` with `ActionType::ScheduleRemove` [owner:api-engineer]
- [x] [4.5] [P-1] Implemented as separate `execute_schedule_tool()` sync function; dispatch in worker.rs via `else if` branch that locks mutex, calls sync function, drops guard before any `.await` [owner:api-engineer]

## Callback Execution

- [x] [5.1] [P-1] Add `ScheduleAdd` arm in `callbacks.rs` `handle_approve()` — extract name/cron_expr/action/channel from payload, call `ScheduleStore::insert()` [owner:api-engineer]
- [x] [5.2] [P-1] Add `ScheduleModify` arm — extract name + optional cron_expr/enabled from payload, call appropriate ScheduleStore methods [owner:api-engineer]
- [x] [5.3] [P-1] Add `ScheduleRemove` arm — extract name from payload, call `ScheduleStore::delete()` [owner:api-engineer]

## Scheduler Integration

- [x] [6.1] [P-1] Extend `spawn_scheduler()` to accept `Arc<Mutex<ScheduleStore>>` parameter [owner:api-engineer]
- [x] [6.2] [P-1] Add user-schedule polling loop (60s interval) inside `spawn_scheduler()` — load enabled schedules, check `last_run_at` vs cron next-fire, emit `Trigger::Cron(CronEvent::UserSchedule { name, action })`, call `mark_run()` [owner:api-engineer]
- [x] [6.3] [P-1] `CronEvent::UserSchedule` handled in worker format_trigger_batch via agent.rs format functions; action text dispatched to appropriate channel [owner:api-engineer]

## Wiring (main.rs + SharedDeps)

- [x] [7.1] [P-1] Add `schedule_store: Option<Arc<std::sync::Mutex<ScheduleStore>>>` field to `SharedDeps` in `worker.rs` [owner:api-engineer]
- [x] [7.2] [P-1] In `main.rs`: construct `ScheduleStore::new(&nv_base)`, wrap in `Arc<Mutex<>>`, add to `SharedDeps` construction, pass clone to `spawn_scheduler()` [owner:api-engineer]

## Verify

- [x] [8.1] `cargo build` passes [owner:api-engineer]
- [x] [8.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [8.3] `cargo test` — existing tests pass, new unit tests for ScheduleStore CRUD and cron validation helpers [owner:api-engineer]
