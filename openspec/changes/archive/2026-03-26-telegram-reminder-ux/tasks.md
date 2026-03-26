# Tasks: telegram-reminder-ux

## Beads epic: nv-myw8

## Dependencies

- callback-handler-completion (Wave 3) — in-process callback dispatch pattern

## Tasks

### nv-core: Keyboard Builder

- [x] Add `InlineKeyboard::reminder_actions(reminder_id: i64) -> Self` in `crates/nv-core/src/types.rs` — one row: `[Mark Done | Snooze | Backlog]` with callback data `reminder_done:{id}`, `reminder_snooze:{id}`, `reminder_backlog:{id}` [owner:api-engineer]
- [x] Add unit test for `reminder_actions` layout: verify 1 row, 3 buttons, correct callback data prefixes [owner:api-engineer]

### reminders.rs: Schema + Struct + Scheduler

- [x] Add migration v2 in `reminders_migrations()`: `ALTER TABLE reminders ADD COLUMN obligation_id TEXT` [owner:api-engineer]
- [x] Add `pub obligation_id: Option<String>` field to `Reminder` struct [owner:api-engineer]
- [x] Update `create_reminder` signature to accept `obligation_id: Option<&str>` and persist it; update all call sites to pass `None` [owner:api-engineer]
- [x] Update `list_active_reminders`, `get_due_reminders` row mapper to read `obligation_id` from column index 7 (shift existing `cancelled` to index 8); update `ReminderStore::mark_delivered` and `cancel_reminder` queries (no column changes needed there) [owner:api-engineer]
- [x] In `spawn_reminder_scheduler`, attach `keyboard: Some(InlineKeyboard::reminder_actions(reminder.id))` to `OutboundMessage` instead of `None` [owner:api-engineer]
- [x] Update existing `reminder_store_crud` and related tests for new `create_reminder` signature (pass `None` for `obligation_id`) [owner:api-engineer]
- [x] Add unit test: `create_reminder` with `Some("obl-id")`, list active, verify `obligation_id` round-trips [owner:api-engineer]

### TelegramChannel: Store Fields + Callback Routing

- [x] Add `reminder_store: Option<Arc<Mutex<ReminderStore>>>` and `obligation_store: Option<Arc<Mutex<ObligationStore>>>` fields to `TelegramChannel` struct in `crates/nv-daemon/src/channels/telegram/mod.rs` [owner:api-engineer]
- [x] Update `TelegramChannel::new()` to accept both optional stores; update `run_poll_loop` signature if needed (pass stores through) [owner:api-engineer]
- [x] In `poll_messages()`, after `answer_callback_query`, check if `cb.data` starts with `"reminder_"` — if so, call `handle_reminder_callback` and `continue` (skip `Trigger::Message` dispatch) [owner:api-engineer]
- [x] Extend `callback_label()` with new prefixes: `reminder_done:` → `"Done. Closed."`, `reminder_snooze:` → `"Choose snooze time..."`, `reminder_snooze_1h:` → `"Snoozed 1 hour."`, `reminder_snooze_4h:` → `"Snoozed 4 hours."`, `reminder_snooze_tomorrow:` → `"Snoozed until tomorrow."`, `reminder_backlog:` → `"Moved to backlog."` [owner:api-engineer]
- [x] Update `callback_label` unit tests for new prefixes [owner:api-engineer]

### callbacks.rs: Handler

- [x] Create `crates/nv-daemon/src/channels/telegram/callbacks.rs` with `pub async fn handle_reminder_callback(data: &str, reminder_store: &Option<Arc<Mutex<ReminderStore>>>, obligation_store: &Option<Arc<Mutex<ObligationStore>>>, client: &TelegramClient, chat_id: i64, timezone: &str)` [owner:api-engineer]
- [x] Implement `reminder_done:{id}` branch: parse id, mark reminder delivered, if obligation_id is Some call `ObligationStore::update_status(Done)` [owner:api-engineer]
- [x] Implement `reminder_snooze:{id}` branch: send snooze picker keyboard as a new Telegram message with buttons `[+1h | +4h | Tomorrow]` and callback data `reminder_snooze_1h:{id}`, `reminder_snooze_4h:{id}`, `reminder_snooze_tomorrow:{id}` [owner:api-engineer]
- [x] Implement `reminder_snooze_1h:{id}`, `reminder_snooze_4h:{id}`, `reminder_snooze_tomorrow:{id}` branches: mark original as delivered, compute new `due_at` (use `reminders::parse_relative_time` for tomorrow), create new reminder via `ReminderStore::create_reminder` copying message/channel/obligation_id, send confirmation text `"Snoozed. New reminder set for {time}."` [owner:api-engineer]
- [x] Implement `reminder_backlog:{id}` branch: parse id, look up reminder obligation_id, if Some call `ObligationStore::update_status(Dismissed)`, mark reminder delivered [owner:api-engineer]
- [x] Add unit tests for `handle_reminder_callback` covering: done with obligation, done without obligation, backlog with obligation, backlog without obligation, snooze picker sent, snooze_1h creates new reminder [owner:api-engineer]

### main.rs: Wiring

- [x] In `main.rs`, pass `reminder_store.clone()` and `obligation_store.clone()` into `TelegramChannel::new()` (or via setter) at startup [owner:api-engineer]

### Verify

- [x] `cargo build` passes for all workspace members [owner:api-engineer]
- [x] `cargo test -p nv-core` passes [owner:api-engineer]
- [x] `cargo test -p nv-daemon` passes (all existing tests + new tests) [owner:api-engineer]
- [x] `cargo clippy` passes with no warnings [owner:api-engineer]
- [ ] Manual gate: run daemon, wait for a due reminder, verify notification arrives with Mark Done / Snooze / Backlog buttons [user]
- [ ] Manual gate: tap Mark Done — toast shows "Done. Closed.", reminder marked delivered [user]
- [ ] Manual gate: tap Snooze — picker appears with +1h / +4h / Tomorrow; tap +1h — new reminder scheduled, confirmation text sent [user]
- [ ] Manual gate: tap Backlog — toast shows "Moved to backlog." [user]
