use std::time::Duration;

use anyhow::bail;
use reqwest::Client;

use crate::channels::discord::types::{DiscordChannel, DiscordGuild, DiscordMessage};

/// Discord maximum message length.
const DISCORD_MAX_MESSAGE_LEN: usize = 2000;

/// Discord REST API base URL.
const DISCORD_API_BASE: &str = "https://discord.com/api/v10";

/// Maximum number of rate-limit retries before giving up.
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// Thin HTTP wrapper for Discord REST API endpoints.
#[derive(Clone)]
pub struct DiscordRestClient {
    http: Client,
    token: String,
}

impl DiscordRestClient {
    /// Create a new REST client for the given bot token.
    pub fn new(bot_token: &str) -> Self {
        Self {
            http: Client::new(),
            token: bot_token.to_string(),
        }
    }

    /// Send a message to a Discord channel.
    ///
    /// Handles automatic chunking for messages over 2000 characters and
    /// rate-limit retries (HTTP 429 with Retry-After header).
    pub async fn send_message(&self, channel_id: &str, text: &str) -> anyhow::Result<()> {
        let chunks = chunk_message(text, DISCORD_MAX_MESSAGE_LEN);

        for chunk in &chunks {
            self.post_message(channel_id, chunk).await?;
        }

        Ok(())
    }

    /// List all guilds (servers) the bot is a member of.
    ///
    /// Calls `GET /users/@me/guilds` and returns a vec of `DiscordGuild`.
    pub async fn list_guilds(&self) -> anyhow::Result<Vec<DiscordGuild>> {
        let url = format!("{DISCORD_API_BASE}/users/@me/guilds");
        let resp = self.get_with_retry(&url).await?;
        let guilds: Vec<DiscordGuild> = resp.json().await?;
        Ok(guilds)
    }

    /// List text channels in a guild.
    ///
    /// Calls `GET /guilds/{guild_id}/channels`, filters to type 0 (text channels),
    /// and sorts by position ascending.
    pub async fn list_channels(&self, guild_id: &str) -> anyhow::Result<Vec<DiscordChannel>> {
        let url = format!("{DISCORD_API_BASE}/guilds/{guild_id}/channels");
        let resp = self.get_with_retry(&url).await?;
        let mut channels: Vec<DiscordChannel> = resp.json().await?;
        // Keep only text channels (type 0)
        channels.retain(|c| c.channel_type == 0);
        channels.sort_by_key(|c| c.position.unwrap_or(0));
        Ok(channels)
    }

    /// Fetch recent messages from a channel.
    ///
    /// Calls `GET /channels/{channel_id}/messages?limit={limit}`.
    /// Discord returns messages newest-first. Limit is clamped to 1..=50.
    pub async fn get_messages(
        &self,
        channel_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<DiscordMessage>> {
        let limit = limit.clamp(1, 50);
        let url = format!("{DISCORD_API_BASE}/channels/{channel_id}/messages?limit={limit}");
        let resp = self.get_with_retry(&url).await?;
        let messages: Vec<DiscordMessage> = resp.json().await?;
        Ok(messages)
    }

    /// Perform a GET request with rate-limit retry (HTTP 429 → Retry-After).
    async fn get_with_retry(&self, url: &str) -> anyhow::Result<reqwest::Response> {
        for attempt in 0..=MAX_RATE_LIMIT_RETRIES {
            let resp = self
                .http
                .get(url)
                .header("Authorization", format!("Bot {}", self.token))
                .send()
                .await?;

            let status = resp.status();

            if status.is_success() {
                return Ok(resp);
            }

            if status.as_u16() == 429 {
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);

                if attempt < MAX_RATE_LIMIT_RETRIES {
                    tracing::warn!(
                        retry_after_secs = retry_after,
                        attempt = attempt + 1,
                        "Discord rate limited on GET, retrying"
                    );
                    tokio::time::sleep(Duration::from_secs_f64(retry_after)).await;
                    continue;
                } else {
                    bail!("Discord rate limited after {MAX_RATE_LIMIT_RETRIES} retries");
                }
            }

            let error_text = resp.text().await.unwrap_or_default();
            bail!("Discord API error ({}): {}", status, error_text);
        }

        bail!("Discord get_with_retry exhausted retries")
    }

    /// POST a single message to a channel (with rate-limit retry).
    async fn post_message(&self, channel_id: &str, content: &str) -> anyhow::Result<()> {
        let url = format!("{DISCORD_API_BASE}/channels/{channel_id}/messages");
        let body = serde_json::json!({ "content": content });

        for attempt in 0..=MAX_RATE_LIMIT_RETRIES {
            let resp = self
                .http
                .post(&url)
                .header("Authorization", format!("Bot {}", self.token))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await?;

            let status = resp.status();

            if status.is_success() {
                return Ok(());
            }

            if status.as_u16() == 429 {
                // Rate limited — check Retry-After header
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);

                if attempt < MAX_RATE_LIMIT_RETRIES {
                    tracing::warn!(
                        retry_after_secs = retry_after,
                        attempt = attempt + 1,
                        "Discord rate limited, retrying"
                    );
                    tokio::time::sleep(Duration::from_secs_f64(retry_after)).await;
                    continue;
                } else {
                    bail!("Discord rate limited after {MAX_RATE_LIMIT_RETRIES} retries");
                }
            }

            let error_text = resp.text().await.unwrap_or_default();
            bail!("Discord API error ({}): {}", status, error_text);
        }

        // Should not reach here, but satisfy compiler
        bail!("Discord send_message exhausted retries")
    }
}

/// Re-export the canonical chunk_message from the shared util module.
pub use crate::channels::util::chunk_message;
