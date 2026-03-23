pub mod client;
pub mod gateway;
pub mod types;

use std::sync::atomic::Ordering;
use std::time::Duration;

use async_trait::async_trait;
use nv_core::channel::Channel;
use nv_core::config::DiscordConfig;
use nv_core::types::{InboundMessage, OutboundMessage, Trigger};
use tokio::sync::mpsc;

use self::client::DiscordRestClient;
use self::gateway::GatewayConnection;

/// Discord channel adapter.
///
/// Implements the `Channel` trait from nv-core. Connects to Discord's gateway
/// WebSocket for real-time MESSAGE_CREATE events, sends replies via REST API.
pub struct DiscordChannel {
    gateway: GatewayConnection,
    rest: DiscordRestClient,
    config: DiscordConfig,
    trigger_tx: mpsc::Sender<Trigger>,
}

impl DiscordChannel {
    /// Create a new Discord channel.
    ///
    /// - `bot_token`: The Discord bot token from `DISCORD_BOT_TOKEN` env var.
    /// - `config`: The Discord configuration (watched servers/channels).
    /// - `trigger_tx`: Sender half of the daemon's trigger channel.
    pub fn new(bot_token: &str, config: DiscordConfig, trigger_tx: mpsc::Sender<Trigger>) -> Self {
        Self {
            gateway: GatewayConnection::new(bot_token),
            rest: DiscordRestClient::new(bot_token),
            config,
            trigger_tx,
        }
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        self.gateway.connect().await?;
        tracing::info!("Discord channel connected via gateway");
        Ok(())
    }

    async fn poll_messages(&self) -> anyhow::Result<Vec<InboundMessage>> {
        let raw_messages = self.gateway.drain_messages().await;

        if raw_messages.is_empty() {
            return Ok(vec![]);
        }

        let bot_user_id = self.gateway.bot_user_id.lock().await.clone();

        let messages: Vec<InboundMessage> = raw_messages
            .into_iter()
            .filter(|m| {
                // Ignore messages from the bot itself (prevent echo loops)
                if let Some(ref bot_id) = bot_user_id {
                    if m.author.id == *bot_id {
                        return false;
                    }
                }
                true
            })
            .filter(|m| {
                // Filter by watched channel IDs
                if self.config.channel_ids.is_empty() {
                    return true; // No filter configured — accept all
                }
                m.channel_id
                    .parse::<u64>()
                    .map(|id| self.config.channel_ids.contains(&id))
                    .unwrap_or(false)
            })
            .filter(|m| {
                // Filter by watched server (guild) IDs
                // DMs (guild_id = None) are always accepted
                if self.config.server_ids.is_empty() {
                    return true; // No filter configured — accept all
                }
                match &m.guild_id {
                    Some(gid) => gid
                        .parse::<u64>()
                        .map(|id| self.config.server_ids.contains(&id))
                        .unwrap_or(false),
                    None => true, // DMs pass through
                }
            })
            .map(|m| m.to_inbound_message())
            .collect();

        Ok(messages)
    }

    async fn send_message(&self, msg: OutboundMessage) -> anyhow::Result<()> {
        // Extract channel_id from the outbound message metadata or reply_to
        // For Discord, the channel_id should be passed via reply_to or metadata
        let channel_id = msg
            .reply_to
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Discord send_message requires reply_to as channel_id"))?;

        self.rest.send_message(channel_id, &msg.content).await
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        tracing::info!("Discord channel disconnecting");
        self.gateway.close().await?;
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ── Poll Loop ──────────────────────────────────────────────────────

/// Run the continuous poll loop as a tokio task.
///
/// Periodically drains the gateway's message buffer and pushes
/// `Trigger::Message` into the mpsc channel.
///
/// When the gateway WebSocket disconnects (event loop exits), `is_connected`
/// is cleared and this loop reconnects with exponential backoff:
/// 1s → 2s → 4s → 8s → 16s → 32s → 60s (capped), with jitter.
pub async fn run_poll_loop(mut channel: DiscordChannel) {
    let mut reconnect_backoff = Duration::from_secs(1);
    let max_reconnect_backoff = Duration::from_secs(60);
    let poll_interval = Duration::from_millis(500);

    loop {
        // Sleep briefly to avoid busy-spinning — gateway events are buffered
        tokio::time::sleep(poll_interval).await;

        // Check if the gateway WebSocket is still live; reconnect if not.
        if !channel.gateway.is_connected.load(Ordering::Relaxed) {
            tracing::warn!(
                backoff_secs = reconnect_backoff.as_secs(),
                "Discord gateway disconnected — reconnecting"
            );
            tokio::time::sleep(reconnect_backoff).await;

            use nv_core::channel::Channel as _;
            match channel.connect().await {
                Ok(()) => {
                    tracing::info!("Discord gateway reconnected");
                    reconnect_backoff = Duration::from_secs(1); // Reset on success
                }
                Err(e) => {
                    tracing::error!(error = %e, "Discord gateway reconnect failed");
                    reconnect_backoff = (reconnect_backoff * 2).min(max_reconnect_backoff);
                }
            }
            continue;
        }

        // Reset reconnect backoff after a successful connected poll cycle
        reconnect_backoff = Duration::from_secs(1);

        match channel.poll_messages().await {
            Ok(messages) => {
                for msg in messages {
                    if let Err(e) = channel.trigger_tx.send(Trigger::Message(msg)).await {
                        tracing::error!("Failed to send trigger: {e}");
                        return; // Receiver dropped, daemon shutting down
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Discord poll error: {e}");
            }
        }
    }
}
