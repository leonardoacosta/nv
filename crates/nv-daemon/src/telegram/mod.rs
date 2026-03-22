pub mod client;
pub mod types;

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use nv_core::channel::Channel;
use nv_core::types::{InboundMessage, OutboundMessage, Trigger};
use tokio::sync::mpsc;

use self::client::TelegramClient;

/// Telegram Bot API channel adapter.
///
/// Implements the `Channel` trait from nv-core. Long-polls `getUpdates`,
/// sends messages via `sendMessage` with inline keyboard support, and
/// handles `callback_query` for action confirmations.
pub struct TelegramChannel {
    pub client: TelegramClient,
    pub chat_id: i64,
    trigger_tx: mpsc::Sender<Trigger>,
    offset: Arc<AtomicI64>,
}

impl TelegramChannel {
    /// Create a new Telegram channel.
    ///
    /// - `bot_token`: The Telegram bot token from `TELEGRAM_BOT_TOKEN` env var.
    /// - `chat_id`: The authorized chat ID from config.
    /// - `trigger_tx`: Sender half of the daemon's trigger channel.
    pub fn new(bot_token: &str, chat_id: i64, trigger_tx: mpsc::Sender<Trigger>) -> Self {
        Self {
            client: TelegramClient::new(bot_token),
            chat_id,
            trigger_tx,
            offset: Arc::new(AtomicI64::new(0)),
        }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        let me = self.client.get_me().await?;
        tracing::info!(
            "Telegram bot connected: @{}",
            me.username.as_deref().unwrap_or(&me.first_name)
        );
        Ok(())
    }

    async fn poll_messages(&self) -> anyhow::Result<Vec<InboundMessage>> {
        let current_offset = self.offset.load(Ordering::Relaxed);
        let updates = self.client.get_updates(current_offset, 30).await?;

        if let Some(max_id) = updates.iter().map(|u| u.update_id).max() {
            self.offset.store(max_id + 1, Ordering::Relaxed);
        }

        // Filter by authorized chat_id and convert to InboundMessage
        let messages: Vec<InboundMessage> = updates
            .iter()
            .filter(|u| u.chat_id() == Some(self.chat_id))
            .filter_map(|u| u.to_inbound_message())
            .collect();

        // Answer callback queries for authorized updates
        for update in &updates {
            if update.chat_id() == Some(self.chat_id) {
                if let Some(cb) = &update.callback_query {
                    if let Err(e) = self.client.answer_callback_query(&cb.id, None).await {
                        tracing::warn!("Failed to answer callback query: {e}");
                    }
                }
            }
        }

        Ok(messages)
    }

    async fn send_message(&self, msg: OutboundMessage) -> anyhow::Result<()> {
        self.client
            .send_message(self.chat_id, &msg.content, msg.reply_to, msg.keyboard.as_ref())
            .await?;
        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        tracing::info!("Telegram channel disconnecting");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ── Long-Poll Loop ─────────────────────────────────────────────────

/// Run the continuous long-poll loop as a tokio task.
///
/// Polls for updates and pushes `Trigger::Message` into the mpsc channel.
/// Uses exponential backoff on failure (1s to 60s).
/// Exits when the trigger receiver is dropped (daemon shutting down).
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

// ── Inline Keyboard Builders ───────────────────────────────────────
//
// Builder methods (confirm_action, from_actions) are defined on
// InlineKeyboard in nv-core::types since InlineKeyboard is owned by
// that crate.

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use nv_core::types::{ActionStatus, ActionType, InlineKeyboard, PendingAction};
    use uuid::Uuid;

    #[test]
    fn confirm_action_keyboard_layout() {
        let kb = InlineKeyboard::confirm_action("abc-123");
        assert_eq!(kb.rows.len(), 1);
        assert_eq!(kb.rows[0].len(), 3);

        assert_eq!(kb.rows[0][0].text, "Approve");
        assert_eq!(kb.rows[0][0].callback_data, "approve:abc-123");

        assert_eq!(kb.rows[0][1].text, "Edit");
        assert_eq!(kb.rows[0][1].callback_data, "edit:abc-123");

        assert_eq!(kb.rows[0][2].text, "Cancel");
        assert_eq!(kb.rows[0][2].callback_data, "cancel:abc-123");
    }

    #[test]
    fn from_actions_keyboard_one_row_per_action() {
        let actions = vec![
            PendingAction {
                id: Uuid::new_v4(),
                description: "Create ticket".to_string(),
                action_type: ActionType::JiraCreate,
                payload: serde_json::json!({}),
                status: ActionStatus::Pending,
                created_at: Utc::now(),
            },
            PendingAction {
                id: Uuid::new_v4(),
                description: "Assign to Leo".to_string(),
                action_type: ActionType::JiraAssign,
                payload: serde_json::json!({}),
                status: ActionStatus::Pending,
                created_at: Utc::now(),
            },
        ];

        let kb = InlineKeyboard::from_actions(&actions);
        assert_eq!(kb.rows.len(), 2);
        assert_eq!(kb.rows[0].len(), 1);
        assert_eq!(kb.rows[1].len(), 1);

        assert_eq!(kb.rows[0][0].text, "Create ticket");
        assert!(kb.rows[0][0].callback_data.starts_with("action:"));

        assert_eq!(kb.rows[1][0].text, "Assign to Leo");
        assert!(kb.rows[1][0].callback_data.starts_with("action:"));
    }

    #[test]
    fn from_actions_empty_list() {
        let kb = InlineKeyboard::from_actions(&[]);
        assert!(kb.rows.is_empty());
    }
}
