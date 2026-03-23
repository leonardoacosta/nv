# Implementation Tasks

<!-- beads:epic:TBD -->

## SQLite Reminders Table + Store

- [x] [1.1] [P-1] Create `crates/nv-daemon/src/reminders.rs` — define `Reminder` struct (id, message, due_at, channel, created_at, delivered_at, cancelled), implement `ReminderStore::new(db_path)` that opens a rusqlite connection and runs `CREATE TABLE IF NOT EXISTS reminders` + indexes from Req-1 [owner:api-engineer]
- [x] [1.2] [P-1] Implement `ReminderStore::create_reminder(message, due_at, channel) -> Result<i64>` — insert row with `created_at` set to current UTC, return autoincrement ID [owner:api-engineer]
- [x] [1.3] [P-1] Implement `ReminderStore::list_active_reminders() -> Result<Vec<Reminder>>` — select where `cancelled = 0 AND delivered_at IS NULL`, order by `due_at ASC` [owner:api-engineer]
- [x] [1.4] [P-1] Implement `ReminderStore::cancel_reminder(id) -> Result<bool>` — set `cancelled = 1`, return whether row existed [owner:api-engineer]
- [x] [1.5] [P-1] Implement `ReminderStore::get_due_reminders() -> Result<Vec<Reminder>>` — select where `due_at <= now AND cancelled = 0 AND delivered_at IS NULL` [owner:api-engineer]
- [x] [1.6] [P-1] Implement `ReminderStore::mark_delivered(id) -> Result<()>` — set `delivered_at` to current UTC [owner:api-engineer]

## Relative Time Parsing

- [x] [2.1] [P-1] Add `parse_relative_time(input: &str, timezone: &str) -> Result<DateTime<Utc>>` in `crates/nv-daemon/src/reminders.rs` — handle short-form durations (`2h`, `30m`, `1d`), long-form (`2 hours`, `30 minutes`, `1 day`), and ISO 8601 passthrough using chrono [owner:api-engineer]
- [x] [2.2] [P-1] Add natural date parsing — `tomorrow`, `tomorrow 9am`, `next Monday`, `next Monday 2pm` — resolve against user timezone from config (`daemon.timezone`, default `America/Chicago`), convert to UTC for storage [owner:api-engineer]
- [x] [2.3] [P-2] Add `timezone` field to daemon config in `crates/nv-core/src/config.rs` — optional string, default `"America/Chicago"` [owner:api-engineer]

## Tool Registration + Dispatch

- [x] [3.1] [P-1] Register `set_reminder`, `list_reminders`, `cancel_reminder` tool definitions in `register_tools()` in `crates/nv-daemon/src/tools.rs` — schemas match Req-3 JSON exactly [owner:api-engineer]
- [x] [3.2] [P-1] Add `set_reminder` dispatch in `execute_tool()` / `execute_tool_send()` — parse `due_at` via `parse_relative_time`, call `ReminderStore::create_reminder`, return confirmation with reminder ID and resolved due time [owner:api-engineer]
- [x] [3.3] [P-1] Add `list_reminders` dispatch — call `list_active_reminders()`, format as readable list with IDs, messages, and due times (converted to user timezone for display) [owner:api-engineer]
- [x] [3.4] [P-1] Add `cancel_reminder` dispatch — call `cancel_reminder(id)`, return success/not-found message [owner:api-engineer]

## Reminder Scheduler (Background Task)

- [x] [4.1] [P-1] Implement `spawn_reminder_scheduler()` in `crates/nv-daemon/src/reminders.rs` — tokio task that polls `get_due_reminders()` every 30s, sends `"Reminder: {message}"` to the reminder's channel via channel registry, calls `mark_delivered(id)` on success [owner:api-engineer]
- [x] [4.2] [P-1] Handle delivery failures — log error via tracing, leave reminder undelivered so next poll cycle retries; log warning if channel is unavailable [owner:api-engineer]

## Wiring (main.rs + worker.rs)

- [x] [5.1] [P-1] Add `mod reminders;` to `crates/nv-daemon/src/main.rs` [owner:api-engineer]
- [x] [5.2] [P-1] Init `ReminderStore` in `main.rs` reusing the messages.db path, add `reminder_store: Option<Arc<Mutex<ReminderStore>>>` field to `SharedDeps` in `crates/nv-daemon/src/worker.rs` [owner:api-engineer]
- [x] [5.3] [P-1] Spawn reminder scheduler task in `main.rs` alongside existing digest scheduler — pass channel registry clone and reminder store reference [owner:api-engineer]

## Tests

- [x] [6.1] [P-2] Unit tests for `parse_relative_time` — short durations (`2h`, `30m`, `1d`), long-form (`2 hours`), natural dates (`tomorrow`, `tomorrow 9am`, `next Monday`, `next Monday 2pm`), ISO 8601 passthrough, invalid input returns error [owner:test-writer]
- [x] [6.2] [P-2] Unit tests for `ReminderStore` CRUD — create returns incrementing IDs, list returns only active reminders, cancel sets flag and returns true (false for nonexistent), get_due filters by time correctly, mark_delivered sets timestamp [owner:test-writer]
- [x] [6.3] [P-2] Unit test for scheduler logic — due reminders are picked up and marked delivered, undeliverable reminders remain active for retry [owner:test-writer]

## Verify

- [x] [7.1] `cargo build` passes [owner:api-engineer]
- [x] [7.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [7.3] `cargo test` — existing tests pass, all new reminder tests pass [owner:api-engineer]
