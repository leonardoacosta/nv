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

/// Split a message into chunks that fit within `max_len`.
///
/// Prefers splitting at paragraph boundaries (`\n\n`), then line boundaries
/// (`\n`), and falls back to a hard cut at `max_len`.
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

        // Avoid zero-length splits
        let split_at = if split_at == 0 { max_len } else { split_at };

        chunks.push(remaining[..split_at].to_string());
        remaining = remaining[split_at..].trim_start();
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_short_message_single_chunk() {
        let text = "Hello, world!";
        let chunks = chunk_message(text, DISCORD_MAX_MESSAGE_LEN);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn chunk_at_discord_limit() {
        let text = "A".repeat(2000);
        let chunks = chunk_message(&text, DISCORD_MAX_MESSAGE_LEN);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 2000);
    }

    #[test]
    fn chunk_over_discord_limit() {
        let text = "A".repeat(2001);
        let chunks = chunk_message(&text, DISCORD_MAX_MESSAGE_LEN);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 2000);
        assert_eq!(chunks[1].len(), 1);
    }

    #[test]
    fn chunk_splits_at_paragraph() {
        let para1 = "A".repeat(1500);
        let para2 = "B".repeat(1500);
        let text = format!("{para1}\n\n{para2}");
        let chunks = chunk_message(&text, DISCORD_MAX_MESSAGE_LEN);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], para1);
        assert_eq!(chunks[1], para2);
    }

    #[test]
    fn chunk_splits_at_line() {
        let line1 = "A".repeat(1500);
        let line2 = "B".repeat(1500);
        let text = format!("{line1}\n{line2}");
        let chunks = chunk_message(&text, DISCORD_MAX_MESSAGE_LEN);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], line1);
        assert_eq!(chunks[1], line2);
    }

    #[test]
    fn chunk_hard_cut_no_breaks() {
        let text = "A".repeat(5000);
        let chunks = chunk_message(&text, DISCORD_MAX_MESSAGE_LEN);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 2000);
        assert_eq!(chunks[1].len(), 2000);
        assert_eq!(chunks[2].len(), 1000);
    }

    #[test]
    fn chunk_empty_message() {
        let chunks = chunk_message("", DISCORD_MAX_MESSAGE_LEN);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }
}
