use std::time::Duration;

use anyhow::bail;
use reqwest::Client;

use super::types::{BotUser, TelegramResponse, Update};

/// Telegram Bot API maximum message length.
const TELEGRAM_MAX_MESSAGE_LEN: usize = 4096;

/// Thin HTTP wrapper for Telegram Bot API endpoints.
#[derive(Clone)]
pub struct TelegramClient {
    http: Client,
    base_url: String,
}

impl TelegramClient {
    /// Create a new client for the given bot token.
    ///
    /// Constructs the base URL `https://api.telegram.org/bot{token}`.
    pub fn new(bot_token: &str) -> Self {
        Self {
            http: Client::new(),
            base_url: format!("https://api.telegram.org/bot{bot_token}"),
        }
    }

    /// Verify the bot token by calling `getMe`.
    pub async fn get_me(&self) -> anyhow::Result<BotUser> {
        let url = format!("{}/getMe", self.base_url);
        let resp: TelegramResponse<BotUser> = self.http.get(&url).send().await?.json().await?;
        if resp.ok {
            resp.result
                .ok_or_else(|| anyhow::anyhow!("getMe returned ok but no result"))
        } else {
            bail!(
                "Telegram getMe failed: {}",
                resp.description.unwrap_or_default()
            )
        }
    }

    /// Long-poll for updates starting from `offset`.
    ///
    /// `timeout` is the long-poll timeout in seconds. The HTTP timeout is set
    /// to `timeout + 10` to give Telegram time to respond.
    pub async fn get_updates(&self, offset: i64, timeout: u64) -> anyhow::Result<Vec<Update>> {
        let url = format!("{}/getUpdates", self.base_url);
        let body = serde_json::json!({
            "offset": offset,
            "timeout": timeout,
            "allowed_updates": ["message", "callback_query"]
        });
        let resp: TelegramResponse<Vec<Update>> = self
            .http
            .post(&url)
            .json(&body)
            .timeout(Duration::from_secs(timeout + 10))
            .send()
            .await?
            .json()
            .await?;
        if resp.ok {
            Ok(resp.result.unwrap_or_default())
        } else {
            bail!(
                "Telegram getUpdates failed: {}",
                resp.description.unwrap_or_default()
            )
        }
    }

    /// Send a message to a chat. Handles message chunking for long content.
    ///
    /// Returns the message ID of the last sent chunk (for later editing).
    /// The inline keyboard (if any) is only attached to the last chunk.
    pub async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        reply_to: Option<String>,
        keyboard: Option<&nv_core::InlineKeyboard>,
    ) -> anyhow::Result<i64> {
        let chunks = chunk_message(text, TELEGRAM_MAX_MESSAGE_LEN);
        let last_idx = chunks.len().saturating_sub(1);
        let mut last_msg_id: i64 = 0;

        for (i, chunk) in chunks.iter().enumerate() {
            let mut body = serde_json::json!({
                "chat_id": chat_id,
                "text": chunk,
                "parse_mode": "HTML",
            });

            // reply_to only on the first chunk
            if i == 0 {
                if let Some(ref reply_id) = reply_to {
                    if let Ok(id) = reply_id.parse::<i64>() {
                        body["reply_to_message_id"] = serde_json::json!(id);
                    }
                }
            }

            // keyboard only on the last chunk
            if i == last_idx {
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
            }

            let url = format!("{}/sendMessage", self.base_url);
            let resp: TelegramResponse<serde_json::Value> =
                self.http.post(&url).json(&body).send().await?.json().await?;
            if !resp.ok {
                bail!(
                    "Telegram sendMessage failed: {}",
                    resp.description.unwrap_or_default()
                );
            }

            // Extract message_id from response
            if let Some(result) = &resp.result {
                if let Some(id) = result.get("message_id").and_then(|v| v.as_i64()) {
                    last_msg_id = id;
                }
            }
        }

        Ok(last_msg_id)
    }

    /// Edit an existing message's text.
    pub async fn edit_message(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
        keyboard: Option<&nv_core::InlineKeyboard>,
    ) -> anyhow::Result<()> {
        let url = format!("{}/editMessageText", self.base_url);
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": &text[..text.len().min(TELEGRAM_MAX_MESSAGE_LEN)],
            "parse_mode": "HTML",
        });

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

        let resp: TelegramResponse<serde_json::Value> =
            self.http.post(&url).json(&body).send().await?.json().await?;
        if !resp.ok {
            // "message is not modified" is not a real error
            let desc = resp.description.unwrap_or_default();
            if !desc.contains("message is not modified") {
                bail!("Telegram editMessageText failed: {}", desc);
            }
        }

        Ok(())
    }

    /// Send a "thinking" indicator, returns the message ID for later editing.
    pub async fn send_thinking(&self, chat_id: i64) -> anyhow::Result<i64> {
        self.send_message(chat_id, "...", None, None).await
    }

    /// Acknowledge a callback query (dismisses the loading spinner on the
    /// inline button).
    pub async fn answer_callback_query(
        &self,
        callback_query_id: &str,
        text: Option<&str>,
    ) -> anyhow::Result<()> {
        let url = format!("{}/answerCallbackQuery", self.base_url);
        let body = serde_json::json!({
            "callback_query_id": callback_query_id,
            "text": text,
        });
        let resp: TelegramResponse<bool> =
            self.http.post(&url).json(&body).send().await?.json().await?;
        if !resp.ok {
            bail!(
                "Telegram answerCallbackQuery failed: {}",
                resp.description.unwrap_or_default()
            );
        }
        Ok(())
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
        let chunks = chunk_message(text, 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn chunk_long_message_splits_at_paragraph() {
        let para1 = "A".repeat(50);
        let para2 = "B".repeat(50);
        let text = format!("{para1}\n\n{para2}");
        let chunks = chunk_message(&text, 60);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], para1);
        assert_eq!(chunks[1], para2);
    }

    #[test]
    fn chunk_long_message_splits_at_line() {
        let line1 = "A".repeat(50);
        let line2 = "B".repeat(50);
        let text = format!("{line1}\n{line2}");
        let chunks = chunk_message(&text, 60);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], line1);
        assert_eq!(chunks[1], line2);
    }

    #[test]
    fn chunk_long_message_hard_cut() {
        let text = "A".repeat(100);
        let chunks = chunk_message(&text, 40);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 40);
        assert_eq!(chunks[1].len(), 40);
        assert_eq!(chunks[2].len(), 20);
    }

    #[test]
    fn chunk_exact_max_len() {
        let text = "A".repeat(4096);
        let chunks = chunk_message(&text, 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 4096);
    }

    #[test]
    fn chunk_empty_message() {
        let chunks = chunk_message("", 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }
}
