# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] Update `temp_db()` in `deploy_watcher.rs` test module -- open a raw `rusqlite::Connection` on the temp file and execute `CREATE TABLE IF NOT EXISTS` for both `alert_rules` and `obligations` tables before constructing `AlertRuleStore` and `ObligationStore`, matching the schema used in `watchers/mod.rs::temp_stores()` [owner:api-engineer]
- [ ] [2.2] [P-2] Fix the misleading comment above `temp_db()` (lines 188-191) that incorrectly claims `ObligationStore` applies v1+v2 migrations -- update to accurately describe that tables are created manually in the helper [owner:api-engineer]

## Verify

- [ ] [3.1] Test `watcher_cycle_stores_obligation_for_deploy_failure_rule` passes (`cargo test -p nv-daemon watcher_cycle_stores_obligation`) [owner:api-engineer]
- [ ] [3.2] Test `watcher_cycle_no_obligation_when_all_ready` still passes [owner:api-engineer]
- [ ] [3.3] `cargo build` passes [owner:api-engineer]
- [ ] [3.4] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [3.5] All existing tests pass (`cargo test -p nv-daemon`) [owner:api-engineer]
