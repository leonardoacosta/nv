use async_trait::async_trait;

use crate::types::{InboundMessage, OutboundMessage};

/// The adapter contract for message channels.
///
/// Each channel (Telegram, Discord, Teams, etc.) implements this trait.
/// Channels are spawned as tokio tasks; `poll_messages` returns a batch
/// of messages since the last poll.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Human-readable channel name (e.g., "telegram", "discord").
    fn name(&self) -> &str;

    /// Establish connection to the channel service.
    async fn connect(&mut self) -> anyhow::Result<()>;

    /// Poll for new messages since last check.
    /// Returns an empty vec if none.
    async fn poll_messages(&self) -> anyhow::Result<Vec<InboundMessage>>;

    /// Send a message through this channel.
    async fn send_message(&self, msg: OutboundMessage) -> anyhow::Result<()>;

    /// Gracefully disconnect from the channel service.
    async fn disconnect(&mut self) -> anyhow::Result<()>;

    /// Downcast support for channel-specific operations (e.g., Telegram message editing).
    fn as_any(&self) -> &dyn std::any::Any;
}
