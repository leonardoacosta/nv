# Proposal: Fix Reminders DB Migration

## Change ID
`fix-reminders-db-migration`

## Summary

The daemon fails to initialize the reminder store on every start with:
`rusqlite_migrate error: MigrationDefinition(DatabaseTooFarAhead)`. v7 wrote
migrations to `~/.nv/reminders.db` that v6 does not know about. Since we
reverted to v6, the DB's `PRAGMA user_version` is ahead of the code. Fix by
detecting `DatabaseTooFarAhead` in `ReminderStore::new()` and recreating the DB
from scratch.

## Context
- File: `crates/nv-daemon/src/reminders.rs` (`ReminderStore::new`)
- Root cause: v7 added migration version(s) beyond what v6's `reminders_migrations()` defines
- No user data at risk — reminders are ephemeral one-shot timers, stale rows have no value

## Motivation

Every daemon restart logs an error and disables reminder tools entirely. Reminders
are broken until someone manually deletes `~/.nv/reminders.db`. The fix is
mechanical: catch the specific error, delete the file, and reopen with migrations
applied from scratch. A warn-level log entry is sufficient notification.

## Requirements

### Req-1: Detect DatabaseTooFarAhead and Recreate

In `ReminderStore::new()`, after `reminders_migrations().to_latest(&mut conn)`
fails, inspect the error string for `DatabaseTooFarAhead`. If matched:

1. Close the connection.
2. Delete the DB file at `db_path`.
3. Reopen the connection and re-run migrations.
4. Log a `warn!` message: `"reminders.db was ahead of migrations — recreated from scratch"`.
5. Return the fresh store.

If the error is any other variant, propagate it unchanged (existing behavior).

### Req-2: Handle Missing DB Path Gracefully

The delete step should not panic if the file does not exist (e.g., in-memory or
temp paths used by tests). Use `std::fs::remove_file` and ignore
`ErrorKind::NotFound`.

## Scope
- **IN**: `ReminderStore::new()` error recovery path in `reminders.rs`
- **OUT**: Schema changes, migration version bumps, altering the backup/restore or deployment flow

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/reminders.rs` | Add `DatabaseTooFarAhead` recovery branch in `ReminderStore::new()` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Deleting a DB with valid future reminders | Acceptable: reminders are transient; user will simply re-set any needed reminders after daemon restart |
| Loop if migration itself is broken | Recovery path runs migrations exactly once; any failure on the second attempt propagates as a hard error |
