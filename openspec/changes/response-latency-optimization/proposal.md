# Proposal: Response Latency Optimization

## Change ID
`response-latency-optimization`

## Summary

Profile the end-to-end response pipeline, eliminate avoidable delays, add streaming response
delivery to Telegram, and add per-stage latency tracing. Target: P95 under 10s for simple
queries, under 15s for tool-calling turns.

## Context
- Phase: Wave 3d — after API client + tool protocol + persistent-conversation-state
- Depends on: `persistent-conversation-state` (full pipeline must be in place)
- Extends: `crates/nv-daemon/src/worker.rs`, `crates/nv-daemon/src/claude.rs`,
  `crates/nv-daemon/src/orchestrator.rs`, `crates/nv-daemon/src/channels/telegram/client.rs`,
  `crates/nv-daemon/src/dashboard.rs`
- Related: `add-worker-dag-events` (existing `WorkerEvent` spans reused as tracing primitives),
  `add-cold-start-logging` (latency chart ties into that logging infrastructure)

## Motivation

The current cold-start path (`fallback_only = true` in `PersistentSession`) spawns a fresh
`claude -p --output-format json` subprocess on every turn. Observed wall-clock latencies are
8-14s for simple queries and 20-60s for multi-tool turns. There are five compounding sources
of delay:

1. **Cold subprocess spawn** (~3-4s) — Node.js + CC CLI startup overhead on every single turn
   because `fallback_only` is pinned to `true`.
2. **Sequential context build** (~200-400ms) — `recent_messages`, `memory_context`,
   `followup_context`, and `conversation_store.load()` are called one after another under
   separate `Mutex` locks inside `Worker::run`.
3. **No streaming delivery** — Telegram only receives the response after the entire turn
   completes. The user watches nothing happen for 8-14s, then a full message appears.
4. **No connection keep-alive** — The `reqwest` HTTP client inside `TelegramClient` is
   recreated on every message send, incurring a TLS handshake per delivery.
5. **No per-stage timing** — There is no structured latency data to know which stages are
   slowest. `WorkerEvent::StageComplete` carries `duration_ms` but it is only logged; it is
   not persisted or surfaced in the dashboard.

## Requirements

### Req-1: Pipeline Profiling — Instrument Every Stage

Extend `Worker::run` to emit a `StageStarted` / `StageComplete` event pair for each named
stage. Persist stage durations into `messages.db` (new `latency_spans` table) so the
dashboard can query P50/P95 per stage.

Named stages (in order):
- `receive` — time from trigger arrival to worker spawn (measured in orchestrator)
- `context_build` — already instrumented; start/complete already emitted
- `api_call` — Claude CLI invocation (from `call_start` to first byte / completion)
- `tool_loop` — each tool execution cycle (per iteration, not total)
- `delivery` — Telegram `sendMessage` / `editMessageText` round-trip

The existing `WorkerEvent::StageComplete { duration_ms }` already carries timing. The missing
piece is persistence: the orchestrator event loop must INSERT into `latency_spans` on each
`StageComplete` event it receives.

### Req-2: Streaming Response Delivery to Telegram

Send an initial placeholder message to Telegram at the start of the API call stage, then
edit it as tokens arrive. This requires switching the cold-start path from
`--output-format json` (single JSON blob at end) to `--output-format stream-json` (line-by-line
events during generation).

Delivery protocol:
1. Worker emits initial "..." message via `TelegramClient::send_message` at start of
   `api_call` stage → captures returned `message_id`.
2. As `assistant` text events arrive from the stream, accumulate into a buffer.
3. Edit the message via `TelegramClient::edit_message_text` every ~1.5s (Telegram rate limit:
   1 edit/s per chat; 1.5s provides headroom) or when the buffer changes by >50 chars.
4. Send final edit when `result` event arrives (ensures final state is clean, no truncation).
5. If streaming produces no text (tool-only turn), delete the placeholder and send final
   response normally.

New `TelegramClient` methods required:
- `edit_message_text(chat_id: i64, message_id: i64, text: &str) -> Result<()>`
- `delete_message(chat_id: i64, message_id: i64) -> Result<()>` (already may exist — check first)

### Req-3: Persistent Subprocess — Fix and Re-enable

The `PersistentSession` is pinned to `fallback_only: true` with a comment:
> "Persistent mode disabled: the CC CLI stream-json subprocess never sends response data back
> (likely a CC 2.1.81 bug with stream-json + hooks). Cold-start mode works reliably (~8s).
> Re-enable once the root cause is identified."

This spec owns diagnosing and resolving that bug. The investigation should:
1. Run `claude --dangerously-skip-permissions -p --verbose --input-format stream-json
   --output-format stream-json --model ...` manually with a minimal payload and capture
   the full stdout/stderr exchange.
2. Identify whether the issue is hooks interference, the `--tools` flag, or stream-json
   protocol mismatch in CC 2.1.81+.
3. Apply the fix (likely: remove `--tools` from the spawn args and rely on the daemon's own
   tool dispatch loop, or add `--no-mcp` to suppress hook loading).
4. Set `fallback_only: false` and remove the override comment once verified.

If the persistent path cannot be fixed within this spec's scope, document the exact failure
mode in a `KNOWN_ISSUES.md` note and proceed with Req-2 (streaming cold-start) as the primary
latency win.

### Req-4: Parallel Context Build

The four context sources in `Worker::run` are independent reads; they block each other only
because they share a `Mutex<MessageStore>` and `Mutex<ConversationStore>`. Refactor to:

```rust
// Run independently where possible
let (recent_msgs, memory_ctx, followup_ctx, prior_turns) = tokio::join!(
    load_recent_messages(&deps),
    load_memory_context(&deps, &trigger_text),
    load_followup_context(&deps),
    load_conversation_turns(&deps),
);
```

Constraint: `MessageStore` and `ConversationStore` use `std::sync::Mutex` (not
`tokio::sync::Mutex`). Locking them across `.await` points is unsound. The correct approach is
to spawn each load as a `tokio::task::spawn_blocking` call and then join the handles. This
keeps the async runtime unblocked while the SQLite reads execute on the blocking thread pool.

Expected gain: ~100-200ms on cache-miss turns where SQLite is warm but mutex contention
forces serialization.

### Req-5: HTTP Connection Pool for Telegram Client

`TelegramClient` holds a `reqwest::Client`. If it is constructed per-message-send rather than
once at startup and reused, each call incurs a TLS handshake (~50-100ms).

Check: confirm that `TelegramClient` stores a single `reqwest::Client` as a field (it should —
the struct owns `client: reqwest::Client`). If so, verify it is constructed once and cloned
cheaply (reqwest clients are cheaply cloneable via internal `Arc`). If `TelegramClient::new`
is being called per-send anywhere, fix the call sites.

### Req-6: Latency Tracing — `latency_spans` Table

Add a new SQLite table to `messages.db` (migrated via `rusqlite_migration`):

```sql
CREATE TABLE IF NOT EXISTS latency_spans (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    worker_id   TEXT    NOT NULL,  -- Uuid as string
    stage       TEXT    NOT NULL,  -- "receive" | "context_build" | "api_call" | "tool_loop" | "delivery"
    duration_ms INTEGER NOT NULL,
    recorded_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_latency_stage ON latency_spans(stage, recorded_at);
```

Wire in `MessageStore`: add `log_latency_span(worker_id: &str, stage: &str, duration_ms: i64)`
and `latency_p95(stage: &str, since_hours: u32) -> Option<f64>` query methods.

### Req-7: Dashboard Latency Chart

Add a `/api/latency` endpoint to `dashboard.rs` that returns P50 and P95 per stage for the
last 24h and last 7d:

```json
{
  "stages": [
    { "stage": "context_build", "p50_ms": 45, "p95_ms": 210, "window": "24h" },
    { "stage": "api_call",      "p50_ms": 6200, "p95_ms": 9800, "window": "24h" }
  ]
}
```

Add a simple latency chart to the dashboard SPA (`dashboard/src/`) — a horizontal bar chart
per stage showing P50 (filled) and P95 (outlined) bars. Use the existing charting approach
already present in the SPA (or a minimal inline SVG if no chart library is available).

## Scope
- **IN**: Pipeline profiling, streaming delivery, persistent-subprocess diagnosis, parallel
  context build, connection pool audit, latency_spans table, dashboard chart
- **OUT**: Per-request cost attribution (separate spec), multi-model routing (haiku for
  triage), WebSocket push for live latency updates, alerting on latency regressions

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/worker.rs` | Emit receive/api_call/tool_loop/delivery spans; parallel context build via spawn_blocking |
| `crates/nv-daemon/src/claude.rs` | Switch cold-start to stream-json output; fix/re-enable persistent path; stream partial text to caller |
| `crates/nv-daemon/src/orchestrator.rs` | Persist StageComplete events to latency_spans; measure receive latency |
| `crates/nv-daemon/src/channels/telegram/client.rs` | Add edit_message_text, delete_message; audit connection pool |
| `crates/nv-daemon/src/messages.rs` | Add latency_spans migration + log_latency_span + latency_p95 |
| `crates/nv-daemon/src/dashboard.rs` | Add /api/latency endpoint |
| `dashboard/src/` | Add latency chart component |

## Risks

| Risk | Mitigation |
|------|-----------|
| Telegram rate limit on edits (1/s) | 1.5s edit interval with jitter; final edit always sent |
| Streaming cold-start diverges from JSON parsing logic | New `parse_stream_response()` fn isolated from existing `parse_json_response()`; both paths preserved |
| Persistent subprocess fix requires CC version bump | Pin CC version in Cargo.toml / Nix flake; document fallback |
| spawn_blocking for context build increases thread pool pressure | Pool size capped by Tokio default (512); context build is short (<100ms); acceptable |
| latency_spans table grows unbounded | Add `DELETE FROM latency_spans WHERE recorded_at < datetime('now', '-30 days')` to the nightly cron |
