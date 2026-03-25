# Tasks: investigate-300s-timeout

## Dependencies

None (standalone investigation + fix spec)

## Tasks

### Phase 1: Tracing

- [x] Add `is_edit_reply: bool` field to `WorkerTask` in `crates/nv-daemon/src/worker.rs` —
  defaults to `false`; set to `true` in `process_trigger_batch` when `editing_action_id` is
  `Some` at dispatch time [owner:api-engineer]
- [x] Add `timeout_reason` string field to the `tracing::warn!` span in the primary timeout
  branch (`worker.rs` line ~319) — value `"edit_wait"` if `task.is_edit_reply`, else
  `"active_work"` [owner:api-engineer]
- [x] Add `timeout_reason` field to the queued-worker timeout branch (`worker.rs` line ~404)
  with the same logic [owner:api-engineer]
- [x] Add `stage_elapsed_ms = task_start.elapsed().as_millis()` to both timeout warn spans so
  actual elapsed time is visible in logs [owner:api-engineer]
- [x] Add `is_edit_reply` to the `"worker started"` tracing span so the log entry identifies
  edit-reply workers from the start [owner:api-engineer]

### Phase 2: Edit flow — consume `editing_action_id`

- [x] In `Orchestrator::process_trigger_batch` (`orchestrator.rs`), add a block before worker
  dispatch that calls `self.editing_action_id.take()` — if `Some(action_id)`, set
  `task.editing_action_id = Some(action_id)` on the task being dispatched [owner:api-engineer]
- [x] Add `editing_action_id: Option<Uuid>` field to `WorkerTask` in `worker.rs` [owner:api-engineer]
- [x] In `Worker::run` (`worker.rs`), at context-build time, check `task.editing_action_id`:
  if `Some(action_id)`, load the `PendingAction` from `deps.state.find_pending_action` and
  prepend a system-level context string to the Claude prompt (e.g., "You are editing a pending
  action. Original: {description}. User's edit instruction follows.") [owner:api-engineer]
- [x] After Claude's response in the edit-aware session, update the `PendingAction` payload
  with the revised description from Claude's output, then re-send the confirmation keyboard via
  Telegram using the existing `InlineKeyboard::confirm_action` pattern [owner:api-engineer]
- [x] Clear `editing_action_id` in `Orchestrator` after dispatch (`.take()` handles this
  automatically; verify it is not re-set on the same batch) [owner:api-engineer]

### Phase 3: Extended timeout for Edit-reply tasks

- [x] In `WorkerPool::spawn_worker` (`worker.rs`), compute `effective_timeout_secs`:
  if `task.is_edit_reply` use `worker_timeout_secs * 2`, else `worker_timeout_secs`; use
  `effective_timeout_secs` for both the primary and queued-worker `tokio::time::timeout` calls
  [owner:api-engineer]
- [x] Add `tracing::info!(effective_timeout_secs, is_edit_reply, "worker timeout configured")`
  at dispatch so the extended timeout is logged [owner:api-engineer]
- [x] Change the Telegram timeout message for edit-reply tasks to:
  "Edit timed out waiting for your reply. The pending action is still queued — tap Edit again
  to retry." (branch on `task.is_edit_reply`) [owner:api-engineer]

### Phase 4: Fix `chat_id` routing in timeout notifications

- [x] In the primary timeout branch (`worker.rs` line ~329), remove `let _ = chat_id;` and
  add `chat_id: task_tg_chat_id` to the `tracing::warn!` span so the chat ID is logged
  [owner:api-engineer]
- [x] In the same branch, pass `task_tg_chat_id` into the `OutboundMessage` routing — if
  `TelegramChannel` exposes a `send_message_to_chat(chat_id, msg)` method use it; otherwise
  log `task_tg_chat_id` and document the limitation [owner:api-engineer]
- [x] Apply the same fix to the queued-worker timeout branch (`worker.rs` line ~413)
  [owner:api-engineer]

### Phase 5: "Still working" feedback

- [x] In `WorkerPool::spawn_worker`, before the `tokio::time::timeout` wrapper, add a
  `tokio::select!` that races `Worker::run` against a `tokio::time::sleep(Duration::from_secs
  (effective_timeout_secs * 4 / 5))` warning future — if the sleep fires first, send a
  Telegram message "Still working... ({elapsed}s elapsed, up to {timeout}s total)" and then
  continue waiting for the worker [owner:api-engineer]
- [x] Ensure the warning future is cancelled if `Worker::run` completes before it fires —
  use `tokio::select!` with a `break`/`return` path on worker completion [owner:api-engineer]
- [x] Do not send the "still working" message for `is_edit_reply` tasks — those are waiting on
  Leo, not on Claude [owner:api-engineer]

### Unit Tests

- [x] Add unit test: `WorkerTask` with `is_edit_reply = true` sets `timeout_reason =
  "edit_wait"` in the emitted `WorkerEvent::Error` log (mock the event channel)
  [owner:api-engineer]
- [x] Add unit test: `editing_action_id.take()` in `process_trigger_batch` correctly consumes
  the ID so subsequent triggers do not inherit it [owner:api-engineer]

### Verify

- [x] `cargo build` passes for all workspace members [owner:api-engineer]
- [x] `cargo test -p nv-daemon` passes [owner:api-engineer]
- [x] `cargo clippy` passes with no warnings [owner:api-engineer]
- [ ] Manual gate: trigger Edit flow, wait >600s without replying, verify Telegram shows
  "Edit timed out" message. Trigger again, reply within timeout, verify pending action is
  updated and confirmation keyboard re-sent with revised description. Trigger long-running
  query, verify "Still working..." message appears at ~240s (for 300s timeout). Check logs
  for `timeout_reason`, `stage_elapsed_ms`, and `is_edit_reply` fields. [user]
