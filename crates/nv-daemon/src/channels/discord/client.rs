use std::time::Duration;

use anyhow::bail;
use reqwest::Client;

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
