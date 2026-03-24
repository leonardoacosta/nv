# Proposal: Add Obligation Store

## Change ID
`add-obligation-store`

## Summary

Create the obligations table via SQLite migration v2. Define Rust types (`Obligation`,
`ObligationStatus`, `ObligationOwner`) in nv-core. Implement `ObligationStore` with CRUD
operations and query methods: list by status, list by owner, count open, update status,
update status and owner.

## Context
- Depends on: `add-sqlite-migrations`
- Files: `crates/nv-daemon/src/messages.rs` (migration v2), `crates/nv-daemon/src/obligation_store.rs`
  (new), `crates/nv-core/src/types.rs` (types)
- Scope-lock reference: Phase 3 "Proactive behavior" -- obligation store

## Motivation

The obligation engine needs a persistent store to track detected commitments and action items
across daemon restarts. Without a dedicated store, detected obligations would be lost on restart
and there would be no way to query open obligations for morning briefings or status reports.

## Design

### Schema (Migration v2 in messages.rs)

```sql
CREATE TABLE obligations (
    id TEXT PRIMARY KEY,
    source_channel TEXT NOT NULL,
    source_message TEXT,
    detected_action TEXT NOT NULL,
    project_code TEXT,
    priority INTEGER NOT NULL DEFAULT 2,
    status TEXT NOT NULL DEFAULT 'open',
    owner TEXT NOT NULL DEFAULT 'nova',
    owner_reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_obligations_status ON obligations(status);
CREATE INDEX idx_obligations_priority ON obligations(priority);
CREATE INDEX idx_obligations_owner ON obligations(owner);
```

### Rust Types (nv-core/src/types.rs)

- `ObligationStatus` enum: `Open`, `InProgress`, `Done`, `Dismissed` with `as_str()`,
  `FromStr`, `Display` implementations
- `ObligationOwner` enum: `Nova`, `Leo` with `as_str()`, `FromStr`, `Display` implementations
- `Obligation` struct: all columns as typed fields, `Serialize`/`Deserialize`

### ObligationStore (obligation_store.rs)

- `ObligationStore::new(db_path)` -- opens SQLite connection with WAL mode. Schema migration is
  owned by `MessageStore`, so callers must ensure `MessageStore::init()` runs first.
- `create(NewObligation) -> Result<Obligation>` -- insert and read back
- `get_by_id(id) -> Result<Option<Obligation>>` -- single lookup
- `list_by_status(status) -> Result<Vec<Obligation>>` -- filtered, ordered by priority ASC then created_at ASC
- `list_by_owner(owner) -> Result<Vec<Obligation>>` -- filtered, ordered by priority ASC then created_at ASC
- `list_all() -> Result<Vec<Obligation>>` -- all rows, ordered
- `update_status(id, status) -> Result<bool>` -- touches updated_at
- `update_status_and_owner(id, status, owner) -> Result<bool>` -- touches updated_at
- `count_open_by_priority() -> Result<Vec<(i32, i64)>>` -- grouped counts for briefing
- `count_open() -> Result<i64>` -- total open count

### SharedDeps Integration

`ObligationStore` is wrapped in `Arc<Mutex<ObligationStore>>` and stored in `SharedDeps` so the
orchestrator and workers can access it. The field is `Option<...>` to gracefully handle DB init
failure.

## Current State

This work is **already implemented**:
- Migration v2 exists in `messages_migrations()` (messages.rs lines 148-165)
- Types defined in `nv-core/src/types.rs` (ObligationStatus, ObligationOwner, Obligation)
- Full `ObligationStore` in `obligation_store.rs` with all CRUD methods
- Unit tests for create, get_by_id, list_by_status, list_by_owner, update_status, count_open,
  ordering by priority
- SharedDeps integration in `worker.rs`

## Remaining Work

- Unit test: `update_status_and_owner` changes both fields correctly
- Unit test: `count_open_by_priority` returns correct grouped counts
- Unit test: `list_all` returns all obligations regardless of status
- Verify `cargo build` gate passes

## Dependencies

- `add-sqlite-migrations` (migration runner must exist before adding v2)

## Out of Scope

- Obligation detection logic (separate spec: `add-obligation-detection`)
- Telegram notification formatting (separate spec: `add-obligation-telegram-ux`)
- Archival or TTL for old obligations

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` -- all obligation_store tests pass
- `cargo clippy -- -D warnings` passes
