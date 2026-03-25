# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] In `ReminderStore::new()`, wrap the `to_latest()` call result and match on `DatabaseTooFarAhead` — check error `Display` string for that variant name [owner:api-engineer]
- [ ] [2.2] [P-1] On `DatabaseTooFarAhead` match: drop the existing `conn`, call `std::fs::remove_file(db_path)` ignoring `NotFound`, reopen the connection, re-run `reminders_migrations().to_latest()`, emit `warn!("reminders.db was ahead of migrations — recreated from scratch")` [owner:api-engineer]
- [ ] [2.3] [P-1] Propagate any non-`DatabaseTooFarAhead` migration error unchanged (existing behavior) [owner:api-engineer]

## Verify

- [ ] [3.1] `cargo build` passes [owner:api-engineer]
- [ ] [3.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [3.3] Unit test: open a DB, manually set `PRAGMA user_version = 99`, call `ReminderStore::new()` — expect success and a fresh store (user_version reset to 1) [owner:api-engineer]
- [ ] [3.4] Unit test: `ReminderStore::new()` on a fresh path still succeeds (regression guard) [owner:api-engineer]
- [ ] [3.5] Existing tests pass [owner:api-engineer]
