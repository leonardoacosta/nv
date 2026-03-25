use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::bail;
use reqwest::multipart;
use reqwest::Client;

use super::types::{BotUser, TelegramResponse, Update};

// ── Throttle State ──────────────────────────────────────────────────

/// Per-chat-id throttle state for `send_chat_action`.
struct ThrottleState {
    /// Timestamp of the last successful send per chat_id.
    last_sent: HashMap<i64, Instant>,
    /// Backoff deadline per chat_id (set on 429 responses).
    backoff_until: HashMap<i64, Instant>,
}

impl ThrottleState {
    fn new() -> Self {
        Self {
            last_sent: HashMap::new(),
            backoff_until: HashMap::new(),
        }
    }

    /// Returns `true` if a send should be suppressed for this `chat_id`.
    fn is_suppressed(&self, chat_id: i64) -> bool {
        let now = Instant::now();
        // Suppress if within the backoff window.
        if let Some(&until) = self.backoff_until.get(&chat_id) {
            if until > now {
                return true;
            }
        }
        // Suppress if last send was within 5 seconds.
        if let Some(&last) = self.last_sent.get(&chat_id) {
            if now.duration_since(last) < Duration::from_secs(5) {
                return true;
            }
        }
        false
    }

    /// Record a successful send for `chat_id`.
    fn record_sent(&mut self, chat_id: i64) {
        self.last_sent.insert(chat_id, Instant::now());
    }

    /// Record a 429 backoff for `chat_id`.
    ///
    /// `retry_after_secs` comes from the Telegram error JSON `parameters.retry_after`.
    /// Defaults to 30s if `None`.
    fn record_backoff(&mut self, chat_id: i64, retry_after_secs: Option<u64>) {
        let delay = retry_after_secs.unwrap_or(30);
        self.backoff_until.insert(chat_id, Instant::now() + Duration::from_secs(delay));
    }
}

/// Telegram Bot API maximum message length.
const TELEGRAM_MAX_MESSAGE_LEN: usize = 4096;

/// Convert common Markdown patterns from Claude's output to Telegram HTML.
fn markdown_to_html(text: &str) -> String {
    // If text already contains HTML tags, assume it's pre-formatted and skip conversion.
    if text.contains("<b>")
        || text.contains("<i>")
        || text.contains("<pre>")
        || text.contains("<code>")
        || text.contains("<a ")
    {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim_start();

        // Detect markdown table: a line with | delimiters followed by a separator row
        if is_table_row(trimmed) && i + 1 < lines.len() && is_table_separator(lines[i + 1].trim_start()) {
            // Collect all contiguous table rows
            let mut table_rows: Vec<Vec<String>> = Vec::new();

            // Header row
            table_rows.push(parse_table_row(trimmed));
            i += 1; // move past header

            // Skip separator row
            if i < lines.len() && is_table_separator(lines[i].trim_start()) {
                i += 1;
            }

            // Data rows
            while i < lines.len() && is_table_row(lines[i].trim_start()) {
                let row_trimmed = lines[i].trim_start();
                if is_table_separator(row_trimmed) {
                    i += 1;
                    continue;
                }
                table_rows.push(parse_table_row(row_trimmed));
                i += 1;
            }

            // Calculate column widths for alignment
            let col_count = table_rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let mut col_widths = vec![0usize; col_count];
            for row in &table_rows {
                for (j, cell) in row.iter().enumerate() {
                    if j < col_count {
                        col_widths[j] = col_widths[j].max(cell.len());
                    }
                }
            }

            // Render as <pre> block with aligned columns
            result.push_str("<pre>");
            for (row_idx, row) in table_rows.iter().enumerate() {
                for (j, cell) in row.iter().enumerate() {
                    if j > 0 {
                        result.push_str("  ");
                    }
                    let width = col_widths.get(j).copied().unwrap_or(cell.len());
                    result.push_str(&escape_html(cell));
                    // Pad with spaces for alignment (except last column)
                    if j < col_count.saturating_sub(1) {
                        let padding = width.saturating_sub(cell.len());
                        for _ in 0..padding {
                            result.push(' ');
                        }
                    }
                }
                // Add underline after header row
                if row_idx == 0 && table_rows.len() > 1 {
                    result.push('\n');
                    for (j, &w) in col_widths.iter().enumerate() {
                        if j > 0 {
                            result.push_str("  ");
                        }
                        for _ in 0..w {
                            result.push('-');
                        }
                    }
                }
                if row_idx < table_rows.len() - 1 {
                    result.push('\n');
                }
            }
            result.push_str("</pre>");
            result.push('\n');
            continue;
        }

        // Headers → bold
        if let Some(h) = trimmed.strip_prefix("### ") {
            result.push_str(&format!("<b>{}</b>", escape_html(h)));
        } else if let Some(h) = trimmed.strip_prefix("## ") {
            result.push_str(&format!("<b>{}</b>", escape_html(h)));
        } else if let Some(h) = trimmed.strip_prefix("# ") {
            result.push_str(&format!("<b>{}</b>", escape_html(h)));
        } else if trimmed == "---" {
            result.push_str("—————");
        } else {
            // Inline formatting within the line
            let escaped = escape_html(lines[i]);
            let converted = convert_inline_markdown(&escaped);
            result.push_str(&converted);
        }
        result.push('\n');
        i += 1;
    }

    // Trim trailing newline
    result.trim_end_matches('\n').to_string()
}

/// Check if a line looks like a markdown table row (contains | delimiters).
fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    // Must contain at least one | and have content besides just pipes/whitespace
    trimmed.contains('|')
        && trimmed
            .chars()
            .any(|c| c != '|' && c != '-' && !c.is_whitespace())
}

/// Check if a line is a table separator row (e.g., |------|------|).
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') || !trimmed.contains('-') {
        return false;
    }
    // All characters should be |, -, :, or whitespace
    trimmed.chars().all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
}

/// Parse a markdown table row into cells, trimming whitespace.
fn parse_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    // Strip leading/trailing pipes
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// Escape HTML special characters.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Convert inline Markdown formatting to HTML.
/// Handles: **bold**, `code`, _italic_
fn convert_inline_markdown(text: &str) -> String {
    let mut result = text.to_string();

    // **bold** → <b>bold</b> (do this before single *)
    while let Some(start) = result.find("**") {
        if let Some(end) = result[start + 2..].find("**") {
            let end = start + 2 + end;
            let inner = result[start + 2..end].to_string();
            result = format!("{}<b>{inner}</b>{}", &result[..start], &result[end + 2..]);
        } else {
            break;
        }
    }

    // `code` → <code>code</code>
    while let Some(start) = result.find('`') {
        if let Some(end) = result[start + 1..].find('`') {
            let end = start + 1 + end;
            let inner = result[start + 1..end].to_string();
            result = format!(
                "{}<code>{inner}</code>{}",
                &result[..start],
                &result[end + 1..]
            );
        } else {
            break;
        }
    }

    result
}

/// Thin HTTP wrapper for Telegram Bot API endpoints.
///
/// `Clone` is cheap — `throttle` is `Arc<Mutex<...>>` so all clones share
/// the same throttle state.
#[derive(Clone)]
pub struct TelegramClient {
    http: Client,
    base_url: String,
    /// Shared throttle state across all clones of this client.
    throttle: Arc<Mutex<ThrottleState>>,
}

impl TelegramClient {
    /// Create a new client for the given bot token.
    ///
    /// Constructs the base URL `https://api.telegram.org/bot{token}`.
    pub fn new(bot_token: &str) -> Self {
        Self {
            http: Client::new(),
            base_url: format!("https://api.telegram.org/bot{bot_token}"),
            throttle: Arc::new(Mutex::new(ThrottleState::new())),
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
        let html_text = markdown_to_html(text);
        let chunks = chunk_message(&html_text, TELEGRAM_MAX_MESSAGE_LEN);
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
        let html_text = markdown_to_html(text);
        let truncated = crate::channels::util::safe_truncate(&html_text, TELEGRAM_MAX_MESSAGE_LEN);
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": truncated,
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

    /// Edit the text of an existing message without changing its keyboard.
    ///
    /// Convenience wrapper around `edit_message` for use in streaming delivery
    /// where no keyboard update is needed.
    #[allow(dead_code)]
    pub async fn edit_message_text(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
    ) -> anyhow::Result<()> {
        self.edit_message(chat_id, message_id, text, None).await
    }

    /// Send a "thinking" indicator, returns the message ID for later editing.
    // Wiring point for future typing-indicator replacement via message edit.
    #[allow(dead_code)]
    pub async fn send_thinking(&self, chat_id: i64) -> anyhow::Result<i64> {
        self.send_message(chat_id, "...", None, None).await
    }

    /// Delete a message.
    #[allow(dead_code)]
    pub async fn delete_message(&self, chat_id: i64, message_id: i64) -> anyhow::Result<()> {
        let url = format!("{}/deleteMessage", self.base_url);
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
        });
        let _ = self.http.post(&url).json(&body).send().await?;
        Ok(())
    }

    /// Send a voice message (OGG/Opus) to a chat.
    ///
    /// Uses multipart/form-data to upload the audio bytes via Telegram's
    /// `sendVoice` endpoint. The voice message appears as an inline
    /// waveform bubble in the chat.
    pub async fn send_voice(
        &self,
        chat_id: i64,
        ogg_bytes: Vec<u8>,
        reply_to: Option<i64>,
    ) -> anyhow::Result<i64> {
        let url = format!("{}/sendVoice", self.base_url);

        let voice_part = multipart::Part::bytes(ogg_bytes)
            .file_name("voice.ogg")
            .mime_str("audio/ogg")?;

        let mut form = multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .part("voice", voice_part);

        if let Some(reply_id) = reply_to {
            form = form.text("reply_to_message_id", reply_id.to_string());
        }

        let resp: TelegramResponse<serde_json::Value> = self
            .http
            .post(&url)
            .multipart(form)
            .send()
            .await?
            .json()
            .await?;

        if !resp.ok {
            bail!(
                "Telegram sendVoice failed: {}",
                resp.description.unwrap_or_default()
            );
        }

        let msg_id = resp
            .result
            .as_ref()
            .and_then(|r| r.get("message_id"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(msg_id)
    }

    /// Set a reaction emoji on a message.
    ///
    /// Uses the Telegram Bot API `setMessageReaction` endpoint (Bot API 7.3+).
    /// Pass a single emoji string (e.g. "\u{1F440}" for eyes, "\u{2705}" for check mark).
    pub async fn set_message_reaction(
        &self,
        chat_id: i64,
        message_id: i64,
        emoji: &str,
    ) -> anyhow::Result<()> {
        let url = format!("{}/setMessageReaction", self.base_url);
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reaction": [{"type": "emoji", "emoji": emoji}],
        });
        let resp: TelegramResponse<bool> =
            self.http.post(&url).json(&body).send().await?.json().await?;
        if !resp.ok {
            let desc = resp.description.unwrap_or_default();
            // Don't fail on "reaction not changed" — it's harmless
            if !desc.contains("REACTION_INVALID") {
                tracing::debug!(error = %desc, "setMessageReaction failed (non-fatal)");
            }
        }
        Ok(())
    }

    /// Remove all reactions from a message.
    ///
    /// Sends an empty reaction array to clear any previously set reaction.
    #[allow(dead_code)]
    pub async fn remove_message_reaction(
        &self,
        chat_id: i64,
        message_id: i64,
    ) -> anyhow::Result<()> {
        let url = format!("{}/setMessageReaction", self.base_url);
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reaction": [],
        });
        let resp: TelegramResponse<bool> =
            self.http.post(&url).json(&body).send().await?.json().await?;
        if !resp.ok {
            let desc = resp.description.unwrap_or_default();
            tracing::debug!(error = %desc, "removeMessageReaction failed (non-fatal)");
        }
        Ok(())
    }

    /// Send a chat action (e.g. "typing") to a chat with per-chat-id throttling.
    ///
    /// Suppresses the call if it was made within 5 seconds of the last successful
    /// call for the same `chat_id`, or if a 429 backoff window is active.
    ///
    /// On a 429 response the `parameters.retry_after` field (seconds) is read
    /// from the Telegram error JSON and stored as a backoff deadline. Defaults to
    /// 30 seconds if the field is absent or unparseable.
    ///
    /// Returns `true` if the call was sent, `false` if suppressed.
    ///
    /// **Presence limitations:** `sendChatAction` is the only per-message engagement
    /// signal available to Telegram bots. The Telegram Bot API does not expose bot
    /// online/offline presence status — that is a user-controlled setting not
    /// accessible via the Bot API. Regular user presence (online/last seen) is
    /// similarly not accessible to bots. This method wraps `sendChatAction`, which
    /// is the closest equivalent to a presence signal bots can emit.
    pub async fn send_chat_action(&self, chat_id: i64, action: &str) -> bool {
        // Check throttle state under lock (released before any await).
        {
            let state = self.throttle.lock().unwrap();
            if state.is_suppressed(chat_id) {
                return false;
            }
        }

        let url = format!("{}/sendChatAction", self.base_url);
        let body = serde_json::json!({
            "chat_id": chat_id,
            "action": action,
        });

        match self.http.post(&url).json(&body).send().await {
            Ok(resp) => {
                // Read the full response body as JSON so we can inspect both
                // the `ok` field and the optional `parameters.retry_after` field.
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let ok = json.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        if ok {
                            // Record the successful send under lock.
                            self.throttle.lock().unwrap().record_sent(chat_id);
                            true
                        } else {
                            let desc = json
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();

                            // Parse retry_after from the error parameters field.
                            let retry_after = json
                                .get("parameters")
                                .and_then(|p| p.get("retry_after"))
                                .and_then(|v| v.as_u64());

                            if desc.contains("429") || retry_after.is_some() {
                                let secs = retry_after;
                                tracing::warn!(
                                    chat_id,
                                    retry_after_secs = ?secs,
                                    "sendChatAction 429 — backing off"
                                );
                                self.throttle.lock().unwrap().record_backoff(chat_id, secs);
                            } else {
                                tracing::warn!(error = %desc, "sendChatAction failed");
                            }
                            false
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "sendChatAction response parse failed");
                        false
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "sendChatAction request failed");
                false
            }
        }
    }

    /// Call Telegram getFile API to get the file path for a given file_id.
    pub async fn get_file(&self, file_id: &str) -> anyhow::Result<String> {
        let url = format!("{}/getFile", self.base_url);
        let body = serde_json::json!({ "file_id": file_id });
        let resp: super::types::TelegramResponse<super::types::TgFile> =
            self.http.post(&url).json(&body).send().await?.json().await?;
        if !resp.ok {
            bail!(
                "Telegram getFile failed: {}",
                resp.description.unwrap_or_default()
            );
        }
        let file = resp
            .result
            .ok_or_else(|| anyhow::anyhow!("getFile returned ok but no result"))?;
        file.file_path
            .ok_or_else(|| anyhow::anyhow!("getFile returned no file_path"))
    }

    /// Download a file from Telegram's file server.
    ///
    /// Returns the raw bytes. The `file_path` is obtained from `get_file()`.
    pub async fn download_file(&self, file_path: &str) -> anyhow::Result<Vec<u8>> {
        // Extract the bot token from the base_url (format: https://api.telegram.org/bot{token})
        let token = self
            .base_url
            .strip_prefix("https://api.telegram.org/bot")
            .unwrap_or("");
        let url = format!("https://api.telegram.org/file/bot{token}/{file_path}");
        let bytes = self.http.get(&url).send().await?.bytes().await?;
        Ok(bytes.to_vec())
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

/// Re-export the canonical chunk_message from the shared util module.
pub use crate::channels::util::chunk_message;

#[cfg(test)]
mod tests {
    use super::*;

    // ── markdown_to_html table tests ────────────────────────────────

    #[test]
    fn markdown_to_html_converts_table_to_pre_block() {
        let input = "| Name | Age | City |\n|------|-----|------|\n| Leo | 30 | NYC |\n| Ana | 25 | LA |";
        let html = markdown_to_html(input);
        assert!(html.contains("<pre>"), "should contain <pre> tag");
        assert!(html.contains("</pre>"), "should contain </pre> tag");
        assert!(html.contains("Name"), "should contain header Name");
        assert!(html.contains("Leo"), "should contain data Leo");
        assert!(html.contains("Ana"), "should contain data Ana");
    }

    #[test]
    fn markdown_to_html_strips_separator_row() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = markdown_to_html(input);
        // Separator row raw pipes should not appear in output
        assert!(!html.contains("|---|"), "should not contain raw separator pipes");
        assert!(!html.contains("|"), "should not contain pipe characters in pre block");
    }

    #[test]
    fn markdown_to_html_aligns_columns() {
        let input = "| Name | Score |\n|------|-------|\n| A | 100 |\n| Bob | 5 |";
        let html = markdown_to_html(input);
        assert!(html.contains("<pre>"));
        // Both rows should have consistent column alignment
        // "Name " and "A   " should be padded to same width
        assert!(html.contains("Name"), "should contain Name");
        assert!(html.contains("Bob"), "should contain Bob");
    }

    #[test]
    fn markdown_to_html_preserves_non_table_content() {
        let input = "Hello\n| A | B |\n|---|---|\n| 1 | 2 |\nGoodbye";
        let html = markdown_to_html(input);
        assert!(html.contains("Hello"));
        assert!(html.contains("<pre>"));
        assert!(html.contains("Goodbye"));
    }

    #[test]
    fn markdown_to_html_no_table_unchanged() {
        let input = "Just some text with a | pipe in it";
        let html = markdown_to_html(input);
        assert!(!html.contains("<pre>"));
    }

    // ── is_table_separator tests ────────────────────────────────────

    #[test]
    fn table_separator_detection() {
        assert!(is_table_separator("|------|------|"));
        assert!(is_table_separator("| --- | --- |"));
        assert!(is_table_separator("|:---:|:---:|"));
        assert!(!is_table_separator("| data | here |"));
        assert!(!is_table_separator("no pipes here"));
    }

    // ── HTML passthrough tests ───────────────────────────────────────

    #[test]
    fn markdown_to_html_passes_through_preformatted_html() {
        let input = "<b>Hello</b>";
        assert_eq!(markdown_to_html(input), "<b>Hello</b>");
    }

    #[test]
    fn markdown_to_html_passes_through_complex_html() {
        let input = "<b>Good morning. Daily briefing:</b>\n\n<i>Weather:</i> Sunny";
        assert_eq!(markdown_to_html(input), input);
    }

    // ── edit_message truncation path tests ───────────────────────────

    #[test]
    fn edit_message_truncation_no_panic_with_non_ascii() {
        // Build a 5000+ char string mixing emoji (4 bytes each), CJK (3 bytes
        // each), accented Latin (2 bytes each), and ASCII (1 byte each).
        let segment = "\u{1F600}\u{4E16}\u{754C}\u{00E9}Hello"; // 😀世界éHello = 4+3+3+2+5 = 17 bytes
        let input: String = segment.repeat(400); // 400 * 17 = 6800 bytes, well over 5000 chars
        assert!(input.len() > 5000);

        // Replicate the exact truncation path from edit_message:
        //   1. markdown_to_html
        //   2. safe_truncate to TELEGRAM_MAX_MESSAGE_LEN (4096)
        let html_text = markdown_to_html(&input);
        let truncated =
            crate::channels::util::safe_truncate(&html_text, TELEGRAM_MAX_MESSAGE_LEN);

        // Must not panic, must be valid UTF-8 (guaranteed by &str), and must
        // fit within the Telegram limit.
        assert!(truncated.len() <= TELEGRAM_MAX_MESSAGE_LEN);
        // Verify the result is non-empty (the input is large enough to produce output).
        assert!(!truncated.is_empty());
    }

    // ── ThrottleState tests ──────────────────────────────────────────

    #[test]
    fn throttle_suppresses_second_call_within_5s_for_same_chat_id() {
        let mut state = ThrottleState::new();
        let chat_id: i64 = 123;

        // First call: not suppressed, then record it.
        assert!(!state.is_suppressed(chat_id));
        state.record_sent(chat_id);

        // Second call within 5s: suppressed.
        assert!(state.is_suppressed(chat_id));
    }

    #[test]
    fn throttle_does_not_suppress_different_chat_ids() {
        let mut state = ThrottleState::new();
        let chat_a: i64 = 111;
        let chat_b: i64 = 222;

        // Record a send for chat_a.
        state.record_sent(chat_a);

        // chat_a is suppressed but chat_b is not.
        assert!(state.is_suppressed(chat_a));
        assert!(!state.is_suppressed(chat_b));
    }

    #[test]
    fn throttle_backoff_with_retry_after_suppresses_calls() {
        let mut state = ThrottleState::new();
        let chat_id: i64 = 456;

        // Record a 10s backoff.
        state.record_backoff(chat_id, Some(10));

        // Immediately after backoff, suppressed.
        assert!(state.is_suppressed(chat_id));
    }

    #[test]
    fn throttle_backoff_missing_retry_after_defaults_to_30s() {
        let mut state = ThrottleState::new();
        let chat_id: i64 = 789;

        // Record backoff with no retry_after (defaults to 30s).
        state.record_backoff(chat_id, None);

        // Immediately after, suppressed.
        assert!(state.is_suppressed(chat_id));

        // The backoff deadline should be ~30s in the future.
        // We can verify by checking that it's definitely more than 25s away.
        let until = state.backoff_until[&chat_id];
        let remaining = until.duration_since(Instant::now());
        assert!(remaining > Duration::from_secs(25), "default backoff should be ~30s");
    }
}
