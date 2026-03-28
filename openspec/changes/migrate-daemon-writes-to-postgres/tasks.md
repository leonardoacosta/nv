# Implementation Tasks

<!-- beads:epic:nv-l5fb -->

## DB Batch

- [x] [1.1] [P-1] Create `crates/nv-daemon/src/pg_pool.rs` -- shared Postgres connection pool using `tokio_postgres::Client` behind `Arc`. Connect to `DATABASE_URL` at startup with TLS detection (TLS for Neon URLs, NoTls for local) matching `scheduler.rs` pattern. Spawn connection driver task. Provide `query`/`execute` interface. Handle reconnection on connection loss. Connection timeout: 5 seconds. [owner:db-engineer] [beads:nv-wu2c]
- [x] [1.2] [P-1] Modify `crates/nv-daemon/src/main.rs` -- initialize `PgPool` at daemon startup, inject into `SharedDeps` so all workers and orchestrator can access it via `Arc` [owner:db-engineer] [beads:nv-lijc]
- [x] [1.3] [P-2] Modify `crates/nv-daemon/src/lib.rs` -- export `pg_pool`, `pg_obligation_store`, and `pg_contact_store` modules [owner:db-engineer] [beads:nv-aa28]

## API Batch

### Req-1/Req-7: PgPool + Config

- [x] [2.1] [P-1] Add `pg_write_enabled` config flag (default: `true`) to daemon config. When `false`, skip all Postgres writes without code changes. Log at warn level when a PG write fails and SQLite fallback is used. [owner:api-engineer] [beads:nv-qju2]

### Req-2: Obligations

- [x] [2.2] [P-1] Create `crates/nv-daemon/src/pg_obligation_store.rs` -- `PgObligationStore` with methods: `create()` INSERT into `obligations` table (id, detected_action, owner, status, priority, project_code, source_channel, source_message, deadline, created_at, updated_at); `update_status()`, `update_status_and_owner()`, `update_detected_action()`, `snooze()` UPDATE methods; `update_last_attempt_at()` for execution cooldown. UUID via `uuid::Uuid::new_v4()`, timestamps via `NOW()`. Omit `owner_reason` (not in PG schema). [owner:api-engineer] [beads:nv-ep44]
- [x] [2.3] [P-1] Modify `crates/nv-daemon/src/orchestrator.rs` and `crates/nv-daemon/src/worker.rs` -- wire dual-write for obligations: every SQLite write is followed by a PG write (guarded by `pg_write_enabled`). PG failures logged at warn, do not block. [owner:api-engineer] [beads:nv-baoe]

### Req-3: Contacts

- [x] [2.4] [P-1] Create `crates/nv-daemon/src/pg_contact_store.rs` -- `PgContactStore` with methods: `create()` INSERT into `contacts` (id, name, channel_ids as JSONB via `serde_json::to_value()`, relationship_type, notes, created_at); `update()`, `delete()`. [owner:api-engineer] [beads:nv-sgo9]
- [x] [2.5] [P-1] Modify `crates/nv-daemon/src/http.rs` -- wire dual-write for contacts: every SQLite contact write (create/update/delete) is followed by a PG write. PG failures logged at warn, do not block. [owner:api-engineer] [beads:nv-fwso]

### Req-4: Sessions

- [x] [2.6] [P-1] Modify `crates/nv-daemon/src/cc_sessions.rs` -- on `CcSessionManager::start()`: INSERT into PG `sessions` table (id as UUID, project, command, status='running', trigger_type, started_at=NOW()). Use clean UUID for PG (strip `ta-` prefix). [owner:api-engineer] [beads:nv-5fbz]
- [x] [2.7] [P-1] Modify `crates/nv-daemon/src/cc_sessions.rs` -- on `stop()` and health monitor transitions (Completed, Failed, Error): UPDATE `sessions` SET status, stopped_at=NOW(). [owner:api-engineer] [beads:nv-umzn]
- [ ] [2.8] [P-2] Modify `crates/nv-daemon/src/orchestrator.rs` -- increment `message_count` and `tool_count` on `sessions` row as `WorkerEvent::ToolCalled` and message completion events flow through. [owner:api-engineer] [beads:nv-qgu1]

### Req-5: Session Events

- [x] [2.9] [P-1] Implement session event writer in a new `pg_session_events.rs` -- `PgSessionEventWriter` with buffered inserts (flush every 5s or on session end). Injected into SharedDeps. Ready for wiring to WorkerEvent handlers once worker_id-to-session_id mapping exists. [owner:api-engineer] [beads:nv-noy5]
- [x] [2.10] [P-2] Use fire-and-forget `tokio::spawn` for session event writes to avoid blocking the orchestrator loop. [owner:api-engineer] [beads:nv-wosx]

### Req-6: Briefings

- [x] [2.11] [P-1] Modify `crates/nv-daemon/src/orchestrator.rs` -- add Postgres INSERT into `briefings` table (id, generated_at, content, sources_status as JSONB, suggested_actions as JSONB) alongside existing JSONL write (dual-write for Phase 1). [owner:api-engineer] [beads:nv-6dhh]
- [ ] [2.12] [P-1] Remove `BriefingStore` from `SharedDeps` and `HttpState`. Remove `/api/briefings` endpoint in `http.rs` that reads from JSONL store (dashboard reads via tRPC from Postgres). Keep `BriefingEntry` type. [owner:api-engineer] [beads:nv-vt41] [deferred] Phase 3 — remove after dual-write verified

### Req-8: Error Handling

- [x] [2.13] [P-1] Ensure all PG operations in `pg_obligation_store.rs`, `pg_contact_store.rs`, session writes, and briefing writes are wrapped in `Result` with warn-level logging on failure. PG errors must never crash the daemon. Reconnection attempted on next write if connection dropped. [owner:api-engineer] [beads:nv-bkcc]

## UI Batch

(No UI tasks -- dashboard already reads from Postgres via tRPC/Drizzle)

## E2E Batch

- [x] [4.1] `cargo build` passes for `crates/nv-daemon` with all new modules [owner:api-engineer] [beads:nv-yfw0]
- [ ] [4.2] [user] Deploy daemon, trigger obligation creation via Telegram, verify obligation row appears in Postgres `obligations` table and on dashboard [owner:api-engineer] [beads:nv-pvon]
- [ ] [4.3] [user] Stop Postgres, verify daemon continues operating with SQLite. Restart Postgres, verify writes resume. [owner:api-engineer] [beads:nv-b2n2]
- [ ] [4.4] [user] After running for 24h with dual-write, compare row counts between SQLite and Postgres for obligations and contacts [owner:api-engineer] [beads:nv-8sos]
