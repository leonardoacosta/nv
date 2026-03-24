pub mod client;
pub mod oauth;
pub mod types;

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use nv_core::channel::Channel;
use nv_core::config::TeamsConfig;
use nv_core::types::{InboundMessage, OutboundMessage, Trigger};
use tokio::sync::{mpsc, Mutex};

use self::client::TeamsClient;
use self::oauth::MsGraphAuth;
use self::types::ChatMessage;

/// Teams channel adapter.
///
/// Implements the `Channel` trait from nv-core. Receives messages via
/// MS Graph subscription webhooks (pushed to buffer by the HTTP handler),
/// sends replies via the MS Graph REST API.
pub struct TeamsChannel {
    auth: Arc<MsGraphAuth>,
    client: TeamsClient,
    config: TeamsConfig,
    trigger_tx: mpsc::Sender<Trigger>,
    /// Shared buffer for inbound webhook messages.
    /// The axum webhook handler pushes here; `poll_messages()` drains.
    pub message_buffer: Arc<Mutex<VecDeque<ChatMessage>>>,
    /// Webhook notification URL for subscription registration.
    webhook_url: String,
    /// Active subscription IDs (for cleanup on disconnect).
    subscription_ids: Mutex<Vec<String>>,
    /// Client state secret for validating webhook notifications.
    pub client_state: String,
}

impl TeamsChannel {
    /// Create a new Teams channel.
    ///
    /// - `tenant_id`: Azure AD tenant ID.
    /// - `client_id`: Azure AD app client ID.
    /// - `client_secret`: Azure AD app client secret.
    /// - `config`: Teams configuration (team_ids, channel_ids).
    /// - `trigger_tx`: Sender for the daemon's trigger channel.
    /// - `webhook_url`: Publicly reachable HTTPS URL for webhook notifications.
    pub fn new(
        tenant_id: &str,
        client_id: &str,
        client_secret: &str,
        config: TeamsConfig,
        trigger_tx: mpsc::Sender<Trigger>,
        webhook_url: String,
    ) -> Self {
        let auth = Arc::new(MsGraphAuth::new(tenant_id, client_id, client_secret));
        let client = TeamsClient::new(Arc::clone(&auth));
        let client_state = format!("nv-{}", uuid::Uuid::new_v4());

        Self {
            auth,
            client,
            config,
            trigger_tx,
            message_buffer: Arc::new(Mutex::new(VecDeque::new())),
            webhook_url,
            subscription_ids: Mutex::new(Vec::new()),
            client_state,
        }
    }

    /// Register subscriptions for all watched channels.
    async fn register_subscriptions(&self) -> anyhow::Result<()> {
        let expiration = subscription_expiration();
        let mut sub_ids = self.subscription_ids.lock().await;

        // MS Graph per-channel subscriptions require delegated permissions which
        // aren't available in client credentials flow. Use a single getAllMessages
        // subscription with application permissions and filter in poll_messages().
        let resource = "teams/getAllMessages".to_string();

        match self
            .client
            .create_subscription(
                &self.webhook_url,
                &resource,
                &expiration,
                Some(&self.client_state),
            )
            .await
        {
            Ok(sub) => {
                sub_ids.push(sub.id);
                tracing::info!("Teams subscription registered for getAllMessages");
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to register Teams subscription");
                return Err(e);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for TeamsChannel {
    fn name(&self) -> &str {
        "teams"
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        // Step 1: Acquire OAuth token
        self.auth.authenticate().await?;
        tracing::info!("Teams OAuth authenticated");

        // Step 2: Register webhook subscriptions
        self.register_subscriptions().await?;

        // Step 3: Spawn background renewal task so subscriptions don't expire after 60 min
        let sub_ids = self.subscription_ids.lock().await.clone();
        spawn_subscription_renewal(&self.client, Arc::clone(&self.auth), sub_ids);

        tracing::info!("Teams channel connected");
        Ok(())
    }

    async fn poll_messages(&self) -> anyhow::Result<Vec<InboundMessage>> {
        let mut buffer = self.message_buffer.lock().await;
        if buffer.is_empty() {
            return Ok(vec![]);
        }

        let messages: Vec<InboundMessage> = buffer
            .drain(..)
            .filter(|m| {
                // Filter out system messages (only process user messages)
                m.message_type
                    .as_deref()
                    .map(|mt| mt == "message")
                    .unwrap_or(true)
            })
            .filter(|m| {
                // Filter by watched channel IDs if configured
                if self.config.channel_ids.is_empty() {
                    return true;
                }
                m.channel_identity
                    .as_ref()
                    .map(|ci| self.config.channel_ids.contains(&ci.channel_id))
                    .unwrap_or(false)
            })
            .map(|m| m.to_inbound_message())
            .collect();

        Ok(messages)
    }

    async fn send_message(&self, msg: OutboundMessage) -> anyhow::Result<()> {
        // Extract routing info from metadata or reply_to
        // Format: "team_id:channel_id" or just "chat_id" for direct messages
        let reply_to = msg
            .reply_to
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Teams send_message requires reply_to for routing"))?;

        if let Some((team_id, channel_id)) = reply_to.split_once(':') {
            self.client
                .send_channel_message(team_id, channel_id, &msg.content)
                .await
        } else {
            // Treat as chat_id for direct messages
            self.client
                .send_chat_message(reply_to, &msg.content)
                .await
        }
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        tracing::info!("Teams channel disconnecting");

        // Delete all active subscriptions
        let sub_ids = self.subscription_ids.lock().await.clone();
        for sub_id in &sub_ids {
            if let Err(e) = self.client.delete_subscription(sub_id).await {
                tracing::warn!(subscription_id = %sub_id, error = %e, "Failed to delete subscription");
            }
        }
        self.subscription_ids.lock().await.clear();

        // Clear OAuth token
        self.auth.clear_token().await;

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ── Subscription Renewal ─────────────────────────────────────────

/// Spawn a background task that renews the Teams subscription before expiry.
///
/// MS Graph channel message subscriptions expire after 60 minutes max.
/// This task renews every 50 minutes (subscription max is 60 min).
pub fn spawn_subscription_renewal(
    _client: &TeamsClient,
    auth: Arc<MsGraphAuth>,
    subscription_ids: Vec<String>,
) {
    let renew_client = TeamsClient::new(auth);
    let ids = subscription_ids;

    tokio::spawn(async move {
        // Renew every 50 minutes (subscription max is 60 min)
        let mut interval = tokio::time::interval(Duration::from_secs(50 * 60));
        interval.tick().await; // Skip immediate tick

        loop {
            interval.tick().await;
            let expiration = subscription_expiration();

            for sub_id in &ids {
                match renew_client.renew_subscription(sub_id, &expiration).await {
                    Ok(()) => {
                        tracing::info!(subscription_id = %sub_id, "Teams subscription renewed");
                    }
                    Err(e) => {
                        tracing::error!(
                            subscription_id = %sub_id,
                            error = %e,
                            "Failed to renew Teams subscription"
                        );
                    }
                }
            }
        }
    });
}

/// Calculate subscription expiration time (55 minutes from now).
fn subscription_expiration() -> String {
    let expiration = chrono::Utc::now() + chrono::Duration::minutes(55);
    expiration.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ── Poll Loop ────────────────────────────────────────────────────

/// Run the continuous poll loop as a tokio task.
///
/// Periodically drains the webhook message buffer and pushes
/// `Trigger::Message` into the mpsc channel. Uses exponential backoff
/// on failure (1s to 60s).
pub async fn run_poll_loop(channel: TeamsChannel) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(60);
    let poll_interval = Duration::from_millis(500);

    loop {
        tokio::time::sleep(poll_interval).await;

        match channel.poll_messages().await {
            Ok(messages) => {
                backoff = Duration::from_secs(1);
                for msg in messages {
                    if let Err(e) = channel.trigger_tx.send(Trigger::Message(msg)).await {
                        tracing::error!("Failed to send trigger: {e}");
                        return; // Receiver dropped, daemon shutting down
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Teams poll error: {e}, retrying in {backoff:?}");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}
