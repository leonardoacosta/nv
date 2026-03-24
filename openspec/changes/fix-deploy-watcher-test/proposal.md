# Proposal: Fix Deploy Watcher Test Setup

## Change ID
`fix-deploy-watcher-test`

## Summary

Fix the `watcher_cycle_stores_obligation_for_deploy_failure_rule` test in `deploy_watcher.rs` which
panics with "no such table: obligations". The test helper `temp_db()` opens `ObligationStore` and
`AlertRuleStore` on a fresh temp file but never creates the required SQLite tables.

## Context
- Extends: `crates/nv-daemon/src/watchers/deploy_watcher.rs` (test module)
- Related: `crates/nv-daemon/src/watchers/mod.rs` (has a correct `temp_stores()` helper)
- Related: `crates/nv-daemon/src/obligation_store.rs` (has a correct `temp_store()` helper)

## Problem

The test `watcher_cycle_stores_obligation_for_deploy_failure_rule` panics at line 380:

```
obligation_store.lock().unwrap().count_open().unwrap()
// -> rusqlite error: "no such table: obligations"
```

Root cause: the `temp_db()` helper (line 192) creates stores against a fresh `NamedTempFile`:

```rust
fn temp_db() -> (AlertRuleStore, ObligationStore, NamedTempFile) {
    let file = NamedTempFile::new().expect("temp db file");
    let obligations = ObligationStore::new(file.path()).expect("ObligationStore init");
    let rules = AlertRuleStore::new(file.path()).expect("AlertRuleStore init");
    (rules, obligations, file)
}
```

Neither `ObligationStore::new()` nor `AlertRuleStore::new()` creates tables -- they only open the
connection and set `PRAGMA journal_mode=WAL`. The comment on lines 188-191 incorrectly claims that
`ObligationStore` applies v1+v2 migrations, but that migration logic lives in `MessageStore::init`,
not in either store's constructor.

Two other test modules solve this correctly:

- `watchers/mod.rs::temp_stores()` (line 243) -- manually runs `CREATE TABLE IF NOT EXISTS` for
  both `alert_rules` and `obligations` before constructing the stores.
- `obligation_store.rs::temp_store()` (line 278) -- manually creates the `obligations` table after
  constructing the store.

## Solution

Update `deploy_watcher.rs::temp_db()` to create both `alert_rules` and `obligations` tables before
constructing the stores, matching the pattern used in `watchers/mod.rs::temp_stores()`.

## Scope
- **IN**: `temp_db()` test helper in `deploy_watcher.rs`
- **OUT**: Production code, migration logic, other test files

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/watchers/deploy_watcher.rs` | Update `temp_db()` to create both SQLite tables before constructing stores |

## Risks
| Risk | Mitigation |
|------|-----------|
| Schema drift between test helper and production migrations | Use `CREATE TABLE IF NOT EXISTS` with the same column definitions as `watchers/mod.rs::temp_stores()` |
