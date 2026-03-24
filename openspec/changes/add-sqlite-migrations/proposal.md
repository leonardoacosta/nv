# Proposal: Add SQLite Migrations

## Change ID
`add-sqlite-migrations`

## Summary

Add `rusqlite_migration` crate and convert existing `CREATE TABLE IF NOT EXISTS` patterns in
`messages.rs` to versioned migration v1. Set up PRAGMA user_version tracking. All future tables
(obligations, alert_rules, server_health) are added as subsequent migrations (v2+). The
`tools/schedule.rs` ScheduleStore follows the same pattern for its own `schedules.db`.

## Context
- Files: `Cargo.toml`, `crates/nv-daemon/Cargo.toml`, `crates/nv-daemon/src/messages.rs`,
  `crates/nv-daemon/src/tools/schedule.rs`
- Scope-lock reference: Phase 3 "Proactive behavior" -- SQLite versioned migrations

## Motivation

The original codebase used `CREATE TABLE IF NOT EXISTS` for all schema setup. This pattern
cannot handle `ALTER TABLE` changes safely -- adding a column to an existing table requires
detecting whether the column already exists, which is fragile. Versioned migrations via
`rusqlite_migration` solve this by tracking schema version in `PRAGMA user_version` and
applying only unapplied migrations on startup.

## Design

### Migration Runner

`messages_migrations()` returns a `Migrations<'static>` with ordered `M::up()` entries:

- **v1**: Initial schema -- messages, tool_usage, messages_fts (FTS5), api_usage, budget_alert_sent
  tables with all indexes and triggers. Converted from the original `CREATE TABLE IF NOT EXISTS`
  pattern.
- **v2**: Obligations table (added by `add-obligation-store` spec).
- **v3**: Alert rules table.
- **v4**: Server health table.

`MessageStore::init()` calls `messages_migrations().to_latest(&mut conn)` on startup.

### Schedule Store Migrations

`tools/schedule.rs` uses its own `Migrations` for `schedules.db` (separate database file),
following the identical pattern with `M::up()` and `.to_latest()`.

### PRAGMA Configuration

Both stores set `PRAGMA journal_mode=WAL` for concurrent read access. The `rusqlite_migration`
crate handles `PRAGMA user_version` internally.

## Current State

This work is **already implemented**:
- `rusqlite_migration` is in workspace dependencies (`Cargo.toml` line 29) and nv-daemon deps
- `messages_migrations()` in `messages.rs` has 4 versioned migrations (v1-v4)
- `MessageStore::init()` calls `.to_latest()` on startup
- `ScheduleStore` in `tools/schedule.rs` uses the same pattern for `schedules.db`
- FTS5 backfill runs after migrations in `MessageStore::init()`

## Remaining Work

- Unit test that migrations run cleanly on a fresh database
- Unit test that migrations are idempotent (running twice does not error)
- Unit test that PRAGMA user_version increments correctly
- Verify `cargo build` gate passes

## Dependencies

None (foundational spec).

## Out of Scope

- Defining specific table schemas for obligations, alert_rules, server_health (separate specs)
- Down migrations (not needed for single-user homelab deployment)
- Migration rollback tooling

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` -- migration tests pass
- `cargo clippy -- -D warnings` passes
