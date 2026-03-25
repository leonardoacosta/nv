# Tasks: callback-handler-completion

## Beads Epic

nv-ags8

## Dependencies

- obligation_store.rs (already implemented — add one method)
- callbacks.rs (already exists — add three new handler functions)
- orchestrator.rs (extend callback router, keyboard builder, add editing field)
- channels/telegram/mod.rs (extend callback_label)
- reminders.rs (reuse parse_relative_time, ReminderStore::create_reminder — no changes)

## Tasks

### ObligationStore: update_detected_action

- [ ] Add `update_detected_action(&self, id: &str, new_text: &str) -> Result<bool>` to
  `crates/nv-daemon/src/obligation_store.rs` — UPDATE obligations SET detected_action = ?1,
  updated_at = datetime('now') WHERE id = ?2, returns Ok(rows_changed > 0) [owner:api-engineer]
- [ ] Add unit test `update_detected_action_changes_text` to `obligation_store.rs` tests block —
  create obligation, call update_detected_action, get_by_id and assert detected_action changed,
  assert updated_at >= created_at [owner:api-engineer]

### callbacks.rs: new obligation handlers

- [ ] Add `handle_ob_cancel(ob_id: &str, telegram: &TelegramClient, chat_id: i64,
  original_message_id: Option<i64>, ob_store: &ObligationStore) -> Result<()>` to
  `crates/nv-daemon/src/callbacks.rs` — call ob_store.update_status(ob_id, Dismissed), edit
  original message with "Cancelled: {detected_action}" [owner:api-engineer]
- [ ] Add `handle_ob_expiry(ob_id: &str, telegram: &TelegramClient, chat_id: i64,
  original_message_id: Option<i64>, ob_store: &ObligationStore,
  reminder_store: Option<&Mutex<ReminderStore>>, timezone: &str) -> Result<()>` to
  `crates/nv-daemon/src/callbacks.rs` — create a 24h reminder via ReminderStore if available,
  edit original message with "Deadline extended by 24h. Obligation remains open." [owner:api-engineer]
- [ ] Add `handle_ob_snooze(ob_id: &str, offset: &str, telegram: &TelegramClient, chat_id: i64,
  original_message_id: Option<i64>, ob_store: &ObligationStore,
  reminder_store: &Mutex<ReminderStore>, timezone: &str) -> Result<()>` to
  `crates/nv-daemon/src/callbacks.rs` — parse offset via parse_relative_time, get obligation
  detected_action, call ReminderStore::create_reminder, edit original message with "Snoozed until
  {human_time}." [owner:api-engineer]

### callback_label: extend toast labels

- [ ] Extend `callback_label()` in `crates/nv-daemon/src/channels/telegram/mod.rs` with new
  prefixes: `ob_edit:` -> `"Editing..."`, `ob_cancel:` -> `"Cancelled."`,
  `ob_expiry:` -> `"Extended."`, `ob_snooze:` -> `"Snoozed."` [owner:api-engineer]
- [ ] Update `callback_label_maps_known_prefixes` unit test in `telegram/mod.rs` to assert all four
  new prefixes return their expected toast strings [owner:api-engineer]

### obligation_keyboard: add new buttons

- [ ] Extend `obligation_keyboard()` in `crates/nv-daemon/src/orchestrator.rs` to produce a
  two-row keyboard: row 0 = [Handle, Delegate to Nova, Dismiss], row 1 = [Edit, Cancel, Extend,
  Snooze 1h, Snooze 4h, Snooze tomorrow] using callback prefixes `ob_edit:`, `ob_cancel:`,
  `ob_expiry:`, `ob_snooze:{id}:1h`, `ob_snooze:{id}:4h`, `ob_snooze:{id}:tomorrow`
  [owner:api-engineer]
- [ ] Update `obligation_keyboard_layout` unit test in `orchestrator.rs` to assert the new two-row
  layout and verify each button's callback_data prefix and obligation_id embedding [owner:api-engineer]

### orchestrator.rs: editing_obligation_id field and inbound edit flow

- [ ] Add `editing_obligation_id: Option<String>` field to the orchestrator struct in
  `crates/nv-daemon/src/orchestrator.rs`, initialize to None in the constructor [owner:api-engineer]
- [ ] In the inbound message handler in `orchestrator.rs`, add a check before dispatching to Claude:
  if `editing_obligation_id` is Some(id) and the message is plain text (not a callback), call
  `ob_store.update_detected_action(id, content)`, edit the original Telegram message with "Updated:
  {new_text}", clear `editing_obligation_id`, and skip Claude dispatch for that message [owner:api-engineer]

### orchestrator.rs: callback router — new branches

- [ ] Add `ob_edit:` branch to the callback router in `orchestrator.rs`: set
  `editing_obligation_id = Some(ob_id.to_string())`, send reply "What should the obligation text
  say?" via telegram [owner:api-engineer]
- [ ] Add `ob_cancel:` branch to the callback router in `orchestrator.rs`: call
  `handle_ob_cancel(ob_id, ...)` [owner:api-engineer]
- [ ] Add `ob_expiry:` branch to the callback router in `orchestrator.rs`: call
  `handle_ob_expiry(ob_id, ...)` [owner:api-engineer]
- [ ] Add `ob_snooze:` branch to the callback router in `orchestrator.rs`: split `rest` on the last
  `:` to extract `(ob_id, offset)`, call `handle_ob_snooze(ob_id, offset, ...)` [owner:api-engineer]

### Verify

- [ ] `cargo build` passes for all workspace members [owner:api-engineer]
- [ ] `cargo test -p nv-daemon` — all existing tests pass plus new tests for
  update_detected_action, handle_ob_cancel, handle_ob_expiry, handle_ob_snooze, new
  callback_label prefixes, and obligation_keyboard two-row layout [owner:api-engineer]
- [ ] `cargo clippy` passes with no warnings [owner:api-engineer]
- [ ] Manual gate: receive obligation alert in Telegram, tap "Edit" — bot prompts for new text,
  send new text — message updates, tap "Cancel" — message reads "Cancelled: ...", tap "Extend" —
  message reads "Deadline extended by 24h...", tap "Snooze 1h" — message reads "Snoozed until
  {time}" and reminder fires ~1h later [user]
