# Implementation Tasks

<!-- beads:epic:nv-clp -->

## 1. Rust: cold_start_store.rs

- [ ] [1.1] [P-1] Create `crates/nv-daemon/src/cold_start_store.rs` — define `ColdStartEvent` struct (session_id, started_at, context_build_ms, first_response_ms, total_ms, tool_count, tokens_in, tokens_out, trigger_type) and `Percentiles` struct (p50_ms, p95_ms, p99_ms, sample_count) [owner:api-engineer]
- [ ] [1.2] [P-1] Implement `ColdStartStore::new(db_path: &Path) -> Result<Self>` — opens rusqlite Connection to messages.db (same path as MessageStore/ObligationStore), creates schema if table missing [owner:api-engineer]
- [ ] [1.3] [P-1] Implement `ColdStartStore::insert(event: &ColdStartEvent) -> Result<()>` — inserts a row into `cold_start_events` [owner:api-engineer]
- [ ] [1.4] [P-2] Implement `ColdStartStore::get_recent(limit: usize) -> Result<Vec<ColdStartEvent>>` — returns last N events ordered by started_at DESC [owner:api-engineer]
- [ ] [1.5] [P-2] Implement `ColdStartStore::get_percentiles(window_hours: u32) -> Result<Percentiles>` — computes P50/P95/P99 of total_ms for events within the last N hours using ORDER BY + offset math [owner:api-engineer]
- [ ] [1.6] [P-1] Add `mod cold_start_store;` declaration in `crates/nv-daemon/src/main.rs` or `lib.rs` [owner:api-engineer]

## 2. Rust: DB migration

- [ ] [2.1] [P-1] Add `cold_start_events` table to the `MessageStore` migration sequence in `messages.rs` (or wherever migrations are defined) — columns: id, session_id, started_at, context_build_ms, first_response_ms, total_ms, tool_count, tokens_in, tokens_out, trigger_type [owner:api-engineer]

## 3. Rust: SharedDeps + main.rs wiring

- [ ] [3.1] [P-1] Add `cold_start_store: Option<Arc<std::sync::Mutex<ColdStartStore>>>` field to `SharedDeps` in `worker.rs` (follow same pattern as `obligation_store`, `schedule_store`) [owner:api-engineer]
- [ ] [3.2] [P-1] In `main.rs`: construct `ColdStartStore` using the same `messages.db` path, wrap in `Arc<Mutex<>>`, populate `SharedDeps.cold_start_store` [owner:api-engineer]

## 4. Rust: Worker::run instrumentation

- [ ] [4.1] [P-1] In `Worker::run` (`worker.rs`): add a `tool_count: u32` local counter, increment it on each iteration of the tool dispatch loop (where tools are executed and `WorkerEvent::ToolCalled` is sent) [owner:api-engineer]
- [ ] [4.2] [P-1] After the tool use loop completes and `tokens_in`/`tokens_out` are known, determine `trigger_type` from the first trigger (`Trigger::Message` → "message", `Trigger::Cron` → "cron", etc.) [owner:api-engineer]
- [ ] [4.3] [P-1] Build `ColdStartEvent` from existing variables: `session_id` from `task_id`, `started_at` from `chrono::Utc::now()` at task start, `context_build_ms` from the `StageComplete` event duration, `first_response_ms` from `response_time_ms as u64`, `total_ms` from `task_start.elapsed().as_millis() as u64` [owner:api-engineer]
- [ ] [4.4] [P-1] Insert the event via `tokio::task::spawn_blocking` wrapping `cold_start_store.lock().unwrap().insert(&event)` — fire-and-forget (log on error, do not propagate) [owner:api-engineer]

## 5. Rust: /api/cold-starts endpoint

- [ ] [5.1] [P-2] Locate the HTTP health/API server in the daemon (likely `crates/nv-daemon/src/health.rs` or `api.rs`) and add route `GET /api/cold-starts?limit=<n>` [owner:api-engineer]
- [ ] [5.2] [P-2] Handler calls `cold_start_store.get_recent(limit)` and `cold_start_store.get_percentiles(24)`, serializes to JSON as `{ events: [...], percentiles: { p50_ms, p95_ms, p99_ms, sample_count } }` [owner:api-engineer]
- [ ] [5.3] [P-2] Default `limit` to 200 if not provided; cap at 1000 [owner:api-engineer]

## 6. Dashboard: ColdStartPage

- [ ] [6.1] [P-2] Create `dashboard/src/pages/ColdStartPage.tsx` — fetches `GET /api/cold-starts?limit=200` from the daemon health server, renders latency chart and percentile cards [owner:ui-engineer]
- [ ] [6.2] [P-2] Latency chart: line chart with `total_ms` and `first_response_ms` series over time (last 100 events), X-axis is `started_at`, Y-axis is milliseconds; use the charting library already present in dashboard (check `package.json` — likely Recharts) [owner:ui-engineer]
- [ ] [6.3] [P-2] Add 20-event rolling average of `total_ms` as a third series (trend line) computed client-side from the events array [owner:ui-engineer]
- [ ] [6.4] [P-2] Percentile cards row: three cards showing P50, P95, P99 (last 24h window from the `percentiles` response field) with units in seconds (e.g. "8.5s") [owner:ui-engineer]
- [ ] [6.5] [P-3] Stats row below chart: average tool_count, average tokens_in, average tokens_out for the visible event window, computed client-side [owner:ui-engineer]
- [ ] [6.6] [P-2] Register the new page in `dashboard/src/App.tsx` — add route and nav link using the same pattern as existing pages (e.g. UsagePage) [owner:ui-engineer]

## 7. Verify

- [ ] [7.1] `cargo build` passes [owner:api-engineer]
- [ ] [7.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [7.3] `cargo test` — unit tests for `ColdStartStore`: `insert_and_get_recent`, `get_percentiles_empty_returns_zeros`, `get_percentiles_computes_correct_p50`, `migration_creates_table` [owner:api-engineer]
- [ ] [7.4] Manual smoke test: send a Telegram message to Nova, check `messages.db` for a new row in `cold_start_events`, verify `total_ms` is plausible (~8000–15000ms) [owner:api-engineer]
- [ ] [7.5] Dashboard smoke test: open ColdStartPage in browser, verify chart renders with real data, percentile cards show non-zero values [owner:ui-engineer]
