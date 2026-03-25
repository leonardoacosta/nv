# Proposal: Improve Typing Indicators

## Change ID
`improve-typing-indicators`

## Summary

Replace the single fire-and-forget `sendChatAction("typing")` with a throttled refresh loop that
re-sends on each `ToolCalled` event and periodically from the orchestrator's inactivity tick.
Adds per-chat-id throttling (max 1 call per 5 seconds) and 429 backoff to eliminate the
~18-30s activity gap during cold-starts without re-introducing rate limit failures.

## Context
- Extends: `crates/nv-daemon/src/worker.rs` (typing at worker start)
- Extends: `crates/nv-daemon/src/orchestrator.rs` (typing refresh on ToolCalled, check_inactivity)
- Extends: `crates/nv-daemon/src/channels/telegram/client.rs` (throttle-aware send_chat_action)
- Related: `add-tool-emoji-indicators` (the emoji thinking-message edits are unaffected — this only touches the chat-header typing indicator)

## Motivation

Telegram's "typing..." indicator in the chat header expires after ~5 seconds. Nova's cold-start
path (context build + first Claude turn) takes 18-30 seconds. The current code sends one
`sendChatAction("typing")` at worker start, which covers only the first 5 seconds; the user sees
nothing for the remainder of the wait.

A previous attempt to fix this with a background loop hit Telegram 429 rate limits because it
sent one call per worker tick without any throttling. The fix needs:

1. Re-send on meaningful progress events (`ToolCalled`) so typing stays visible during tool
   execution phases.
2. Per-chat-id throttle so multiple workers or rapid events don't burst calls.
3. Respect 429 `retry-after` headers — back off automatically when Telegram asks.

## Requirements

### Req-1: Throttle-Aware `send_chat_action` in TelegramClient

Change `send_chat_action` from a plain fire-and-forget call to a throttle-aware method that:

- Accepts `action: &str` as before.
- Returns `bool` — `true` if the call was sent, `false` if suppressed by the throttle.
- Internally tracks `last_sent: Instant` on a `HashMap<i64, Instant>` keyed by `chat_id`,
  suppressing calls made within 5 seconds of the previous call for the same `chat_id`.
- On a 429 response, reads the `retry_after` field from the Telegram error JSON and records a
  `backoff_until: Instant` per `chat_id`. Suppresses all calls for that `chat_id` until the
  backoff expires.

Because `TelegramClient` is `Clone` and shared across callers, the throttle state must be
wrapped in `Arc<Mutex<ThrottleState>>` so all clones share the same state.

`ThrottleState` holds:
```rust
struct ThrottleState {
    last_sent: HashMap<i64, Instant>,
    backoff_until: HashMap<i64, Instant>,
}
```

### Req-2: Send Typing on Worker Start (existing, no change)

Worker already calls `tg.send_chat_action(chat_id, "typing").await` at start. This call now goes
through the throttled method — no other change needed in `worker.rs`.

Remove the unused `typing_cancel` watch channel that was scaffolded for a loop that was never
implemented (lines 479-480 in worker.rs).

### Req-3: Re-Send Typing on ToolCalled in Orchestrator

In `handle_worker_event`, when `WorkerEvent::ToolCalled` fires, call `send_chat_action` on the
Telegram client for the worker's associated `chat_id` (if any). The throttle in Req-1 ensures
this does not burst even if tools are called in rapid succession.

The `chat_id` for the active worker is already tracked in the orchestrator via
`worker_stage_started` keyed by `worker_id`. Extend the map value from
`(String, Instant)` to `(String, Instant, Option<i64>)` to carry the chat_id, populated from
`WorkerTask::telegram_chat_id` when `StageStarted` fires.

Alternatively — simpler — add a separate `worker_chat_id: HashMap<Uuid, i64>` map that is
populated on `StageStarted` (from the existing `task.telegram_chat_id`) and cleared on
`WorkerEvent::Complete`/`Error`.

### Req-4: Orchestrator check_inactivity Continues Refreshing

`check_inactivity` already calls `send_chat_action("typing")` every `TYPING_REFRESH` (5s) for
all active workers. This continues unchanged — the throttle in Req-1 will suppress the call if
one was already sent recently via Req-3, avoiding double-sends.

No structural change to `check_inactivity` is needed. The typed `chat_id` it uses
(`tg_channel.chat_id`) is the channel-level default — correct for single-chat deployments.

### Req-5: Optional Phase Signal via Action Variant (deferred)

Different Telegram chat actions (`typing`, `upload_document`) could signal different phases
(thinking vs tool execution). This is deferred — the UX value is marginal and the throttle
complexity increases if we switch actions mid-session. Default to `"typing"` for all phases.

## Scope
- **IN**: `ThrottleState` in TelegramClient, throttled `send_chat_action`, per-worker chat_id
  tracking in orchestrator, `ToolCalled` -> typing refresh, remove dead `typing_cancel` watch
- **OUT**: action-variant phase signaling (Req-5 deferred), per-tool timeout changes, changes to
  the emoji thinking-message edit logic

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/channels/telegram/client.rs` | Add `ThrottleState`, wrap in `Arc<Mutex<...>>`, update `send_chat_action` signature and logic |
| `crates/nv-daemon/src/orchestrator.rs` | Add `worker_chat_id` map, populate on `StageStarted`, call `send_chat_action` on `ToolCalled` |
| `crates/nv-daemon/src/worker.rs` | Remove dead `typing_cancel` watch channel (2 lines) |

## Risks
| Risk | Mitigation |
|------|-----------|
| `Arc<Mutex<>>` contention on ThrottleState | All callers are async; lock is held only for a HashMap lookup + insert (nanoseconds). No contention in practice. |
| 429 retry_after field absent or malformed | Default to 30s backoff if field is missing or unparseable. |
| Multiple workers for different chat_ids share one throttle map | The map is keyed by `chat_id` — each chat gets its own independent throttle window. Correct. |
| Orchestrator `check_inactivity` uses channel-level chat_id, not per-worker | For Nova's single-chat deployment this is identical. If multi-chat is ever added, the per-worker map (Req-3) handles it correctly. |
