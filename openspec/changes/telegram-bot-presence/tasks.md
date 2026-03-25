# Tasks: telegram-bot-presence

## Beads Epic
`nv-r25l`

## Engineer Notes

Most of the typing indicator infrastructure already exists. This spec makes three targeted
changes: (1) move the first typing call upstream to `process_trigger_batch` for immediate
feedback, (2) add a doc comment to `send_chat_action` documenting presence limitations,
(3) add a code comment to `check_inactivity` documenting the stop-on-delivery contract.

No new dependencies, no new types, no behavior changes beyond the earlier indicator.

---

## Tasks

### Task 1 — Immediate typing indicator in orchestrator
**File:** `crates/nv-daemon/src/orchestrator.rs`
**Agent:** `api-engineer`

In `process_trigger_batch`, before the `WorkerPool::dispatch` call, add a
`send_chat_action(chat_id, "typing")` call. The call must only fire for triggers that result
in a worker dispatch (i.e., skip `TriggerClass::Chat`, `Callback`, `NexusEvent`,
`BotCommand`, and `Digest` inline paths — they either respond immediately or are not
user-facing interactive turns).

The correct insertion point is after the `match primary_class` block that handles inline cases
returns early, and before `self.worker_pool.dispatch(task).await`. The chat_id to use is
`self.telegram_chat_id` (the daemon's configured chat — same as the worker uses).

```rust
// Before dispatch — send typing indicator immediately so the user sees feedback
// while the worker starts up (worker also sends one at startup as belt-and-suspenders)
if let (Some(tg), Some(chat_id)) = (&self.telegram_client, self.telegram_chat_id) {
    tg.send_chat_action(chat_id, "typing").await;
}
```

Verification: `cargo build -p nv-daemon` passes. Unit test: add a test to
`orchestrator.rs` tests that verifies the typing call site is reachable (mock or doc
comment level — the live HTTP call is already covered by `ThrottleState` unit tests).

---

### Task 2 — Doc comment on `send_chat_action` (presence limitations)
**File:** `crates/nv-daemon/src/channels/telegram/client.rs`
**Agent:** `api-engineer`

Extend the existing doc comment on `TelegramClient::send_chat_action` to document:
- This is the only engagement signal available to bots
- Telegram does not expose bot online/offline presence status
- `sendChatAction` is the closest equivalent and is what this method wraps

The addition should be a short paragraph appended to the existing doc comment, not a
replacement.

---

### Task 3 — Code comment in `check_inactivity` (stop-on-delivery contract)
**File:** `crates/nv-daemon/src/orchestrator.rs`
**Agent:** `api-engineer`

In `Orchestrator::check_inactivity`, add a code comment above the typing refresh block
explaining the stop-on-delivery contract:

```rust
// Stop-on-delivery: Telegram "typing..." expires automatically after ~5s.
// When a worker completes, it's removed from worker_stage_started and worker_chat_id.
// The next tick of this function finds no active workers and sends no refresh,
// so the indicator expires within 5s of response delivery.
// There is no explicit "stop typing" API call in the Telegram Bot API.
```

---

## Verification

- [ ] `cargo build -p nv-daemon` passes after all three tasks
- [ ] No new clippy warnings introduced
- [ ] The typing indicator is visually present from the moment a text message is received
      (manual verification: send a message, observe typing indicator before response arrives)
