# Implementation Tasks

<!-- beads:epic:TBD -->

## Req-1: Pipeline Profiling

- [ ] [1.1] [P-1] Add `latency_spans` table migration to `MessageStore` in `crates/nv-daemon/src/messages.rs` — `CREATE TABLE latency_spans (id, worker_id, stage, duration_ms, recorded_at)` with index on `(stage, recorded_at)` [owner:api-engineer]
- [ ] [1.2] [P-1] Add `log_latency_span(worker_id: &str, stage: &str, duration_ms: i64)` method to `MessageStore` [owner:api-engineer]
- [ ] [1.3] [P-1] Add `latency_p95(stage: &str, since_hours: u32) -> Option<f64>` query to `MessageStore` — uses SQLite percentile approximation (`ORDER BY duration_ms LIMIT 1 OFFSET ...`) [owner:api-engineer]
- [ ] [1.4] [P-2] Wire orchestrator event loop (`orchestrator.rs`) to persist `WorkerEvent::StageComplete` events: call `deps.message_store.lock().log_latency_span(worker_id, stage, duration_ms)` [owner:api-engineer]
- [ ] [1.5] [P-2] Add `receive` span: record time from trigger arrival (`process_trigger_batch` entry) to `WorkerPool::dispatch` call in `orchestrator.rs`; emit as `WorkerEvent::StageComplete { stage: "receive", duration_ms }` [owner:api-engineer]
- [ ] [1.6] [P-3] Add `delivery` span to `Worker::run`: record `Instant::now()` before the Telegram `send_message` call and emit `StageComplete { stage: "delivery" }` after it completes [owner:api-engineer]
- [ ] [1.7] [P-3] Add nightly `latency_spans` pruning: in the existing cron scheduler, add `DELETE FROM latency_spans WHERE recorded_at < datetime('now', '-30 days')` [owner:api-engineer]

## Req-2: Streaming Response Delivery

- [ ] [2.1] [P-1] Add `edit_message_text(chat_id: i64, message_id: i64, text: &str) -> Result<()>` to `TelegramClient` in `crates/nv-daemon/src/channels/telegram/client.rs` — POST `/editMessageText` [owner:api-engineer]
- [ ] [2.2] [P-1] Check if `delete_message` already exists on `TelegramClient`; add it if missing — POST `/deleteMessage` [owner:api-engineer]
- [ ] [2.3] [P-1] Add `send_messages_streaming` variant to `ClaudeClient` in `claude.rs`: spawn `claude -p --output-format stream-json` and return an `mpsc::Receiver<StreamEvent>` where `StreamEvent` is `Text(String)` | `ToolUse(...)` | `Done(ApiResponse)` [owner:api-engineer]
- [ ] [2.4] [P-2] Implement streaming delivery in `Worker::run`: at start of `api_call` stage send "..." placeholder via `TelegramClient::send_message`, capture `message_id`; accumulate `Text` stream events; call `edit_message_text` every 1.5s or on >50-char change; send final edit on `Done` [owner:api-engineer]
- [ ] [2.5] [P-2] Handle tool-only turns in streaming delivery: if no `Text` events arrive before `Done`, delete the "..." placeholder and proceed with normal response send [owner:api-engineer]
- [ ] [2.6] [P-2] Add `STREAMING_EDIT_INTERVAL_MS` constant (default 1500) and `STREAMING_EDIT_MIN_DELTA_CHARS` constant (default 50) to `worker.rs` for tunability [owner:api-engineer]
- [ ] [2.7] [P-3] Add `parse_stream_response()` fn isolated from existing `parse_json_response()` — handles `assistant`/`text`/`tool_use`/`result` event types from stream-json format [owner:api-engineer]

## Req-3: Persistent Subprocess Investigation

- [ ] [3.1] [P-1] Reproduce the stream-json subprocess hang: write a minimal test harness in `crates/nv-daemon/src/claude.rs` tests section that spawns the persistent process, sends one turn, and asserts a response arrives within 15s [owner:api-engineer]
- [ ] [3.2] [P-1] Diagnose root cause: run `claude --dangerously-skip-permissions -p --verbose --input-format stream-json --output-format stream-json --model claude-sonnet-4-5` manually; capture stdout/stderr; document findings in a `# Known Issues` block in `claude.rs` [owner:api-engineer]
- [ ] [3.3] [P-2] Apply fix based on diagnosis — most likely candidates: (a) remove `--tools Read,Glob,Grep,Bash(git:*)` from spawn args and rely on daemon tool loop; (b) add `--no-mcp` flag to suppress hook loading; (c) update `drain_init_events` timeout from 10s to 20s if hooks are slow [owner:api-engineer]
- [ ] [3.4] [P-2] Set `fallback_only: false` in `PersistentSession::new` and remove the disabling comment once the test harness from 3.1 passes [owner:api-engineer]
- [ ] [3.5] [P-3] Add `FALLBACK_RESET_DURATION` integration test: verify that after 5 consecutive failures, the session enters `fallback_only` mode, and that after the reset duration it retries persistent mode [owner:api-engineer]

## Req-4: Parallel Context Build

- [ ] [4.1] [P-2] Extract `load_recent_messages(deps: &SharedDeps) -> Option<String>` as a standalone async fn in `worker.rs`; implementation wraps existing `Mutex` lock in `tokio::task::spawn_blocking` [owner:api-engineer]
- [ ] [4.2] [P-2] Extract `load_memory_context(deps: &SharedDeps, trigger: &str) -> Option<String>` as a standalone async fn; uses `spawn_blocking` around `deps.memory.get_context_summary_for` [owner:api-engineer]
- [ ] [4.3] [P-2] Extract `load_followup_context(deps: &SharedDeps) -> Option<String>` as a standalone async fn [owner:api-engineer]
- [ ] [4.4] [P-2] Extract `load_conversation_turns(deps: &SharedDeps) -> Vec<Message>` as a standalone async fn; wraps `ConversationStore::load` in `spawn_blocking` [owner:api-engineer]
- [ ] [4.5] [P-2] Replace sequential context build block in `Worker::run` with `tokio::join!(load_recent_messages, load_memory_context, load_followup_context, load_conversation_turns)` [owner:api-engineer]

## Req-5: HTTP Connection Pool Audit

- [ ] [5.1] [P-2] Audit `TelegramClient::new` call sites — confirm that `TelegramClient` is constructed once at daemon startup (in `main.rs`) and cloned cheaply thereafter; fix any per-call construction [owner:api-engineer]
- [ ] [5.2] [P-3] Add `#[cfg(test)] fn reqwest_client_is_cheaply_cloneable()` doc test comment explaining the `reqwest::Client` Arc semantics to prevent future regressions [owner:api-engineer]

## Req-6 + Req-7: Dashboard Latency Endpoint and Chart

- [ ] [6.1] [P-2] Add `GET /api/latency` handler to `dashboard.rs` — returns P50 and P95 per stage for last 24h and 7d windows using `MessageStore::latency_p95` [owner:api-engineer]
- [ ] [6.2] [P-2] Define response type `LatencyResponse { stages: Vec<StageLatency> }` where `StageLatency { stage: String, p50_ms: u64, p95_ms: u64, window: String }` [owner:api-engineer]
- [ ] [6.3] [P-3] Add latency chart component to dashboard SPA (`dashboard/src/`) — horizontal bar chart per stage showing P50 (solid) and P95 (outlined) bars for the 24h window; fetch from `/api/latency` [owner:ui-engineer]
- [ ] [6.4] [P-3] Add latency chart to the existing dashboard Sessions or Stats page (not a separate route) [owner:ui-engineer]

## Verify

- [ ] [7.1] `cargo build` passes [owner:api-engineer]
- [ ] [7.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [7.3] `cargo test` — new tests: `latency_spans_migration`, `log_and_query_latency_span`, `streaming_cold_start_delivers_text_events`, `parallel_context_build_completes`, `persistent_session_harness` [owner:api-engineer]
- [ ] [7.4] [user] Manual test: send a simple query on Telegram — observe "..." placeholder appear within 2s, text fills in as it streams, final message is clean [owner:api-engineer]
- [ ] [7.5] [user] Manual test: open dashboard latency chart — P50/P95 bars render for `api_call` and `context_build` stages after a few turns [owner:api-engineer]
- [ ] [7.6] [user] Manual benchmark: 5 simple queries back-to-back — confirm P95 under 10s in dashboard chart [owner:api-engineer]
