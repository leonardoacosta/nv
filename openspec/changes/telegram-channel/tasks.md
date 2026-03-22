# Tasks: telegram-channel

## Dependencies

- core-types-and-config (spec-2)

## Tasks

### Telegram API Types

- [ ] Create `crates/nv-daemon/src/telegram/types.rs` with `TelegramResponse<T>`, `Update`, `TgMessage`, `TgUser`, `TgChat`, `CallbackQuery` — all `#[derive(Debug, Deserialize)]`
- [ ] Implement `Update::to_inbound_message()` — converts regular messages (text → InboundMessage with message_id/chat_id in metadata)
- [ ] Handle `callback_query` in `to_inbound_message()` — content prefixed `[callback]`, thread_id links to original message, metadata carries callback_query_id and callback_data

### Bot API Client

- [ ] Create `crates/nv-daemon/src/telegram/client.rs` with `TelegramClient` struct (reqwest::Client + base_url from bot token)
- [ ] Implement `TelegramClient::new(bot_token: &str)` — constructs base URL `https://api.telegram.org/bot{token}`
- [ ] Implement `get_me()` — GET `/getMe`, verify bot token is valid, return bot username
- [ ] Implement `get_updates(offset: i64, timeout: u64)` — POST `/getUpdates` with offset, timeout, allowed_updates `["message", "callback_query"]`. HTTP timeout = poll timeout + 10s buffer.
- [ ] Implement `send_message(chat_id, text, reply_to, keyboard)` — POST `/sendMessage` with `parse_mode: "Markdown"`, optional `reply_to_message_id`, optional `reply_markup` (inline_keyboard JSON)
- [ ] Implement `answer_callback_query(callback_query_id, text)` — POST `/answerCallbackQuery` to dismiss inline button loading spinner
- [ ] Implement `chunk_message(text, max_len)` — split messages at paragraph/line boundaries when exceeding 4096 char Telegram limit

### TelegramChannel (Channel Trait)

- [ ] Create `crates/nv-daemon/src/telegram/mod.rs` with `TelegramChannel` struct: `TelegramClient`, `chat_id: i64`, `trigger_tx: mpsc::Sender<Trigger>`, `offset: Arc<AtomicI64>`
- [ ] Implement `TelegramChannel::new(bot_token, chat_id, trigger_tx)` — initialize client and offset at 0
- [ ] Implement `Channel::name()` — returns `"telegram"`
- [ ] Implement `Channel::connect()` — call `get_me()` to verify token, log bot username
- [ ] Implement `Channel::poll_messages()` — call `get_updates` with current offset, filter by authorized `chat_id`, convert to `InboundMessage` vec, advance offset to `max(update_id) + 1`
- [ ] Implement `Channel::send_message()` — delegate to `TelegramClient::send_message`, handle message chunking for long content
- [ ] Implement `Channel::disconnect()` — log disconnection, no cleanup needed

### Long-Poll Loop

- [ ] Implement `run_poll_loop(channel: TelegramChannel)` — continuous loop: poll → push triggers → handle errors
- [ ] Implement exponential backoff on poll failure: start 1s, double on consecutive failures, cap at 60s, reset on success
- [ ] Exit loop when `trigger_tx.send()` fails (receiver dropped = daemon shutting down)
- [ ] Filter updates by `chat_id` — silently drop messages from unauthorized chats

### Inline Keyboard Builder

- [ ] Implement `InlineKeyboard::confirm_action(action_id)` — 3-button row: Approve (`approve:{id}`), Edit (`edit:{id}`), Cancel (`cancel:{id}`)
- [ ] Implement `InlineKeyboard::from_actions(actions: &[PendingAction])` — one button per action, callback_data `action:{uuid}`

### Daemon Integration

- [ ] Add `mod telegram;` to nv-daemon, create `telegram/` module directory
- [ ] Add reqwest dependency to nv-daemon Cargo.toml (workspace)
- [ ] Update `main.rs`: load config + secrets, create `mpsc::channel::<Trigger>(256)`, conditionally spawn Telegram poll loop if telegram config + bot token present
- [ ] Wire `ctrl_c()` shutdown to drop trigger_tx (causes poll loop to exit)

### Unit Tests

- [ ] Test: `Update` with message parses to `InboundMessage` with correct channel, sender, content, metadata
- [ ] Test: `Update` with callback_query parses to `InboundMessage` with `[callback]` prefix, correct thread_id, callback metadata
- [ ] Test: `Update` with neither message nor callback returns `None`
- [ ] Test: chat_id filtering — only authorized chat_id passes through
- [ ] Test: `InlineKeyboard::confirm_action` produces expected 3-button layout with correct callback_data
- [ ] Test: `InlineKeyboard::from_actions` produces one row per action
- [ ] Test: `chunk_message` with short text returns single chunk
- [ ] Test: `chunk_message` with long text splits at paragraph boundary
- [ ] Test: `chunk_message` with no natural break splits at max_len

### Integration Test

- [ ] Create integration test (behind `#[cfg(feature = "integration")]` or env var gate): connect to real Telegram API, send echo message, verify delivery

### Verify

- [ ] `cargo build` passes for all workspace members
- [ ] `cargo test -p nv-daemon` — all unit tests pass
- [ ] `cargo clippy` passes with no warnings
- [ ] Manual gate: set `TELEGRAM_BOT_TOKEN` + chat_id, run daemon, send "hello" on Telegram, bot echoes back
