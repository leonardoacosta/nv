# Tasks: add-sqlite-migrations

## Dependencies

None (foundational spec).

## Tasks

### Core Implementation (already complete)

- [x] [1.1] Add `rusqlite_migration = "1.2"` to workspace dependencies in `Cargo.toml` and `crates/nv-daemon/Cargo.toml` [owner:api-engineer]
- [x] [1.2] Create `messages_migrations()` function in `messages.rs` returning `Migrations<'static>` with v1 migration containing the full initial schema (messages, tool_usage, messages_fts, api_usage, budget_alert_sent tables with indexes and triggers) [owner:api-engineer]
- [x] [1.3] Wire migration runner into `MessageStore::init()` -- call `messages_migrations().to_latest(&mut conn)` after PRAGMA setup, before FTS backfill [owner:api-engineer]
- [x] [1.4] Apply same migration pattern to `tools/schedule.rs` ScheduleStore -- `schedule_migrations()` with v1 for schedules table, called in `ScheduleStore::new()` [owner:api-engineer]

### Tests

- [ ] [2.1] Unit test: fresh database migration -- create temp DB, run `MessageStore::init()`, verify all tables exist via `SELECT name FROM sqlite_master WHERE type='table'` [owner:api-engineer]
- [ ] [2.2] Unit test: idempotent migration -- run `MessageStore::init()` twice on same DB file, verify no error and schema unchanged [owner:api-engineer]
- [ ] [2.3] Unit test: PRAGMA user_version tracks migration count -- after `MessageStore::init()`, query `PRAGMA user_version` and assert it equals the number of migrations (currently 4) [owner:api-engineer]
- [ ] [2.4] Unit test: ScheduleStore migration -- create temp DB, run `ScheduleStore::new()`, verify schedules table exists [owner:api-engineer]

### Verify

- [ ] [3.1] `cargo build` passes for all workspace members [owner:api-engineer]
- [ ] [3.2] `cargo test -p nv-daemon` -- all new and existing tests pass [owner:api-engineer]
- [ ] [3.3] `cargo clippy -- -D warnings` passes [owner:api-engineer]
