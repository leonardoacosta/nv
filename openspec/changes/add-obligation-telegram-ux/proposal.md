# Proposal: Add Obligation Telegram UX

## Change ID
`add-obligation-telegram-ux`

## Summary

Obligation notifications on Telegram: formatted HTML cards with priority badge, source channel,
project code, detected action, and owner classification. Inline keyboard with three buttons:
[Handle] [Delegate to Nova] [Dismiss]. Callback handlers update obligation status and owner.
Morning briefing digest summarizes open obligations by priority.

## Context
- Depends on: `add-obligation-detection`
- Files: `crates/nv-daemon/src/orchestrator.rs` (notification + callbacks + briefing),
  `crates/nv-daemon/src/channels/telegram/client.rs` (send/edit message with keyboard)
- Scope-lock reference: Phase 3 "Proactive behavior" -- Telegram notification

## Motivation

Obligation detection without notification is invisible. Leo needs to see detected obligations
in Telegram with enough context to decide: handle it personally, delegate to Nova, or dismiss.
The inline keyboard enables one-tap action without typing. The morning briefing provides a daily
summary of open obligations so nothing falls through the cracks.

## Design

### Notification Card (format_obligation_card)

HTML-formatted message sent via Telegram `sendMessage` with `parse_mode: "HTML"`:

```
[P1 HIGH] New Obligation
Channel: telegram
Project: OO
Action: Deploy the auth service by Friday
Owner: leo (Requires Leo to coordinate with DevOps.)
```

- Priority badge: P0 CRITICAL, P1 HIGH, P2 IMPORTANT, P3 MINOR, P4 BACKLOG
- Source channel in `<code>` tags
- Project code shown only when present
- Owner with reason on same line

### Inline Keyboard (obligation_keyboard)

Single row with three buttons:

| Button | Callback Data | Effect |
|--------|--------------|--------|
| Handle | `ob_handle:{id}` | status=in_progress, owner=Leo |
| Delegate to Nova | `ob_delegate:{id}` | status=in_progress, owner=Nova |
| Dismiss | `ob_dismiss:{id}` | status=dismissed, owner unchanged |

### Callback Handlers (handle_obligation_callback)

Generic handler for all three callbacks:
1. Parse obligation ID from callback data prefix (`ob_handle:`, `ob_delegate:`, `ob_dismiss:`)
2. Lock ObligationStore mutex
3. Call `update_status_and_owner` or `update_status` depending on whether owner changes
4. Edit original Telegram message to show confirmation text (e.g., "Obligation assigned to Leo.")
5. Log the action with obligation ID, new status, and new owner

### Notification Trigger Rules

- **P0-P1**: Always notify immediately via Telegram card with keyboard
- **P2-P4**: Stored silently, included in morning briefing only

### Morning Briefing (send_morning_briefing)

Triggered by cron schedule. Queries `ObligationStore::count_open_by_priority()` and formats:

```
Good morning. Daily briefing:

Open obligations: 5 total
  P0 Critical: 1
  P1 High: 2
  P2 Important: 2
```

When no open obligations: "No open obligations."

### TelegramClient Support

The existing `TelegramClient` already supports all required operations:
- `send_message(chat_id, text, reply_to, keyboard)` with HTML parse mode and inline keyboard
- `edit_message(chat_id, message_id, text, keyboard)` for callback confirmations
- `answer_callback_query(callback_query_id, text)` for dismissing button loading spinner
- `get_updates` with `allowed_updates: ["message", "callback_query"]`

## Current State

This work is **already implemented**:
- `format_obligation_card()` helper in orchestrator.rs with HTML formatting, priority badge,
  channel, project code, action, owner with reason
- `obligation_keyboard()` helper returning 3-button inline keyboard (Handle, Delegate, Dismiss)
- `handle_obligation_callback()` method on Orchestrator handling all three callback types
- Callback routing in `process_trigger_batch()` for `ob_handle:`, `ob_delegate:`, `ob_dismiss:` prefixes
- P0-P1 immediate notification with card + keyboard in detection spawn block
- Morning briefing via `send_morning_briefing()` with `format_morning_briefing()` helper
- `answer_callback_query` called in callback processing

## Remaining Work

- Unit test: `format_obligation_card` produces expected HTML for various priorities and with/without
  project code
- Unit test: `obligation_keyboard` produces correct callback data prefixes
- Unit test: `format_morning_briefing` with zero obligations, single priority, multiple priorities
- Verify `cargo build` gate passes

## Dependencies

- `add-obligation-detection` (detection must work to trigger notifications)

## Out of Scope

- Obligation list command (`/obligations` bot command -- separate spec)
- Rich media cards (photos, documents attached to obligations)
- Notification deduplication (same obligation detected twice)
- Quiet hours suppression for obligation notifications (digest already respects quiet hours)

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` -- all obligation notification tests pass
- `cargo clippy -- -D warnings` passes
- [user] Manual test: send a message containing an obligation via Telegram, verify card appears
  with keyboard, tap Handle/Delegate/Dismiss and verify confirmation message replaces card
