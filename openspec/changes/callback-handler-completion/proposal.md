# callback-handler-completion

## Summary

Complete the Telegram inline keyboard callback handling for obligation actions. Four new callback
prefixes are wired into the orchestrator's callback router: `ob_edit:{id}` (modify obligation
text), `ob_cancel:{id}` (dismiss obligation), `ob_expiry:{id}` (extend deadline), and
`ob_snooze:{id}:{offset}` (postpone reminder by 1h/4h/tomorrow). Each handler updates
`obligation_store` and edits the original Telegram message to reflect new state. The obligation
keyboard is extended with the new buttons. Toast labels and unit tests are added throughout.

## Motivation

The obligation watcher sends Telegram alerts with an inline keyboard when obligations are detected.
Currently only three buttons are functional — "Handle" (`ob_handle:`), "Delegate to Nova"
(`ob_delegate:`), and "Dismiss" (`ob_dismiss:`). The keyboard defined in `obligation_keyboard()` in
`orchestrator.rs` only renders those three buttons.

Four additional obligation management actions described in the roadmap have no callback handlers:

- **Edit**: Let the user change the obligation text in-place (e.g. fix a mis-detected action).
- **Cancel**: Dismiss the obligation and update the message to show it was cancelled.
- **Extend expiry**: Push the reminder due-time forward without creating a full reminder entry.
- **Snooze**: Create a new reminder for the obligation at a configurable offset (1h, 4h, tomorrow).

Without these handlers, the buttons either do not exist or are dead. This spec closes that gap.

## Current State

`callbacks.rs` contains `handle_approve_with_backend`, `handle_edit`, `handle_cancel`, and
`check_expired_actions` — these all operate on `PendingAction` / `State`, not on obligations.

The obligation-specific callbacks `ob_handle:`, `ob_delegate:`, and `ob_dismiss:` are routed in
`orchestrator.rs` via `handle_obligation_callback()`, which calls
`obligation_store.update_status()` or `obligation_store.update_status_and_owner()` and edits the
original Telegram message. That helper is already implemented and tested.

The obligation keyboard (`obligation_keyboard()`) currently produces one row of three buttons. No
`ob_edit:`, `ob_cancel:`, `ob_expiry:`, or `ob_snooze:` buttons exist.

The `ObligationStore` has `update_status()` and `update_status_and_owner()` but no method for
updating `detected_action` text or for scheduling a snooze reminder.

## Design

### Callback Prefix Scheme

| Button text | Callback data | Effect |
|---|---|---|
| Edit | `ob_edit:{obligation_id}` | Prompt user for new text, then update `detected_action` |
| Cancel | `ob_cancel:{obligation_id}` | Set status=Dismissed, edit message |
| Extend (+24h) | `ob_expiry:{obligation_id}` | No-op against obligation store; edits message to acknowledge |
| Snooze 1h | `ob_snooze:{obligation_id}:1h` | Create reminder via `ReminderStore` in 1h |
| Snooze 4h | `ob_snooze:{obligation_id}:4h` | Create reminder via `ReminderStore` in 4h |
| Snooze tomorrow | `ob_snooze:{obligation_id}:tomorrow` | Create reminder via `ReminderStore` for tomorrow 9am |

`ob_cancel:` is distinct from `ob_dismiss:` in name only — both set `status=Dismissed`. The
keyboard will expose `ob_cancel:` as a user-visible "Cancel" button alongside the existing three.
`ob_dismiss:` is retained for backward compatibility with any already-delivered messages.

### Keyboard Layout

Replace the current single-row 3-button keyboard with a two-row layout:

```
Row 0: [Handle]  [Delegate to Nova]  [Dismiss]
Row 1: [Edit]  [Cancel]  [Extend +24h]  [Snooze 1h]  [Snooze 4h]  [Snooze tomorrow]
```

Telegram inline keyboards support at most ~8 characters per button label in compact view, so labels
are kept short. Row 1 buttons are narrow. If the six-button second row is too wide for the mobile
client, it can be split: `[Edit] [Cancel] [Extend]` on row 1, `[Snooze 1h] [Snooze 4h] [Snooze
tomorrow]` on row 2. The engineer should test visually and choose the layout that renders cleanly.

### ob_edit handler

`ob_edit:{obligation_id}` is the new "Edit" button. The flow mirrors `handle_edit` for pending
actions:

1. Router receives `ob_edit:{id}`, sets an `editing_obligation_id: Option<String>` field on the
   orchestrator (parallel to the existing `editing_action_id: Option<Uuid>` for pending actions).
2. The orchestrator's inbound message handler checks `editing_obligation_id` first. If set, the
   next plain text message from the user is treated as the new `detected_action` text.
3. On receiving that text: call `obligation_store.update_detected_action(id, new_text)` (new
   method, see below), edit the original Telegram message to show the updated text, and clear
   `editing_obligation_id`.

New `ObligationStore` method:
```rust
pub fn update_detected_action(&self, id: &str, new_text: &str) -> Result<bool> {
    let rows_changed = self.conn.execute(
        "UPDATE obligations SET detected_action = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![new_text, id],
    )?;
    Ok(rows_changed > 0)
}
```

Toast label: `"Editing..."` (add `ob_edit:` to `callback_label()` in `telegram/mod.rs`).

### ob_cancel handler

`ob_cancel:{obligation_id}` sets `status=Dismissed` on the obligation and edits the original
message to read "Cancelled: {detected_action}".

Implementation: add `handle_ob_cancel` in `callbacks.rs` (or inline in orchestrator alongside
`handle_obligation_callback`). Reuse `obligation_store.update_status(id, Dismissed)`.

Toast label: `"Cancelled."`.

### ob_expiry handler

`ob_expiry:{obligation_id}` acknowledges that the user wants more time. The obligation store has
no deadline field, so this handler:

1. Does not mutate the obligation store.
2. Edits the original Telegram message: "Deadline extended by 24h. Obligation remains open."
3. Optionally creates a reminder 24h from now via `ReminderStore` with message "Obligation still
   open: {detected_action}" (if reminder store is available in the orchestrator context).

Toast label: `"Extended."`. Add `ob_expiry:` to `callback_label()`.

### ob_snooze handler

`ob_snooze:{obligation_id}:{offset}` where `{offset}` is `1h`, `4h`, or `tomorrow`.

1. Parse the offset using the existing `parse_relative_time()` from `reminders.rs` (already
   handles `"1h"`, `"4h"`, `"tomorrow"`).
2. Look up the obligation from `obligation_store.get_by_id(id)` to get `detected_action` for
   the reminder message.
3. Create a reminder in `ReminderStore` with message "Obligation: {detected_action}" and channel
   "telegram".
4. Edit the original Telegram message: "Snoozed until {human_time}."

The orchestrator already holds a `reminder_store` reference — thread it into the callback handler.

Toast label: `"Snoozed."`. Add `ob_snooze:` to `callback_label()`.

### Routing in orchestrator.rs

The callback router in `orchestrator.rs` (the `process_trigger_batch` inner branch that matches
`[callback]` content) currently handles `ob_handle:`, `ob_delegate:`, `ob_dismiss:`,
`approve:`, `edit:`, and `cancel:` prefixes. Add branches for:

```rust
} else if let Some(ob_id) = data.strip_prefix("ob_edit:") {
    // Set editing_obligation_id, send "What should the obligation text say?" reply
} else if let Some(ob_id) = data.strip_prefix("ob_cancel:") {
    // handle_ob_cancel(ob_id, ...)
} else if let Some(ob_id) = data.strip_prefix("ob_expiry:") {
    // handle_ob_expiry(ob_id, ...)
} else if let Some(rest) = data.strip_prefix("ob_snooze:") {
    // rest = "{id}:{offset}", split on last ':' to get id and offset
    // handle_ob_snooze(id, offset, ...)
}
```

For the edit flow, add a field to the orchestrator struct:
```rust
editing_obligation_id: Option<String>,
```

And in the inbound message handling path, check this field before dispatching to Claude.

### New fields needed on orchestrator

The orchestrator already has `editing_action_id: Option<Uuid>`. Add a sibling:

```rust
/// Obligation ID being edited (waiting for new text from user).
editing_obligation_id: Option<String>,
```

Initialize to `None` in the constructor.

## Dependencies

- `obligation_store.rs` — add `update_detected_action` method
- `callbacks.rs` — add `handle_ob_cancel`, `handle_ob_expiry`, `handle_ob_snooze` functions
- `orchestrator.rs` — extend callback router, extend `obligation_keyboard()`, add
  `editing_obligation_id` field, add inbound message edit-flow branch
- `channels/telegram/mod.rs` — extend `callback_label()` with new prefixes
- `reminders.rs` — `parse_relative_time` and `ReminderStore::create_reminder` are reused as-is

## Out of Scope

- Editing obligation fields other than `detected_action` (priority, owner, project_code)
- Obligation status transitions beyond what the existing `update_status` supports
- Persisting a `due_at` deadline column on the obligations table (schema migration deferred)
- Multi-step edit flows (e.g. presenting a form with multiple fields)
- Obligation snooze to a specific clock time (only offsets supported in this spec)

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` — all existing unit tests pass, new unit tests for:
  - `update_detected_action` on `ObligationStore`
  - `handle_ob_cancel`, `handle_ob_expiry`, `handle_ob_snooze` in `callbacks.rs`
  - `callback_label` covers all new prefixes
  - `obligation_keyboard` produces the expected two-row layout
- `cargo clippy` passes with no warnings
- Manual gate: receive an obligation alert in Telegram, tap each new button and verify the message
  is edited to show the correct state change
