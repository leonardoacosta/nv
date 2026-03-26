# Proposal: Telegram Bot Presence

## Change ID
`telegram-bot-presence`

## Beads Epic
`nv-r25l`

## Roadmap
Phase: 1 — Telegram UX | Wave: 2

## Summary

Formalize and complete Telegram bot typing indicator coverage. The `sendChatAction("typing")`
infrastructure is partially in place — the worker sends one indicator at startup, and the
orchestrator refreshes it every 5s via `TYPING_REFRESH`. This spec audits what is missing,
fills the gaps, and documents the final contract. Also investigates whether Telegram bots support
online/offline presence status (they do not — confirmed by API).

## Context

- Primary files: `crates/nv-daemon/src/channels/telegram/client.rs`,
  `crates/nv-daemon/src/worker.rs`, `crates/nv-daemon/src/orchestrator.rs`
- `TelegramClient::send_chat_action` exists with 5s throttle + 429 backoff
- `Worker::run` calls `send_chat_action` once at the start of processing
- `Orchestrator::check_inactivity` refreshes typing every `TYPING_REFRESH` (5s) while any
  worker stage is active
- `WorkerEvent::StageStarted` carries `telegram_chat_id` so the orchestrator maps
  `worker_id → chat_id` for per-worker refresh

## Motivation

The current implementation has three gaps:

1. **Immediate indicator missing** — the typing indicator is sent inside `Worker::run`, which
   runs asynchronously after the trigger is dispatched. On a busy system, the worker may not
   start for hundreds of milliseconds. The user sees no feedback until the worker actually
   begins. The indicator should be sent in `process_trigger_batch` immediately on message
   receipt, before dispatch.

2. **Stop-on-delivery missing** — typing is refreshed every 5s while workers are active, but
   there is no explicit call to stop the typing appearance when the response is sent. The
   indicator naturally expires after ~5s, so this is cosmetically acceptable, but an explicit
   stop via `sendChatAction("cancel")` (or just ceasing the refresh) should be documented and
   confirmed as the intended behavior.

3. **Presence status investigation** — the roadmap spec asks to check if bots can show
   online/offline status. Investigation result: **Telegram Bot API does not expose bot presence
   status**. `sendChatAction` is the only per-message engagement signal available to bots.
   Regular user presence (online/offline/last seen) is user-controlled and not accessible to
   bots. No implementation change needed for this.

## Design

### Req-1: Immediate Typing on Message Receipt

In `Orchestrator::process_trigger_batch`, after classifying a trigger as a `Query` or `Command`
(i.e., any trigger that will be dispatched to a worker), send the typing indicator before
calling `WorkerPool::dispatch`. This eliminates the gap between message receipt and worker start.

```rust
// In process_trigger_batch, before dispatch:
if let (Some(tg), Some(chat_id)) = (&self.telegram_client, self.telegram_chat_id) {
    // Fire immediately — user sees typing the instant we receive their message
    tg.send_chat_action(chat_id, "typing").await;
}
// Then dispatch...
self.worker_pool.dispatch(task).await;
```

The worker's existing startup call remains in place as a belt-and-suspenders check (handles
cases where the orchestrator-level send is throttled by the 5s window due to a rapid second
message).

### Req-2: Confirm Stop-on-Delivery Behavior

`sendChatAction` does not have an explicit "stop typing" call in the Bot API. The indicator
automatically expires after ~5s. When a response is delivered:

1. The worker completes and sends `WorkerEvent::Complete`
2. The orchestrator removes the worker from `worker_stage_started` and `worker_chat_id`
3. The next `check_inactivity` tick finds no active workers and sends no refresh
4. The typing indicator expires naturally within 5s

This is the correct behavior. No explicit cancellation is needed. Add a code comment in
`check_inactivity` documenting this contract so future engineers do not attempt to add a
spurious cancel call.

### Req-3: Document Presence Limitations

Add a doc comment to `TelegramClient::send_chat_action` noting that this is the only presence
signal available to bots and that Telegram does not expose bot online/offline status.

### Typing Indicator Coverage Map (final state after this spec)

| Scenario | When sent | Mechanism |
|---|---|---|
| Text message received | Immediately on receipt | `process_trigger_batch` (new) |
| Worker processing starts | At worker run start | `Worker::run` (existing) |
| Processing >5s | Every 5s refresh | `check_inactivity` (existing) |
| Tool called | On each `ToolCalled` event | `handle_worker_event` (existing) |
| Response delivered | Typing expires naturally | No explicit cancel |
| Voice/photo/audio download | During download | `handle_*_message` in mod.rs (existing) |

## Scope

- **IN**: immediate typing call in `process_trigger_batch`, doc comment additions,
  code comment in `check_inactivity` on stop-on-delivery contract
- **OUT**: streaming response delivery (separate spec), presence status (not supported by API)

## Impact

| File | Change |
|------|--------|
| `crates/nv-daemon/src/orchestrator.rs` | Add `send_chat_action` call in `process_trigger_batch` before worker dispatch; add doc comment in `check_inactivity` |
| `crates/nv-daemon/src/channels/telegram/client.rs` | Add doc comment to `send_chat_action` noting presence limitations |

## Risks

| Risk | Mitigation |
|------|-----------|
| Orchestrator-level call is throttled when worker also calls at startup | Throttle is per-chat, 5s window — both calls within the same turn only incur one HTTP call. Correct behavior. |
| 429 rate-limit on rapid message bursts | Existing backoff in `ThrottleState` handles this. Orchestrator-level call is also gated by the same throttle. |
| Typing expires before long Claude turn completes | `check_inactivity` refreshes every 5s — already handles this. |
