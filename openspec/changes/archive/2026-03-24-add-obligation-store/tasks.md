# Tasks: add-obligation-store

## Dependencies

- `add-sqlite-migrations` (migration runner must exist before adding v2)

## Tasks

### Schema (already complete)

- [x] [1.1] Add migration v2 to `messages_migrations()` in `messages.rs` -- CREATE TABLE obligations with id, source_channel, source_message, detected_action, project_code, priority, status, owner, owner_reason, created_at, updated_at columns. Add indexes on status, priority, owner [owner:api-engineer]

### Types (already complete)

- [x] [2.1] Define `ObligationStatus` enum in `crates/nv-core/src/types.rs` -- variants Open, InProgress, Done, Dismissed with `as_str()`, `FromStr`, `Display`, `Serialize`, `Deserialize` [owner:api-engineer]
- [x] [2.2] Define `ObligationOwner` enum in `crates/nv-core/src/types.rs` -- variants Nova, Leo with `as_str()`, `FromStr`, `Display`, `Serialize`, `Deserialize` [owner:api-engineer]
- [x] [2.3] Define `Obligation` struct in `crates/nv-core/src/types.rs` -- all columns as typed fields with Serialize/Deserialize [owner:api-engineer]

### CRUD Implementation (already complete)

- [x] [3.1] Create `crates/nv-daemon/src/obligation_store.rs` with `ObligationStore` struct wrapping `rusqlite::Connection` [owner:api-engineer]
- [x] [3.2] Implement `NewObligation` input type with all fields for creating a new obligation [owner:api-engineer]
- [x] [3.3] Implement `ObligationStore::new(db_path)` -- open connection with WAL mode, no schema init (owned by MessageStore) [owner:api-engineer]
- [x] [3.4] Implement `create(NewObligation) -> Result<Obligation>` -- insert and read back via get_by_id [owner:api-engineer]
- [x] [3.5] Implement `get_by_id(id) -> Result<Option<Obligation>>` -- single row lookup with row_to_obligation mapper [owner:api-engineer]
- [x] [3.6] Implement `list_by_status(status) -> Result<Vec<Obligation>>` -- filtered query ordered by priority ASC, created_at ASC [owner:api-engineer]
- [x] [3.7] Implement `list_by_owner(owner) -> Result<Vec<Obligation>>` -- filtered query ordered by priority ASC, created_at ASC [owner:api-engineer]
- [x] [3.8] Implement `list_all() -> Result<Vec<Obligation>>` -- unfiltered query ordered by priority ASC, created_at ASC [owner:api-engineer]
- [x] [3.9] Implement `update_status(id, status) -> Result<bool>` -- UPDATE with datetime('now') for updated_at, returns row count [owner:api-engineer]
- [x] [3.10] Implement `update_status_and_owner(id, status, owner) -> Result<bool>` -- UPDATE both fields with datetime('now') for updated_at [owner:api-engineer]
- [x] [3.11] Implement `count_open_by_priority() -> Result<Vec<(i32, i64)>>` -- GROUP BY priority for morning briefing [owner:api-engineer]
- [x] [3.12] Implement `count_open() -> Result<i64>` -- simple COUNT for dashboard [owner:api-engineer]

### SharedDeps Integration (already complete)

- [x] [4.1] Add `obligation_store: Option<Arc<Mutex<ObligationStore>>>` to `SharedDeps` in `worker.rs` [owner:api-engineer]

### Existing Tests (already complete)

- [x] [5.1] Test create_and_get_by_id -- insert obligation, verify fields, fetch by ID [owner:api-engineer]
- [x] [5.2] Test get_by_id_missing_returns_none [owner:api-engineer]
- [x] [5.3] Test list_by_status_returns_matching_rows -- create 2, mark 1 done, verify filter [owner:api-engineer]
- [x] [5.4] Test list_by_owner_filters_correctly -- create Nova + Leo obligations, verify filter [owner:api-engineer]
- [x] [5.5] Test update_status_changes_status -- update to InProgress, verify [owner:api-engineer]
- [x] [5.6] Test update_status_missing_id_returns_false [owner:api-engineer]
- [x] [5.7] Test count_open_tracks_correctly -- create 2, close 1, verify count [owner:api-engineer]
- [x] [5.8] Test list_by_status_ordered_by_priority -- insert in reverse order, verify ASC ordering [owner:api-engineer]

### Remaining Tests

- [x] [6.1] Unit test: `update_status_and_owner` -- create obligation with owner=Nova, call update_status_and_owner to set status=InProgress and owner=Leo, verify both fields changed and updated_at is newer than created_at [owner:api-engineer]
- [x] [6.2] Unit test: `count_open_by_priority` -- create obligations at priorities 0, 1, 2 (multiple at P2), close one P2, verify grouped counts match expected values [owner:api-engineer]
- [x] [6.3] Unit test: `list_all` -- create mix of open/done/dismissed obligations, verify list_all returns all regardless of status, ordered by priority ASC [owner:api-engineer]

### Verify

- [x] [7.1] `cargo build` passes for all workspace members [owner:api-engineer]
- [x] [7.2] `cargo test -p nv-daemon` -- all obligation_store tests pass [owner:api-engineer]
- [x] [7.3] `cargo clippy -- -D warnings` passes [owner:api-engineer]
