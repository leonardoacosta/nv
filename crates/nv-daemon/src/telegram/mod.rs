pub mod client;
pub mod types;

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
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
///
/// The `voice_enabled` flag is toggled by the `/voice` command, which is
/// intercepted here before reaching the agent loop.
pub async fn run_poll_loop(channel: TelegramChannel, voice_enabled: Arc<AtomicBool>) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(60);

    loop {
        match channel.poll_messages().await {
            Ok(messages) => {
                backoff = Duration::from_secs(1); // Reset on success
                for msg in messages {
                    // Intercept /voice command — toggle voice and respond directly
                    if msg.content.trim() == "/voice" {
                        let was_enabled = voice_enabled.fetch_xor(true, Ordering::Relaxed);
                        let now_enabled = !was_enabled;
                        let status = if now_enabled { "enabled" } else { "disabled" };
                        let reply = format!("Voice replies {status}.");
                        tracing::info!(voice_enabled = now_enabled, "voice toggle");
                        if let Err(e) = channel
                            .client
                            .send_message(channel.chat_id, &reply, Some(msg.id.clone()), None)
                            .await
                        {
                            tracing::error!(error = %e, "failed to send /voice response");
                        }
                        continue;
                    }

                    // Handle voice messages — transcribe before dispatch
                    let msg = if msg.metadata.get("voice").and_then(|v| v.as_bool()).unwrap_or(false) {
                        match transcribe_voice_message(&channel, &msg).await {
                            Ok(transcribed) => transcribed,
                            Err(e) => {
                                tracing::warn!(error = %e, "voice transcription failed");
                                // Send error reply but don't dispatch the message
                                let chat_id = msg.metadata.get("chat_id")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(channel.chat_id);
                                let _ = channel.client.send_message(
                                    chat_id,
                                    "Could not transcribe voice message. Please try again or type your message.",
                                    Some(msg.id.clone()),
                                    None,
                                ).await;
                                continue;
                            }
                        }
                    } else if msg.metadata.get("photo").and_then(|v| v.as_bool()).unwrap_or(false) {
                        // Handle photo messages — download and attach for Claude vision
                        match handle_photo_message(&channel, &msg).await {
                            Ok(enriched) => enriched,
                            Err(e) => {
                                tracing::warn!(error = %e, "photo download failed");
                                let chat_id = msg.metadata.get("chat_id")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(channel.chat_id);
                                let _ = channel.client.send_message(
                                    chat_id,
                                    "Could not download photo. Please try again.",
                                    Some(msg.id.clone()),
                                    None,
                                ).await;
                                continue;
                            }
                        }
                    } else if msg.metadata.get("audio").and_then(|v| v.as_bool()).unwrap_or(false) {
                        // Handle audio file messages — transcribe via ElevenLabs STT
                        match handle_audio_message(&channel, &msg).await {
                            Ok(transcribed) => transcribed,
                            Err(e) => {
                                tracing::warn!(error = %e, "audio transcription failed");
                                let chat_id = msg.metadata.get("chat_id")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(channel.chat_id);
                                let _ = channel.client.send_message(
                                    chat_id,
                                    "Could not transcribe audio file. Please try again.",
                                    Some(msg.id.clone()),
                                    None,
                                ).await;
                                continue;
                            }
                        }
                    } else {
                        msg
                    };

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

// ── Photo Handling ──────────────────────────────────────────────────

/// Download a photo from Telegram and save to a temp file.
///
/// Sets `"image_path"` in the returned message metadata. The caller is
/// responsible for cleaning up the temp file after the Claude turn completes.
async fn handle_photo_message(
    channel: &TelegramChannel,
    msg: &InboundMessage,
) -> anyhow::Result<InboundMessage> {
    let file_id = msg
        .metadata
        .get("file_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("photo message missing file_id"))?;

    let chat_id = msg
        .metadata
        .get("chat_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(channel.chat_id);

    // Send typing indicator while downloading
    channel.client.send_chat_action(chat_id, "typing").await;

    // Download the photo file
    let file_path = channel.client.get_file(file_id).await?;
    let photo_bytes = channel.client.download_file(&file_path).await?;

    tracing::info!(
        file_id,
        bytes = photo_bytes.len(),
        "downloaded photo for vision"
    );

    // Save to a temp file
    let tmp_path = format!("/tmp/nv-photo-{}.jpg", uuid::Uuid::new_v4());
    std::fs::write(&tmp_path, &photo_bytes)
        .map_err(|e| anyhow::anyhow!("failed to write photo to {tmp_path}: {e}"))?;

    tracing::info!(path = %tmp_path, "photo saved to temp file");

    // Add image_path to metadata
    let mut metadata = msg.metadata.clone();
    metadata["image_path"] = serde_json::Value::String(tmp_path);

    Ok(InboundMessage {
        id: msg.id.clone(),
        channel: msg.channel.clone(),
        sender: msg.sender.clone(),
        content: msg.content.clone(),
        timestamp: msg.timestamp,
        thread_id: msg.thread_id.clone(),
        metadata,
    })
}

// ── Audio Handling ──────────────────────────────────────────────────

/// Download an audio file from Telegram and transcribe via ElevenLabs STT.
///
/// Returns a modified `InboundMessage` with the transcript as content.
/// If caption was present it is prepended to the transcript.
async fn handle_audio_message(
    channel: &TelegramChannel,
    msg: &InboundMessage,
) -> anyhow::Result<InboundMessage> {
    let file_id = msg
        .metadata
        .get("file_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("audio message missing file_id"))?;

    let mime_type = msg
        .metadata
        .get("mime_type")
        .and_then(|v| v.as_str())
        .unwrap_or("audio/mpeg");

    let chat_id = msg
        .metadata
        .get("chat_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(channel.chat_id);

    // Check for ElevenLabs API key
    let api_key = match std::env::var("ELEVENLABS_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            let _ = channel
                .client
                .send_message(
                    chat_id,
                    "Audio transcription not configured (ELEVENLABS_API_KEY missing).",
                    Some(msg.id.clone()),
                    None,
                )
                .await;
            anyhow::bail!("ELEVENLABS_API_KEY not set");
        }
    };

    // Send typing indicator while transcribing
    channel.client.send_chat_action(chat_id, "typing").await;

    // Download the audio file
    let file_path = channel.client.get_file(file_id).await?;
    let audio_bytes = channel.client.download_file(&file_path).await?;

    tracing::info!(
        file_id,
        bytes = audio_bytes.len(),
        mime_type,
        "downloaded audio file for transcription"
    );

    // Derive a sensible file name for the multipart upload
    let file_name = if mime_type.contains("mpeg") || mime_type.contains("mp3") {
        "audio.mp3"
    } else if mime_type.contains("wav") {
        "audio.wav"
    } else {
        "audio.bin"
    };

    let transcript =
        crate::speech_to_text::transcribe_audio_elevenlabs(audio_bytes, file_name, mime_type, &api_key)
            .await?;

    if transcript.is_empty() {
        let _ = channel
            .client
            .send_message(
                chat_id,
                "No speech detected in audio file.",
                Some(msg.id.clone()),
                None,
            )
            .await;
        anyhow::bail!("empty transcript from ElevenLabs STT");
    }

    // Prepend caption if present
    let caption = msg
        .metadata
        .get("caption")
        .and_then(|v| v.as_str());

    let content = if let Some(cap) = caption {
        format!("{cap}\n\n[Transcription]: {transcript}")
    } else {
        transcript
    };

    Ok(InboundMessage {
        id: msg.id.clone(),
        channel: msg.channel.clone(),
        sender: msg.sender.clone(),
        content,
        timestamp: msg.timestamp,
        thread_id: msg.thread_id.clone(),
        metadata: msg.metadata.clone(),
    })
}

// ── Voice Transcription ─────────────────────────────────────────────

/// Transcribe a voice message by downloading from Telegram and calling Deepgram.
///
/// Returns a modified `InboundMessage` with the transcribed text as content.
/// The original voice metadata is preserved.
async fn transcribe_voice_message(
    channel: &TelegramChannel,
    msg: &InboundMessage,
) -> anyhow::Result<InboundMessage> {
    let file_id = msg
        .metadata
        .get("file_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("voice message missing file_id"))?;

    let mime_type = msg
        .metadata
        .get("mime_type")
        .and_then(|v| v.as_str())
        .unwrap_or("audio/ogg");

    let chat_id = msg
        .metadata
        .get("chat_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(channel.chat_id);

    // Check for DEEPGRAM_API_KEY
    let api_key = match std::env::var("DEEPGRAM_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            let _ = channel
                .client
                .send_message(
                    chat_id,
                    "Voice transcription not configured.",
                    Some(msg.id.clone()),
                    None,
                )
                .await;
            anyhow::bail!("DEEPGRAM_API_KEY not set");
        }
    };

    // Send typing indicator while transcribing
    channel.client.send_chat_action(chat_id, "typing").await;

    // Download the voice file from Telegram
    let file_path = channel.client.get_file(file_id).await?;
    let audio_bytes = channel.client.download_file(&file_path).await?;

    tracing::info!(
        file_id,
        bytes = audio_bytes.len(),
        mime_type,
        "downloaded voice message for transcription"
    );

    // Transcribe via Deepgram
    let model = std::env::var("DEEPGRAM_MODEL").unwrap_or_else(|_| "nova-2".to_string());
    let transcript =
        crate::voice_input::transcribe_voice(&audio_bytes, mime_type, &api_key, &model).await?;

    if transcript.is_empty() {
        let _ = channel
            .client
            .send_message(
                chat_id,
                "No speech detected in voice message.",
                Some(msg.id.clone()),
                None,
            )
            .await;
        anyhow::bail!("empty transcript from Deepgram");
    }

    tracing::info!(
        transcript_len = transcript.len(),
        "voice message transcribed"
    );

    // Return a new InboundMessage with the transcribed text
    Ok(InboundMessage {
        id: msg.id.clone(),
        channel: msg.channel.clone(),
        sender: msg.sender.clone(),
        content: transcript,
        timestamp: msg.timestamp,
        thread_id: msg.thread_id.clone(),
        metadata: msg.metadata.clone(),
    })
}

// ── Inline Keyboard Builders ───────────────────────────────────────
//
// Builder methods (confirm_action, from_actions) are defined on
// InlineKeyboard in nv-core::types since InlineKeyboard is owned by
// that crate.

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use nv_core::types::{ActionStatus, ActionType, InlineKeyboard, PendingAction};
    use uuid::Uuid;

    #[test]
    fn voice_toggle_flips_atomic_bool() {
        let voice_enabled = Arc::new(AtomicBool::new(false));

        // Simulate first /voice toggle: false → true
        let was_enabled = voice_enabled.fetch_xor(true, Ordering::Relaxed);
        assert!(!was_enabled);
        assert!(voice_enabled.load(Ordering::Relaxed));

        // Simulate second /voice toggle: true → false
        let was_enabled = voice_enabled.fetch_xor(true, Ordering::Relaxed);
        assert!(was_enabled);
        assert!(!voice_enabled.load(Ordering::Relaxed));
    }

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
                telegram_message_id: None,
                telegram_chat_id: None,
            },
            PendingAction {
                id: Uuid::new_v4(),
                description: "Assign to Leo".to_string(),
                action_type: ActionType::JiraAssign,
                payload: serde_json::json!({}),
                status: ActionStatus::Pending,
                created_at: Utc::now(),
                telegram_message_id: None,
                telegram_chat_id: None,
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

    /// Integration test against real Telegram Bot API.
    ///
    /// Requires `NV_TELEGRAM_INTEGRATION_TEST=1`, `TELEGRAM_BOT_TOKEN`,
    /// and `NV_TEST_CHAT_ID` environment variables.
    ///
    /// Run with: `cargo test -p nv-daemon --features integration telegram_real_api`
    #[cfg(feature = "integration")]
    #[tokio::test]
    async fn telegram_real_api() {
        if std::env::var("NV_TELEGRAM_INTEGRATION_TEST").is_err() {
            eprintln!("Skipping: set NV_TELEGRAM_INTEGRATION_TEST=1 to run");
            return;
        }

        let token = std::env::var("TELEGRAM_BOT_TOKEN")
            .expect("TELEGRAM_BOT_TOKEN required for integration test");
        let chat_id: i64 = std::env::var("NV_TEST_CHAT_ID")
            .expect("NV_TEST_CHAT_ID required for integration test")
            .parse()
            .expect("NV_TEST_CHAT_ID must be i64");

        let client = TelegramClient::new(&token);

        // Verify bot token via getMe
        let me = client.get_me().await.expect("get_me should succeed");
        assert!(
            me.username.is_some() && !me.username.as_ref().unwrap().is_empty(),
            "Bot username should be non-empty"
        );

        // Send echo message
        let msg_id = client
            .send_message(
                chat_id,
                "Integration test: echo from nv-daemon",
                None,
                None,
            )
            .await
            .expect("send_message should succeed");

        assert!(msg_id > 0, "send_message should return a valid message ID");
    }
}
