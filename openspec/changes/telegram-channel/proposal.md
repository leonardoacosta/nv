# telegram-channel

## Summary

Implement a Telegram Bot API channel adapter that satisfies the `Channel` trait. Long-polls `getUpdates`, sends messages via `sendMessage` with inline keyboard support, handles `callback_query` for action confirmations, and reconnects with exponential backoff. Spawns as a tokio task pushing `Trigger::Message` into an `mpsc::Sender<Trigger>`.

## Motivation

Telegram is the primary command and notification channel for the weekend MVP. Leo interacts with NV exclusively through Telegram — sending commands, receiving digests, and confirming actions via inline keyboards. This is the first concrete Channel implementation and proves the Channel trait design from spec-2.

## Design

### Module Structure

```
crates/nv-daemon/src/
├── main.rs
└── telegram/
    ├── mod.rs          # TelegramChannel struct + Channel impl
    ├── client.rs       # Bot API HTTP client (getUpdates, sendMessage, answerCallbackQuery)
    └── types.rs        # Telegram-specific API types (Update, Message, CallbackQuery)
```

The Telegram module lives in nv-daemon (not nv-core) because it's a runtime adapter, not a shared type.

### TelegramChannel Struct

```rust
pub struct TelegramChannel {
    client: TelegramClient,
    chat_id: i64,
    trigger_tx: mpsc::Sender<Trigger>,
    offset: Arc<AtomicI64>,  // Last processed update_id + 1
}
```

- `client`: HTTP client wrapper for Bot API calls
- `chat_id`: Authorized chat ID from config (only process messages from this chat)
- `trigger_tx`: Sender half of the daemon's trigger channel
- `offset`: Tracks the last processed `update_id` for long-poll cursor

### Channel Trait Implementation

```rust
#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str { "telegram" }

    async fn connect(&mut self) -> anyhow::Result<()> {
        // Verify bot token by calling getMe
        let me = self.client.get_me().await?;
        tracing::info!("Telegram bot connected: @{}", me.username);
        Ok(())
    }

    async fn poll_messages(&self) -> anyhow::Result<Vec<InboundMessage>> {
        // Single long-poll call, returns batch
        let updates = self.client.get_updates(self.offset.load(Ordering::Relaxed), 30).await?;
        // Convert updates to InboundMessages, advance offset
        // ...
    }

    async fn send_message(&self, msg: OutboundMessage) -> anyhow::Result<()> {
        self.client.send_message(self.chat_id, &msg.content, msg.reply_to, msg.keyboard).await
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        tracing::info!("Telegram channel disconnecting");
        Ok(())
    }
}
```

### TelegramClient (Bot API HTTP)

Thin reqwest wrapper for Telegram Bot API endpoints. All methods are `async` and return `anyhow::Result`.

#### getUpdates

```rust
pub async fn get_updates(&self, offset: i64, timeout: u64) -> anyhow::Result<Vec<Update>> {
    let url = format!("{}/getUpdates", self.base_url);
    let body = serde_json::json!({
        "offset": offset,
        "timeout": timeout,
        "allowed_updates": ["message", "callback_query"]
    });
    let resp: TelegramResponse<Vec<Update>> = self.http
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(timeout + 10)) // HTTP timeout > long-poll timeout
        .send()
        .await?
        .json()
        .await?;
    if resp.ok {
        Ok(resp.result)
    } else {
        anyhow::bail!("Telegram API error: {:?}", resp.description)
    }
}
```

- `offset`: ID of the first update to receive. Set to last `update_id + 1` after processing.
- `timeout`: Long-poll timeout in seconds (30s default). Server holds connection open until updates arrive or timeout.
- `allowed_updates`: Only receive `message` and `callback_query` — ignore edits, channel posts, etc.
- HTTP timeout is `poll_timeout + 10s` buffer to prevent reqwest from timing out before Telegram responds.

#### sendMessage

```rust
pub async fn send_message(
    &self,
    chat_id: i64,
    text: &str,
    reply_to: Option<String>,
    keyboard: Option<InlineKeyboard>,
) -> anyhow::Result<()> {
    let mut body = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "Markdown",
    });
    if let Some(reply_id) = reply_to {
        body["reply_to_message_id"] = serde_json::json!(reply_id.parse::<i64>()?);
    }
    if let Some(kb) = keyboard {
        body["reply_markup"] = serde_json::json!({
            "inline_keyboard": kb.rows.iter().map(|row| {
                row.iter().map(|btn| serde_json::json!({
                    "text": btn.text,
                    "callback_data": btn.callback_data,
                })).collect::<Vec<_>>()
            }).collect::<Vec<_>>()
        });
    }
    // POST and check response.ok
}
```

- `parse_mode`: Markdown for formatted messages (digests, action summaries)
- `reply_markup`: Converts `InlineKeyboard` to Telegram's `inline_keyboard` JSON format
- Messages longer than 4096 characters should be chunked (Telegram limit)

#### answerCallbackQuery

```rust
pub async fn answer_callback_query(&self, callback_query_id: &str, text: Option<&str>) -> anyhow::Result<()> {
    let body = serde_json::json!({
        "callback_query_id": callback_query_id,
        "text": text,
    });
    // POST to /answerCallbackQuery
}
```

Required after receiving a `callback_query` — tells Telegram to dismiss the loading spinner on the inline button.

### Telegram API Types

Minimal Telegram-specific types. Only the fields NV needs are deserialized.

```rust
#[derive(Debug, Deserialize)]
pub struct TelegramResponse<T> {
    pub ok: bool,
    pub result: T,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<TgMessage>,
    pub callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Deserialize)]
pub struct TgMessage {
    pub message_id: i64,
    pub from: Option<TgUser>,
    pub chat: TgChat,
    pub text: Option<String>,
    pub date: i64,
}

#[derive(Debug, Deserialize)]
pub struct TgUser {
    pub id: i64,
    pub first_name: String,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TgChat {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: TgUser,
    pub message: Option<TgMessage>,
    pub data: Option<String>,
}
```

### Message Parsing (Update → InboundMessage)

Each `Update` is converted to an `InboundMessage` for the unified pipeline:

```rust
impl Update {
    pub fn to_inbound_message(&self) -> Option<InboundMessage> {
        if let Some(msg) = &self.message {
            Some(InboundMessage {
                id: msg.message_id.to_string(),
                channel: "telegram".to_string(),
                sender: msg.from.as_ref()
                    .map(|u| u.username.clone().unwrap_or(u.first_name.clone()))
                    .unwrap_or_default(),
                content: msg.text.clone().unwrap_or_default(),
                timestamp: DateTime::from_timestamp(msg.date, 0)
                    .unwrap_or_else(Utc::now),
                thread_id: None,
                metadata: serde_json::json!({
                    "message_id": msg.message_id,
                    "chat_id": msg.chat.id,
                }),
            })
        } else if let Some(cb) = &self.callback_query {
            Some(InboundMessage {
                id: cb.id.clone(),
                channel: "telegram".to_string(),
                sender: cb.from.username.clone().unwrap_or(cb.from.first_name.clone()),
                content: format!("[callback] {}", cb.data.as_deref().unwrap_or("")),
                timestamp: Utc::now(),
                thread_id: cb.message.as_ref().map(|m| m.message_id.to_string()),
                metadata: serde_json::json!({
                    "callback_query_id": cb.id,
                    "callback_data": cb.data,
                    "original_message_id": cb.message.as_ref().map(|m| m.message_id),
                }),
            })
        } else {
            None
        }
    }
}
```

- Regular messages: `content` is the text, `metadata` carries `message_id` and `chat_id`
- Callback queries: `content` is prefixed with `[callback]` so the agent loop can identify confirmations. `thread_id` links to the original message. `metadata` carries `callback_query_id` for answering.

### Long-Poll Loop

The Telegram listener runs as a spawned tokio task. It continuously polls for updates and pushes triggers into the mpsc channel.

```rust
pub async fn run_poll_loop(channel: TelegramChannel) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(60);

    loop {
        match channel.poll_messages().await {
            Ok(messages) => {
                backoff = Duration::from_secs(1); // Reset on success
                for msg in messages {
                    if let Err(e) = channel.trigger_tx.send(Trigger::Message(msg)).await {
                        tracing::error!("Failed to send trigger: {e}");
                        return; // Receiver dropped, daemon shutting down
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Telegram poll error: {e}, retrying in {backoff:?}");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}
```

**Backoff strategy:**
- Start at 1 second
- Double on each consecutive failure
- Cap at 60 seconds
- Reset to 1 second on any successful poll
- If `trigger_tx.send()` fails, the receiver has been dropped — the daemon is shutting down, so the loop exits

### Chat ID Authorization

Only messages from the configured `chat_id` are processed. All others are silently dropped. This prevents unauthorized users from interacting with NV.

```rust
// In poll_messages, after getting updates:
let authorized: Vec<InboundMessage> = updates.iter()
    .filter(|u| {
        let chat = u.message.as_ref().map(|m| m.chat.id)
            .or_else(|| u.callback_query.as_ref()
                .and_then(|cb| cb.message.as_ref())
                .map(|m| m.chat.id));
        chat == Some(self.chat_id)
    })
    .filter_map(|u| u.to_inbound_message())
    .collect();
```

### Inline Keyboard Builder

Utility function for constructing common keyboard layouts used by the agent loop.

```rust
impl InlineKeyboard {
    /// Standard action confirmation keyboard
    pub fn confirm_action(action_id: &str) -> Self {
        Self {
            rows: vec![vec![
                InlineButton {
                    text: "Approve".to_string(),
                    callback_data: format!("approve:{action_id}"),
                },
                InlineButton {
                    text: "Edit".to_string(),
                    callback_data: format!("edit:{action_id}"),
                },
                InlineButton {
                    text: "Cancel".to_string(),
                    callback_data: format!("cancel:{action_id}"),
                },
            ]],
        }
    }

    /// Digest suggested actions keyboard
    pub fn from_actions(actions: &[PendingAction]) -> Self {
        Self {
            rows: actions.iter().map(|a| {
                vec![InlineButton {
                    text: a.description.clone(),
                    callback_data: format!("action:{}", a.id),
                }]
            }).collect(),
        }
    }
}
```

### Integration with nv-daemon main.rs

The daemon wires up the Telegram channel at startup:

```rust
// In main.rs (updated from spec-1 skeleton):
let config = Config::load()?;
let secrets = Secrets::from_env()?;

let (trigger_tx, trigger_rx) = mpsc::channel::<Trigger>(256);

if let (Some(tg_config), Some(bot_token)) = (&config.telegram, &secrets.telegram_bot_token) {
    let tg_channel = TelegramChannel::new(
        bot_token.clone(),
        tg_config.chat_id,
        trigger_tx.clone(),
    );
    tokio::spawn(async move {
        run_poll_loop(tg_channel).await;
    });
    tracing::info!("Telegram channel started");
}
```

### Message Length Handling

Telegram has a 4096 character limit per message. Messages exceeding this are split at paragraph boundaries (double newline) or at the limit if no natural break exists.

```rust
pub fn chunk_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }
        // Find split point: prefer paragraph break, then line break, then hard cut
        let split_at = remaining[..max_len]
            .rfind("\n\n")
            .or_else(|| remaining[..max_len].rfind('\n'))
            .unwrap_or(max_len);
        chunks.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..].trim_start();
    }
    chunks
}
```

## Verification

- `cargo build` succeeds
- `cargo test` — unit tests for message parsing, keyboard building, message chunking
- Integration test: set `TELEGRAM_BOT_TOKEN` and `NV_TEST_CHAT_ID`, send "hello" to bot, bot echoes back (manual gate)
