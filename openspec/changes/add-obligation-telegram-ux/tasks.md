# Tasks: add-obligation-telegram-ux

## Dependencies

- `add-obligation-detection` (detection must work to trigger notifications)

## Tasks

### Notification Formatter (already complete)

- [x] [1.1] Implement `format_obligation_card(obligation, source_channel) -> String` in `orchestrator.rs` -- HTML-formatted card with priority badge (P0 CRITICAL through P4 BACKLOG), source channel in `<code>` tags, optional project code, detected action, owner with reason [owner:api-engineer]
- [x] [1.2] Implement `escape_html_plain(text) -> String` helper for escaping `&`, `<`, `>` in card content [owner:api-engineer]

### Inline Keyboard (already complete)

- [x] [2.1] Implement `obligation_keyboard(obligation_id) -> InlineKeyboard` in `orchestrator.rs` -- single row with Handle (`ob_handle:{id}`), Delegate to Nova (`ob_delegate:{id}`), Dismiss (`ob_dismiss:{id}`) buttons [owner:api-engineer]

### Callback Handlers (already complete)

- [x] [3.1] Implement `handle_obligation_callback()` method on Orchestrator -- accepts obligation_id, new_status, optional new_owner, chat_id, msg_id, confirmation_text. Locks ObligationStore, calls update_status_and_owner or update_status, edits original Telegram message with confirmation [owner:api-engineer]
- [x] [3.2] Wire `ob_handle:` callback routing in `process_trigger_batch()` callback section -- sets status=InProgress, owner=Leo, confirmation "Obligation assigned to Leo." [owner:api-engineer]
- [x] [3.3] Wire `ob_delegate:` callback routing -- sets status=InProgress, owner=Nova, confirmation "Obligation delegated to Nova." [owner:api-engineer]
- [x] [3.4] Wire `ob_dismiss:` callback routing -- sets status=Dismissed, no owner change, confirmation "Obligation dismissed." [owner:api-engineer]

### P0-P1 Notification Trigger (already complete)

- [x] [4.1] In orchestrator detection spawn block: after storing obligation, check if priority <= 1. If yes and Telegram client + chat_id available, send card via `send_message` with keyboard [owner:api-engineer]

### Morning Briefing (already complete)

- [x] [5.1] Implement `send_morning_briefing()` method on Orchestrator -- query `count_open_by_priority()`, format with `format_morning_briefing()`, send via Telegram [owner:api-engineer]
- [x] [5.2] Implement `format_morning_briefing(open_obligations, total_open) -> String` -- HTML formatted, shows "No open obligations." when empty, otherwise lists counts per priority level [owner:api-engineer]

### Remaining Tests

- [ ] [6.1] Unit test: `format_obligation_card` with P0 priority -- verify output contains "P0 CRITICAL", source channel in code tags, detected action text [owner:api-engineer]
- [ ] [6.2] Unit test: `format_obligation_card` with project_code=None -- verify project line is omitted [owner:api-engineer]
- [ ] [6.3] Unit test: `format_obligation_card` with project_code=Some("OO") -- verify "Project: OO" line present [owner:api-engineer]
- [ ] [6.4] Unit test: `format_obligation_card` with owner_reason=Some(...) -- verify reason appears after owner [owner:api-engineer]
- [ ] [6.5] Unit test: `obligation_keyboard` -- verify 3 buttons in single row, callback_data starts with correct prefixes (ob_handle:, ob_delegate:, ob_dismiss:), all contain the obligation_id [owner:api-engineer]
- [ ] [6.6] Unit test: `format_morning_briefing` with zero obligations -- verify contains "No open obligations." [owner:api-engineer]
- [ ] [6.7] Unit test: `format_morning_briefing` with mixed priorities -- verify all priority counts appear, total shown correctly [owner:api-engineer]
- [ ] [6.8] Unit test: `format_morning_briefing` HTML structure -- verify contains `<b>` tags for heading, priority labels match P0-P4 names [owner:api-engineer]

### Verify

- [ ] [7.1] `cargo build` passes for all workspace members [owner:api-engineer]
- [ ] [7.2] `cargo test -p nv-daemon` -- all obligation notification tests pass [owner:api-engineer]
- [ ] [7.3] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [7.4] [user] Manual test: send obligation-triggering message via Telegram, verify card appears with inline keyboard, tap each button (Handle/Delegate/Dismiss) on separate obligations, verify confirmation replaces card [owner:api-engineer]
