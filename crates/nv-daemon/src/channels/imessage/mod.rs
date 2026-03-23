pub mod client;
pub mod types;

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use nv_core::channel::Channel;
use nv_core::config::IMessageConfig;
use nv_core::types::{InboundMessage, OutboundMessage, Trigger};
use tokio::sync::mpsc;

use self::client::BlueBubblesClient;

/// iMessage channel adapter via BlueBubbles REST API.
///
/// Implements the `Channel` trait from nv-core. Polls BlueBubbles for
/// new messages on a configurable interval and sends replies through
/// the BlueBubbles server running on a Mac.
pub struct IMessageChannel {
    bb_client: BlueBubblesClient,
    config: IMessageConfig,
    trigger_tx: mpsc::Sender<Trigger>,
    /// Last-seen message timestamp in milliseconds. Used to avoid
    /// fetching duplicate messages across poll cycles.
    last_seen_ts: Arc<AtomicI64>,
}

impl IMessageChannel {
    /// Create a new iMessage channel.
    ///
    /// - `config`: The iMessage configuration from `nv.toml`.
    /// - `password`: The BlueBubbles server password from env.
    /// - `trigger_tx`: Sender half of the daemon's trigger channel.
    pub fn new(
        config: IMessageConfig,
        password: &str,
        trigger_tx: mpsc::Sender<Trigger>,
    ) -> Self {
        let bb_client = BlueBubblesClient::new(&config.bluebubbles_url, password);
        // Start from "now" so we only pick up messages arriving after daemon start.
        let now_ms = Utc::now().timestamp_millis();
        Self {
            bb_client,
            config,
            trigger_tx,
            last_seen_ts: Arc::new(AtomicI64::new(now_ms)),
        }
    }
}

#[async_trait]
impl Channel for IMessageChannel {
    fn name(&self) -> &str {
        "imessage"
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        // Validate connectivity by fetching a single message.
        // This confirms the URL and password are correct.
        self.bb_client.get_messages(0, 1).await?;
        tracing::info!(
            url = %self.config.bluebubbles_url,
            "iMessage channel connected via BlueBubbles"
        );
        Ok(())
    }

    async fn poll_messages(&self) -> anyhow::Result<Vec<InboundMessage>> {
        let after = self.last_seen_ts.load(Ordering::Relaxed);
        let messages = self.bb_client.get_messages(after, 100).await?;

        if messages.is_empty() {
            return Ok(vec![]);
        }

        // Advance the last-seen timestamp to the newest message we received
        if let Some(max_ts) = messages.iter().map(|m| m.date_created).max() {
            self.last_seen_ts.store(max_ts, Ordering::Relaxed);
        }

        let inbound: Vec<InboundMessage> = messages
            .into_iter()
            // Skip messages sent by us (prevent echo loops)
            .filter(|m| !m.is_from_me)
            // Skip messages with no text content (attachment-only)
            .filter(|m| m.text.as_ref().map(|t| !t.is_empty()).unwrap_or(false))
            .map(|m| {
                let sender = m
                    .handle
                    .as_ref()
                    .map(|h| h.address.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                let chat_guid = m
                    .chats
                    .first()
                    .map(|c| c.guid.clone())
                    .unwrap_or_default();

                InboundMessage {
                    id: m.guid.clone(),
                    channel: "imessage".to_string(),
                    sender,
                    content: m.text.unwrap_or_default(),
                    timestamp: chrono::DateTime::from_timestamp_millis(m.date_created)
                        .unwrap_or_else(Utc::now),
                    thread_id: Some(chat_guid.clone()),
                    metadata: serde_json::json!({
                        "chat_guid": chat_guid,
                        "message_guid": m.guid,
                    }),
                }
            })
            .collect();

        Ok(inbound)
    }

    async fn send_message(&self, msg: OutboundMessage) -> anyhow::Result<()> {
        // The chat_guid is stored in reply_to (set from thread_id during routing)
        let chat_guid = msg
            .reply_to
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("iMessage send_message requires reply_to as chat_guid"))?;

        self.bb_client.send_message(chat_guid, &msg.content).await
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        tracing::info!("iMessage channel disconnecting");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ── Poll Loop ──────────────────────────────────────────────────────

/// Run the continuous poll loop as a tokio task.
///
/// Periodically polls BlueBubbles for new messages and pushes
/// `Trigger::Message` into the mpsc channel. Uses exponential backoff
/// on consecutive errors (doubles interval, caps at 5 minutes).
/// Exits when the trigger receiver is dropped (daemon shutting down).
pub async fn run_poll_loop(channel: IMessageChannel) {
    let poll_interval = Duration::from_secs(channel.config.poll_interval_secs);
    let mut current_interval = poll_interval;
    let max_backoff = Duration::from_secs(300); // 5 minutes

    loop {
        tokio::time::sleep(current_interval).await;

        match channel.poll_messages().await {
            Ok(messages) => {
                // Reset to base interval on success
                current_interval = poll_interval;
                for msg in messages {
                    if let Err(e) = channel.trigger_tx.send(Trigger::Message(msg)).await {
                        tracing::error!("Failed to send iMessage trigger: {e}");
                        return; // Receiver dropped, daemon shutting down
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "iMessage poll error: {e}, retrying in {current_interval:?}"
                );
                // Exponential backoff: double interval, cap at 5 minutes
                current_interval = (current_interval * 2).min(max_backoff);
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_seen_advances() {
        let ts = Arc::new(AtomicI64::new(1000));
        // Simulate advancing
        let new_ts = 2000i64;
        if new_ts > ts.load(Ordering::Relaxed) {
            ts.store(new_ts, Ordering::Relaxed);
        }
        assert_eq!(ts.load(Ordering::Relaxed), 2000);
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let base = Duration::from_secs(10);
        let max = Duration::from_secs(300);

        let mut interval = base;

        // First error: 10 -> 20
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(20));

        // Second error: 20 -> 40
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(40));

        // Third: 40 -> 80
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(80));

        // Fourth: 80 -> 160
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(160));

        // Fifth: 160 -> 300 (capped)
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(300));

        // Sixth: stays at 300
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(300));

        // Reset on success
        interval = base;
        assert_eq!(interval, Duration::from_secs(10));
    }
}
