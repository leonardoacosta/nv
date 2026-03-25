# telegram-reminder-ux

## Summary

Add inline keyboard action buttons to Telegram reminder notifications. When a reminder fires, the notification includes three buttons: "Mark Done" (closes the associated obligation), "Snooze" (defers with a 1h/4h/tomorrow sub-picker), and "Backlog" (moves the obligation to P4). Callback data from each button is routed in `channels/telegram/mod.rs` directly — no agent round-trip required for these structural state transitions.

## Motivation

Reminders currently fire as plain text: `"Reminder: {message}"`. The user must either ignore the notification or manually follow up. There is no way to act on the reminder in-place. Adding action buttons turns each reminder into a lightweight triage touchpoint — single tap closes the loop, snoozes, or deprioritizes without typing.

## Design

### Notification Format

The reminder scheduler (`reminders.rs` → `spawn_reminder_scheduler`) currently constructs:

```rust
let text = format!("Reminder: {}", reminder.message);
let msg = OutboundMessage {
    channel: reminder.channel.clone(),
    content: text,
    reply_to: None,
    keyboard: None,
};
```

Change `keyboard: None` to attach a `reminder_keyboard(reminder_id)` builder. The message text is enriched with the obligation ID when one is associated (see "Obligation Linkage" below).

New notification format:

```
Reminder: <message>
```

With inline keyboard (one row of 3 buttons):

```
[ Mark Done ]  [ Snooze ]  [ Backlog ]
```

### Keyboard Builder

Add `InlineKeyboard::reminder_actions(reminder_id: i64)` in `crates/nv-core/src/types.rs`:

```rust
pub fn reminder_actions(reminder_id: i64) -> Self {
    Self {
        rows: vec![vec![
            InlineButton {
                text: "Mark Done".to_string(),
                callback_data: format!("reminder_done:{reminder_id}"),
            },
            InlineButton {
                text: "Snooze".to_string(),
                callback_data: format!("reminder_snooze:{reminder_id}"),
            },
            InlineButton {
                text: "Backlog".to_string(),
                callback_data: format!("reminder_backlog:{reminder_id}"),
            },
        ]],
    }
}
```

### Callback Data Contract

| Button | Callback data | Effect |
|--------|--------------|--------|
| Mark Done | `reminder_done:{reminder_id}` | Mark reminder delivered; if obligation linked, set status=Done |
| Snooze | `reminder_snooze:{reminder_id}` | Send snooze picker keyboard (1h / 4h / Tomorrow) |
| Backlog | `reminder_backlog:{reminder_id}` | If obligation linked, set priority=4 (Backlog); ack reminder |

Snooze picker is a follow-up keyboard sent as a new message (not an edit of the original):

```
[ +1h ]  [ +4h ]  [ Tomorrow ]
```

Callback data for picker: `reminder_snooze_1h:{reminder_id}`, `reminder_snooze_4h:{reminder_id}`, `reminder_snooze_tomorrow:{reminder_id}`.

Each snooze action creates a new reminder in the store with the appropriate offset and marks the original as delivered.

### Callback Routing

Extend `callback_label()` in `channels/telegram/mod.rs` to cover the new prefixes:

| Prefix | Toast text |
|--------|-----------|
| `reminder_done:` | `"Done. Closed."` |
| `reminder_snooze:` | `"Choose snooze time..."` |
| `reminder_snooze_1h:` | `"Snoozed 1 hour."` |
| `reminder_snooze_4h:` | `"Snoozed 4 hours."` |
| `reminder_snooze_tomorrow:` | `"Snoozed until tomorrow."` |
| `reminder_backlog:` | `"Moved to backlog."` |

The actual callback handling lives in `channels/telegram/mod.rs` inside `poll_messages()`. Callbacks with `reminder_` prefix are handled in-process (no `Trigger::Message` dispatch to the agent loop):

```rust
// In poll_messages(), after answering the callback query:
if let Some(data) = &cb.data {
    if data.starts_with("reminder_") {
        handle_reminder_callback(data, &self.reminder_store, &self.obligation_store, &self.client, self.chat_id).await?;
        continue; // do not dispatch as Trigger::Message
    }
}
```

`handle_reminder_callback` is a free async function in `channels/telegram/callbacks.rs` (new file).

### Obligation Linkage

Reminders are currently stored without an obligation ID. To support "Mark Done" and "Backlog" on obligation-linked reminders, the reminder store schema needs an optional `obligation_id` column.

**Schema migration** (new migration version in `reminders_migrations()`):

```sql
ALTER TABLE reminders ADD COLUMN obligation_id TEXT;
```

**`Reminder` struct**: add `pub obligation_id: Option<String>`.

**`ReminderStore::create_reminder`**: add `obligation_id: Option<&str>` parameter and persist it.

All existing callers pass `None` — no behavioral change for non-obligation reminders.

The `obligation_id` is populated by the obligation detector when it creates reminder-linked obligations (out of scope for this spec — callers simply pass `None` for now, and the callback handler skips the obligation transition when `obligation_id` is `None`).

### Dependency Injection: TelegramChannel

`TelegramChannel` does not currently hold `ReminderStore` or `ObligationStore`. Two options:

**Option A**: Pass `Arc<Mutex<ReminderStore>>` and `Arc<Mutex<ObligationStore>>` into `TelegramChannel::new()`.

**Option B**: Route callbacks through `Trigger::Message` to the agent loop which already has access to both stores.

**Decision: Option A.** Reminder actions are structural state transitions (not AI decisions). Routing through the agent loop adds latency and burns Claude API tokens for what is fundamentally a database write. The stores are already `Arc<Mutex<...>>` and cheap to clone.

`TelegramChannel` gains two optional fields:

```rust
pub struct TelegramChannel {
    pub client: TelegramClient,
    pub chat_id: i64,
    trigger_tx: mpsc::Sender<Trigger>,
    offset: Arc<AtomicI64>,
    reminder_store: Option<Arc<Mutex<ReminderStore>>>,
    obligation_store: Option<Arc<Mutex<ObligationStore>>>,
}
```

Both are `Option` so the channel remains constructable without them (for tests and channels that don't need it).

`main.rs` wires both stores into `TelegramChannel::new()` at startup.

### Snooze Implementation

`handle_reminder_callback` for snooze actions:

1. Parse `reminder_id` from callback data.
2. Mark the original reminder as delivered (`ReminderStore::mark_delivered`).
3. Compute new `due_at` using existing `parse_relative_time` helpers:
   - `reminder_snooze_1h` → `Utc::now() + 1h`
   - `reminder_snooze_4h` → `Utc::now() + 4h`
   - `reminder_snooze_tomorrow` → `parse_relative_time("tomorrow", tz)` (uses daemon config timezone)
4. Create a new reminder via `ReminderStore::create_reminder` with the same message and channel, copying `obligation_id`.
5. Send confirmation text: `"Snoozed. New reminder set for {time}."` as a new Telegram message.

### Backlog Implementation

For `reminder_backlog:{reminder_id}`:

1. Parse `reminder_id`, look up the reminder to get `obligation_id`.
2. If `obligation_id` is `Some`, call `ObligationStore::update_status` to set status to `Dismissed` (P4 semantics; there is no separate `priority` update method — Dismissed maps to "moved out of active queue").
3. Mark the reminder as delivered.
4. Answer: `"Moved to backlog."` (already done by `answer_callback_query`).

Note: `ObligationStore` does not have an `update_priority` method. "Backlog" in this context means closing the obligation as Dismissed rather than updating its priority field. If a true priority-update method is needed in future, that is a separate spec.

### Mark Done Implementation

For `reminder_done:{reminder_id}`:

1. Parse `reminder_id`, look up the reminder to get `obligation_id`.
2. If `obligation_id` is `Some`, call `ObligationStore::update_status(id, ObligationStatus::Done)`.
3. Mark the reminder as delivered.
4. The `answer_callback_query` toast already says `"Done. Closed."`.

## Files Affected

| File | Change |
|------|--------|
| `crates/nv-core/src/types.rs` | Add `InlineKeyboard::reminder_actions(id)` builder |
| `crates/nv-daemon/src/reminders.rs` | Add `obligation_id` to schema (migration v2), `Reminder` struct, `create_reminder` signature; attach keyboard in scheduler |
| `crates/nv-daemon/src/channels/telegram/mod.rs` | Add store fields to `TelegramChannel`; route `reminder_` callbacks in-process in `poll_messages()`; extend `callback_label()` |
| `crates/nv-daemon/src/channels/telegram/callbacks.rs` | New file: `handle_reminder_callback` async fn |
| `crates/nv-daemon/src/main.rs` | Wire `reminder_store` and `obligation_store` into `TelegramChannel::new()` |

## Dependencies

- `callback-handler-completion` (Wave 3) — establishes the in-process callback dispatch pattern used here

## Out of Scope

- Populating `obligation_id` on reminders from the obligation detector (deferred)
- Custom snooze duration input (e.g. "remind me in 3h")
- Editing the original notification message to remove buttons after action
- Non-Telegram channels (iMessage, email do not support inline keyboards)

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` passes (existing + new unit tests)
- `cargo test -p nv-core` passes
- `cargo clippy` passes with no warnings
- Unit tests: `InlineKeyboard::reminder_actions` layout; `callback_label` new prefixes; `handle_reminder_callback` for done/snooze/backlog with mock stores
- Manual gate: daemon running, trigger a due reminder, verify notification arrives with 3 buttons; tap each button and verify correct behavior [user]
