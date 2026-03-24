# Tasks: telegram-channel

## Dependencies

- core-types-and-config (spec-2)

## Tasks

### Telegram API Types

- [x] Create `crates/nv-daemon/src/telegram/types.rs` with `TelegramResponse<T>`, `Update`, `TgMessage`, `TgUser`, `TgChat`, `CallbackQuery` — all `#[derive(Debug, Deserialize)]`
- [x] Implement `Update::to_inbound_message()` — converts regular messages (text → InboundMessage with message_id/chat_id in metadata)
- [x] Handle `callback_query` in `to_inbound_message()` — content prefixed `[callback]`, thread_id links to original message, metadata carries callback_query_id and callback_data

### Bot API Client

- [x] Create `crates/nv-daemon/src/telegram/client.rs` with `TelegramClient` struct (reqwest::Client + base_url from bot token)
- [x] Implement `TelegramClient::new(bot_token: &str)` — constructs base URL `https://api.telegram.org/bot{token}`
- [x] Implement `get_me()` — GET `/getMe`, verify bot token is valid, return bot username
- [x] Implement `get_updates(offset: i64, timeout: u64)` — POST `/getUpdates` with offset, timeout, allowed_updates `["message", "callback_query"]`. HTTP timeout = poll timeout + 10s buffer.
- [x] Implement `send_message(chat_id, text, reply_to, keyboard)` — POST `/sendMessage` with `parse_mode: "Markdown"`, optional `reply_to_message_id`, optional `reply_markup` (inline_keyboard JSON)
- [x] Implement `answer_callback_query(callback_query_id, text)` — POST `/answerCallbackQuery` to dismiss inline button loading spinner
- [x] Implement `chunk_message(text, max_len)` — split messages at paragraph/line boundaries when exceeding 4096 char Telegram limit

### TelegramChannel (Channel Trait)

- [x] Create `crates/nv-daemon/src/telegram/mod.rs` with `TelegramChannel` struct: `TelegramClient`, `chat_id: i64`, `trigger_tx: mpsc::Sender<Trigger>`, `offset: Arc<AtomicI64>`
- [x] Implement `TelegramChannel::new(bot_token, chat_id, trigger_tx)` — initialize client and offset at 0
- [x] Implement `Channel::name()` — returns `"telegram"`
- [x] Implement `Channel::connect()` — call `get_me()` to verify token, log bot username
- [x] Implement `Channel::poll_messages()` — call `get_updates` with current offset, filter by authorized `chat_id`, convert to `InboundMessage` vec, advance offset to `max(update_id) + 1`
- [x] Implement `Channel::send_message()` — delegate to `TelegramClient::send_message`, handle message chunking for long content
- [x] Implement `Channel::disconnect()` — log disconnection, no cleanup needed

### Long-Poll Loop

- [x] Implement `run_poll_loop(channel: TelegramChannel)` — continuous loop: poll → push triggers → handle errors
- [x] Implement exponential backoff on poll failure: start 1s, double on consecutive failures, cap at 60s, reset on success
- [x] Exit loop when `trigger_tx.send()` fails (receiver dropped = daemon shutting down)
- [x] Filter updates by `chat_id` — silently drop messages from unauthorized chats

### Inline Keyboard Builder

- [x] Implement `InlineKeyboard::confirm_action(action_id)` — 3-button row: Approve (`approve:{id}`), Edit (`edit:{id}`), Cancel (`cancel:{id}`)
- [x] Implement `InlineKeyboard::from_actions(actions: &[PendingAction])` — one button per action, callback_data `action:{uuid}`

### Daemon Integration

- [x] Add `mod telegram;` to nv-daemon, create `telegram/` module directory
- [x] Add reqwest dependency to nv-daemon Cargo.toml (workspace)
- [x] Update `main.rs`: load config + secrets, create `mpsc::channel::<Trigger>(256)`, conditionally spawn Telegram poll loop if telegram config + bot token present
- [x] Wire `ctrl_c()` shutdown to drop trigger_tx (causes poll loop to exit)

### Unit Tests

- [x] Test: `Update` with message parses to `InboundMessage` with correct channel, sender, content, metadata
- [x] Test: `Update` with callback_query parses to `InboundMessage` with `[callback]` prefix, correct thread_id, callback metadata
- [x] Test: `Update` with neither message nor callback returns `None`
- [x] Test: chat_id filtering — only authorized chat_id passes through
- [x] Test: `InlineKeyboard::confirm_action` produces expected 3-button layout with correct callback_data
- [x] Test: `InlineKeyboard::from_actions` produces one row per action
- [x] Test: `chunk_message` with short text returns single chunk
- [x] Test: `chunk_message` with long text splits at paragraph boundary
- [x] Test: `chunk_message` with no natural break splits at max_len

### Integration Test

- [ ] Create integration test (behind `#[cfg(feature = "integration")]` or env var gate): connect to real Telegram API, send echo message, verify delivery

### Verify

- [x] `cargo build` passes for all workspace members
- [x] `cargo test -p nv-daemon` — all unit tests pass
- [x] `cargo clippy` passes with no warnings
- [ ] Manual gate: set `TELEGRAM_BOT_TOKEN` + chat_id, run daemon, send "hello" on Telegram, bot echoes back
