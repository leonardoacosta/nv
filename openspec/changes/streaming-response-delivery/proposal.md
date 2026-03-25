# Proposal: Streaming Response Delivery

## Change ID
`streaming-response-delivery`

## Summary

Add progressive streaming delivery to the Telegram channel: send a `"..."` placeholder message
immediately when Claude begins processing, then edit it with accumulated text every 1.5s or on
50+ character deltas as the stream-json events arrive. On turn completion, send the final edit
with the full response text. Handle tool-only turns (no text) by deleting the placeholder. Add
`parse_stream_response()` to `claude.rs` for a streaming-aware variant of the existing
`read_stream_response_from_lines`.

Constants already declared in `worker.rs`:
- `STREAMING_EDIT_INTERVAL_MS = 1500`
- `STREAMING_EDIT_MIN_DELTA_CHARS = 50`

## Context

- Extends: `crates/nv-daemon/src/worker.rs` (Worker::run, response delivery path)
- Extends: `crates/nv-daemon/src/claude.rs` (add streaming callback variant)
- Extends: `crates/nv-daemon/src/channels/telegram/client.rs` (existing `edit_message_text`, `send_thinking`, `delete_message` are already implemented)
- Carry-forward from: `fix-chat-bugs` Req-2 (nv-vvq roadmap item, Wave 1 Phase 1 Telegram UX)
- Depends on: none — `edit_message_text`, `send_thinking`, and `delete_message` are already present in `TelegramClient`

## Motivation

Nova currently sends a `sendChatAction` typing indicator at the start of each turn and delivers
the full response only after the entire tool loop and Claude generation complete. For non-trivial
turns (tool calls + long answers), this means 5-15 seconds of silent "typing..." with no
indication of progress. The user cannot distinguish a fast response from a stalled one.

Streaming delivery solves this by immediately confirming Nova received the request (placeholder
message), then progressively revealing the answer as Claude generates it. The UX matches what
users expect from modern AI chat interfaces.

The infrastructure is already in place: `TelegramClient` has `send_thinking()`,
`edit_message_text()`, and `delete_message()`. The stream-json protocol already emits
`{"type":"assistant","subtype":"text","text":"..."}` events incrementally. This spec wires
those two together.

## Requirements

### Req-1: Placeholder Message on Turn Start

When a Telegram message triggers a worker turn, immediately send a `"..."` placeholder via
`send_message` (using `send_thinking()`) before invoking Claude. Store the returned `message_id`
(`placeholder_msg_id: Option<i64>`) on the worker scope for the duration of the turn.

This replaces the existing `send_chat_action("typing")` call at turn start for Telegram triggers.
The `sendChatAction` path remains for non-Telegram triggers (CLI, cron, etc.) since those do not
benefit from a placeholder edit loop.

### Req-2: Streaming Edit Loop in claude.rs

Add a new function `read_stream_response_streaming` to `claude.rs` with the following signature:

```rust
pub async fn read_stream_response_streaming<R, F>(
    reader: &mut R,
    on_text_delta: F,
) -> Result<ApiResponse>
where
    R: tokio::io::AsyncBufRead + Unpin,
    F: FnMut(&str),
```

This function is identical to `read_stream_response_from_lines` in structure but calls
`on_text_delta(delta)` for every `{"type":"assistant","subtype":"text","text":"..."}` event
before accumulating it into `current_text`. The `on_text_delta` callback receives the incremental
text chunk (not the full accumulated buffer).

Expose `read_stream_response_streaming` as `pub(crate)` — it is called from `worker.rs` via the
`PersistentSession` path.

### Req-3: Streaming Edit Throttle in worker.rs

In `Worker::run`, after sending the placeholder (Req-1), pass an `on_text_delta` closure into
`read_stream_response_streaming` (Req-2). The closure accumulates a `stream_buffer: String` and
tracks `last_edit_at: Instant` and `chars_since_last_edit: usize`. It fires a Telegram edit
when either condition is met:

- `last_edit_at.elapsed() >= Duration::from_millis(STREAMING_EDIT_INTERVAL_MS)`, OR
- `chars_since_last_edit >= STREAMING_EDIT_MIN_DELTA_CHARS`

When an edit fires, call `tg_client.edit_message_text(chat_id, placeholder_msg_id, &stream_buffer)`
and reset both `last_edit_at` and `chars_since_last_edit`.

The closure must capture `tg_client` and `placeholder_msg_id` by reference. Because the closure
is called from within the async `read_stream_response_streaming` loop but is itself sync, Telegram
edits are dispatched with `tokio::spawn` (fire-and-forget). Rate-limit: at most one in-flight
edit spawn per Telegram rate window (the 1500ms interval provides adequate headroom against
Telegram's 1 edit/second/chat limit).

### Req-4: Final Response Edit

After `read_stream_response_streaming` returns (stream complete), if a placeholder message was
sent and the response contains text content:

1. Edit the placeholder to the final `response_text` (after `extract_text` + `extract_summary`
   processing) using `edit_message_text`.
2. Do NOT send an additional `send_message` call for the response body — the final edit IS the
   response delivery.

This replaces the existing `channel.send_message` call for Telegram triggers when a placeholder
is active.

### Req-5: Tool-Only Turn Cleanup

If the final `response_text` is empty (tool-only turn — Claude called tools but produced no
text response to deliver), delete the placeholder via `tg_client.delete_message(chat_id,
placeholder_msg_id)` instead of sending an empty or unchanged message.

### Req-6: Non-Streaming Fallback

The cold-start path (`send_messages_cold_start_with_image`) does not support incremental text
delivery (the subprocess only returns after completion). For cold-start turns:

- Do not send a placeholder message (keep existing `send_chat_action("typing")` behavior).
- Deliver the response via `channel.send_message` as today.

The streaming path applies exclusively to turns routed through `PersistentSession.send_turn`.
Since `fallback_only: true` is currently the default (persistent mode disabled), streaming
delivery will silently degrade to the current behavior until persistent mode is re-enabled.
This is correct — the spec should not change when streaming activates, only how it works
when active.

### Req-7: Keyboard Preservation

When the final response includes an `InlineKeyboard` (e.g. action confirmation), the final
edit must include the keyboard via `edit_message` (not `edit_message_text`). The keyboard
is only attached on the final edit, never on intermediate streaming edits.

## Scope

- **IN**: placeholder send, stream edit loop, final edit delivery, tool-only delete, keyboard on final edit
- **OUT**: streaming on cold-start path, streaming on non-Telegram channels, voice reply streaming, chunked multi-message streaming (messages > 4096 chars keep existing chunking behavior)

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/claude.rs` | Add `read_stream_response_streaming` (new pub(crate) fn) |
| `crates/nv-daemon/src/worker.rs` | Placeholder send on turn start; streaming edit closure; final edit replaces send_message for Telegram; tool-only delete |
| `crates/nv-daemon/src/channels/telegram/client.rs` | No changes — `edit_message_text`, `send_thinking`, `delete_message` already implemented |

## Risks

| Risk | Mitigation |
|------|-----------|
| Telegram 429 on edit floods | 1500ms interval provides 50% headroom over 1 edit/s limit; existing `send_chat_action` throttle is separate |
| Placeholder lingers if worker panics | `tokio::spawn` task timeout at worker level already handles hung workers — placeholder will remain visible but is not a correctness issue |
| Streaming edits during tool calls (no text yet) | Placeholder stays as `"..."` during tool-only phases; first text delta triggers first edit |
| Final edit race with last streaming edit | Final edit always overwrites — last write wins; no consistency issue |
| persistent mode currently disabled | Streaming silently degrades to current behavior on cold-start path; unblocks future re-enable |
