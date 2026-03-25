# investigate-300s-timeout

## Summary

Trace the exact source of "Request timed out after 300s" in the Edit flow, add structured
tracing so the timeout location is observable, send a Telegram notification when timeout occurs
(it already fires but with only a generic message), and fix the Edit flow so a delayed user
reply does not race against the worker timeout.

## Motivation

When Leo taps "Edit" on a pending action confirmation, the orchestrator sets
`editing_action_id` and sends "What would you like to change?" via Telegram. The user's reply
arrives as a normal message trigger and is dispatched to a new `Worker::run` call. That worker
is wrapped in `tokio::time::timeout(Duration::from_secs(worker_timeout_secs))` (default 300s)
in `WorkerPool::spawn_worker`.

The timeout message "Request timed out after 300s" is already being sent to Telegram (lines
334 and 418 of `worker.rs`). However, there are two compounding problems:

1. **The timeout source is unobservable.** The `tracing::warn!` emitted at timeout does not
   distinguish between a Claude reasoning timeout (worker was actively thinking for 300s) and
   an Edit-flow wait timeout (worker was idle, waiting for Leo to type). The log fields are
   identical in both cases.

2. **The Edit flow is broken by design.** `editing_action_id` is set in
   `Orchestrator::handle_callbacks` (line 813) but is never read anywhere in the codebase.
   The orchestrator has no logic to intercept the follow-up message and route it as an edit
   instruction. The next inbound message is dispatched as an ordinary worker task, which starts
   a full Claude session. If Leo takes longer than 300s to reply, that worker times out with
   the generic message above — giving no indication that it was waiting for an edit reply.

3. **Silent failure path.** Even when the timeout fires correctly and the Telegram message is
   sent, the `chat_id` suppression bug on line 342 (`let _ = chat_id; // suppress unused
   warning`) means the `OutboundMessage` routing does not carry the originating chat ID — the
   `content` field embeds the `worker_timeout_secs` value but the message is routed via the
   default channel chat ID, not the task's chat ID. This is a latent routing bug exposed during
   Edit flow where `task_tg_chat_id` may differ.

## Design

### Phase 1: Tracing — identify the timeout source

Add a `timeout_reason` field to the worker span that distinguishes the two timeout scenarios:

- `"edit_wait"` — the task was dispatched during an active Edit flow (orchestrator had
  `editing_action_id` set when this task was created)
- `"active_work"` — the task ran out of time while Claude was actively reasoning or executing
  tools

To propagate this context, add an `is_edit_reply: bool` field to `WorkerTask`. Set it in
`process_trigger_batch` when `editing_action_id.is_some()` and the trigger is a `Message`.
Log it at both the `worker started` span and the timeout warn span.

Also add a monotonic `stage_elapsed_ms` field to the timeout warn — computed from
`task_start.elapsed()` — so the log shows how much actual work time elapsed vs. the full
300s budget.

### Phase 2: Edit flow — consume `editing_action_id`

The orchestrator currently stores `editing_action_id` but never uses it. Fix this:

In `process_trigger_batch`, before dispatching to the worker pool, check
`self.editing_action_id`:

```rust
if let Some(action_id) = self.editing_action_id.take() {
    // Route as edit instruction: attach action_id to the task
    // so the worker can load the PendingAction and apply the edit
    task.editing_action_id = Some(action_id);
}
```

In `Worker::run`, if `task.editing_action_id.is_some()`, load the `PendingAction` from state,
prepend the original description to the Claude context (so Claude knows what it is editing),
and after Claude responds, update the `PendingAction` payload and re-send the confirmation
keyboard via Telegram.

This replaces the current behavior (start a full Claude session that has no context about the
pending action being edited) with an edit-aware session.

### Phase 3: Extend timeout for Edit-reply tasks

For tasks where `is_edit_reply = true`, double the timeout: `worker_timeout_secs * 2`. This
gives Leo more time to type a reply without hitting the wall. Add a `tracing::info!` at
dispatch time noting the extended timeout.

If the extended timeout fires, the Telegram notification should be more specific:
"Edit timed out waiting for your reply. The pending action is still queued — tap Edit again
to retry."

### Phase 4: Fix the `chat_id` routing bug in timeout notifications

Lines 329–348 and 413–434 of `worker.rs` construct `OutboundMessage` but drop `chat_id`
with `let _ = chat_id`. The `OutboundMessage` struct has no `chat_id` field — routing is done
by the channel using its default `chat_id`. For most tasks this is fine. For tasks that
originated from a non-default chat (or queued tasks), the message may route incorrectly.

Fix: pass `task_tg_chat_id` through to the channel by using
`channel.send_message_to(chat_id, msg)` if available, or by encoding `chat_id` into the
`OutboundMessage` struct. At minimum, log `task_tg_chat_id` in the timeout warn so the
routing is auditable.

### Phase 5: "Still working" feedback before timeout

At 80% of the timeout budget (240s for a 300s timeout), send a Telegram message:
"Still working... (240s elapsed, may take up to 300s total)."

Implement via a `tokio::time::sleep` future raced in a `tokio::select!` alongside `Worker::run`
inside `spawn_worker`. If the worker completes before the warning fires, cancel it. If the
warning fires first, send the message and let the worker continue until the hard timeout.

## Files

| File | Change |
|------|--------|
| `crates/nv-daemon/src/worker.rs` | Add `is_edit_reply`/`editing_action_id` to `WorkerTask`, add `timeout_reason` tracing, fix `chat_id` routing, add 80% warning future |
| `crates/nv-daemon/src/orchestrator.rs` | Consume `editing_action_id` before dispatch, set `is_edit_reply` on task, extend timeout for edit-reply tasks |
| `crates/nv-daemon/src/callbacks.rs` | No structural change; `handle_edit` already returns the UUID correctly |

## Out of Scope

- Re-queue on timeout (complex state machine, separate spec if needed)
- Configuring `worker_timeout_secs` at runtime via Telegram command
- Timeout for tool-level calls (already handled by per-tool timeout added in `fix-chat-bugs`)

## Risks

| Risk | Mitigation |
|------|-----------|
| Doubling timeout for edit-reply tasks may hold a worker slot for 600s | Edit-reply workers are rare; pool slot is cheap. Accept the tradeoff. |
| Prepending PendingAction description to Claude context may confuse the model | Use a system-level prefix, not a user message, so Claude treats it as context not chat history |
| 80% warning may alarm Leo if the task completes at 85% | Only send if worker has not yet completed; cancel the warning future on success |

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` passes
- `cargo clippy` passes with no warnings
- Manual gate: trigger an Edit flow, wait >300s without replying, verify Telegram shows
  "Edit timed out" message (not generic timeout). Trigger again, reply within timeout, verify
  the pending action is updated and re-confirmed with new keyboard. Trigger a long-running
  query, verify "Still working..." fires at ~240s.
