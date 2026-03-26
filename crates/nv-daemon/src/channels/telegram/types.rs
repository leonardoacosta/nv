use chrono::{DateTime, Utc};
use nv_core::InboundMessage;
use serde::Deserialize;

/// Wrapper for all Telegram Bot API responses.
#[derive(Debug, Deserialize)]
pub struct TelegramResponse<T> {
    pub ok: bool,
    pub result: Option<T>,
    pub description: Option<String>,
}

/// An inline query from a user typing `@bot_username <query>` in any chat.
///
/// Received when `"inline_query"` is included in `allowed_updates`.
/// Bot API 9.3+.
#[derive(Debug, Deserialize)]
pub struct InlineQuery {
    /// Unique identifier for this query.
    pub id: String,
    /// Sender of the inline query.
    pub from: TgUser,
    /// Text of the query (empty string if no query provided).
    pub query: String,
    /// Offset of the results to be returned (pagination token).
    #[allow(dead_code)]
    pub offset: String,
}

/// A single update from the Telegram Bot API.
#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<TgMessage>,
    pub callback_query: Option<CallbackQuery>,
    /// Inline query from a user typing `@bot_username <query>`.
    pub inline_query: Option<InlineQuery>,
}

/// A Telegram voice message.
#[derive(Debug, Deserialize)]
pub struct Voice {
    pub file_id: String,
    #[allow(dead_code)]
    pub file_unique_id: String,
    pub duration: i64,
    pub mime_type: Option<String>,
    #[allow(dead_code)]
    pub file_size: Option<i64>,
}

/// A single photo resolution from a Telegram photo message.
///
/// Telegram sends photos as an array of `PhotoSize` objects sorted
/// smallest-first by resolution. The last element is the largest.
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // API schema fields — deserialized but not all read in code
pub struct PhotoSize {
    pub file_id: String,
    pub file_unique_id: String,
    pub width: i64,
    pub height: i64,
    pub file_size: Option<i64>,
}

/// A Telegram audio file (MP3/WAV sent via the audio player, not voice notes).
///
/// Distinct from `Voice` which covers OGG voice notes. `Audio` covers any
/// audio file explicitly sent as an audio attachment.
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // API schema fields — deserialized but not all read in code
pub struct Audio {
    pub file_id: String,
    pub file_unique_id: String,
    pub duration: i64,
    pub performer: Option<String>,
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
}

/// Telegram getFile response.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TgFile {
    #[allow(dead_code)]
    pub file_id: String,
    pub file_path: Option<String>,
}

/// A Telegram message (subset of fields NV needs).
#[derive(Debug, Deserialize)]
pub struct TgMessage {
    pub message_id: i64,
    pub from: Option<TgUser>,
    pub chat: TgChat,
    pub text: Option<String>,
    pub voice: Option<Voice>,
    /// Photo message — array of `PhotoSize` sorted smallest-first. Last element
    /// is the largest resolution.
    pub photo: Option<Vec<PhotoSize>>,
    /// Audio file message (MP3/WAV). Distinct from voice notes.
    pub audio: Option<Audio>,
    /// Caption attached to a photo or audio message.
    pub caption: Option<String>,
    pub date: i64,
}

/// A Telegram user.
#[derive(Debug, Deserialize)]
pub struct TgUser {
    #[allow(dead_code)]
    pub id: i64,
    pub first_name: String,
    pub username: Option<String>,
}

/// A Telegram chat (only the id).
#[derive(Debug, Deserialize)]
pub struct TgChat {
    pub id: i64,
}

/// A callback query from an inline keyboard button press.
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: TgUser,
    pub message: Option<TgMessage>,
    pub data: Option<String>,
}

/// Bot info returned by getMe.
#[derive(Debug, Deserialize)]
pub struct BotUser {
    #[allow(dead_code)]
    pub id: i64,
    pub first_name: String,
    pub username: Option<String>,
}

impl Update {
    /// Convert a Telegram Update to the unified InboundMessage format.
    ///
    /// Returns `None` if the update contains neither a message nor a callback query.
    ///
    /// Metadata fields by message type:
    /// - Voice: `"voice": true`, `"file_id"`, `"duration_secs"`, `"mime_type"`
    /// - Photo: `"photo": true`, `"file_id"` (largest), `"caption"` (if present)
    /// - Audio: `"audio": true`, `"file_id"`, `"duration_secs"`, `"mime_type"`, `"title"`
    pub fn to_inbound_message(&self) -> Option<InboundMessage> {
        if let Some(msg) = &self.message {
            // Build metadata and content based on message type
            let (metadata, content) = if let Some(voice) = &msg.voice {
                // Voice note
                let mut meta = serde_json::json!({
                    "message_id": msg.message_id,
                    "chat_id": msg.chat.id,
                    "voice": true,
                    "file_id": voice.file_id,
                    "duration_secs": voice.duration,
                    "mime_type": voice.mime_type.as_deref().unwrap_or("audio/ogg"),
                });
                // Include file_size when present — used by the large-file gate (>20 MB reject)
                if let Some(size) = voice.file_size {
                    meta["file_size"] = serde_json::Value::Number(size.into());
                }
                (meta, msg.text.clone().unwrap_or_default())
            } else if let Some(photos) = &msg.photo {
                // Photo message — use the last (largest) PhotoSize
                let largest = photos.last();
                let file_id = largest.map(|p| p.file_id.as_str()).unwrap_or("");
                let caption = msg.caption.as_deref();
                let content = caption.unwrap_or("User sent a photo.").to_string();
                let mut meta = serde_json::json!({
                    "message_id": msg.message_id,
                    "chat_id": msg.chat.id,
                    "photo": true,
                    "file_id": file_id,
                });
                if let Some(cap) = caption {
                    meta["caption"] = serde_json::Value::String(cap.to_string());
                }
                (meta, content)
            } else if let Some(audio) = &msg.audio {
                // Audio file
                let caption = msg.caption.as_deref();
                let content = caption.unwrap_or("User sent an audio file.").to_string();
                let mut meta = serde_json::json!({
                    "message_id": msg.message_id,
                    "chat_id": msg.chat.id,
                    "audio": true,
                    "file_id": audio.file_id,
                    "duration_secs": audio.duration,
                    "mime_type": audio.mime_type.as_deref().unwrap_or("audio/mpeg"),
                });
                if let Some(title) = &audio.title {
                    meta["title"] = serde_json::Value::String(title.clone());
                }
                if let Some(cap) = caption {
                    meta["caption"] = serde_json::Value::String(cap.to_string());
                }
                (meta, content)
            } else {
                // Plain text message
                let meta = serde_json::json!({
                    "message_id": msg.message_id,
                    "chat_id": msg.chat.id,
                });
                (meta, msg.text.clone().unwrap_or_default())
            };

            Some(InboundMessage {
                id: msg.message_id.to_string(),
                channel: "telegram".to_string(),
                sender: msg
                    .from
                    .as_ref()
                    .map(|u| u.username.clone().unwrap_or_else(|| u.first_name.clone()))
                    .unwrap_or_default(),
                content,
                timestamp: DateTime::from_timestamp(msg.date, 0).unwrap_or_else(Utc::now),
                thread_id: None,
                metadata,
            })
        } else {
            self.callback_query.as_ref().map(|cb| InboundMessage {
                id: cb.id.clone(),
                channel: "telegram".to_string(),
                sender: cb
                    .from
                    .username
                    .clone()
                    .unwrap_or_else(|| cb.from.first_name.clone()),
                content: format!("[callback] {}", cb.data.as_deref().unwrap_or("")),
                timestamp: Utc::now(),
                thread_id: cb.message.as_ref().map(|m| m.message_id.to_string()),
                metadata: serde_json::json!({
                    "callback_query_id": cb.id,
                    "callback_data": cb.data,
                    "original_message_id": cb.message.as_ref().map(|m| m.message_id),
                }),
            })
        }
    }

    /// Extract the chat_id from a message or callback query.
    pub fn chat_id(&self) -> Option<i64> {
        self.message
            .as_ref()
            .map(|m| m.chat.id)
            .or_else(|| {
                self.callback_query
                    .as_ref()
                    .and_then(|cb| cb.message.as_ref())
                    .map(|m| m.chat.id)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_message_update(chat_id: i64) -> Update {
        Update {
            update_id: 100,
            message: Some(TgMessage {
                message_id: 42,
                from: Some(TgUser {
                    id: 1,
                    first_name: "Leo".to_string(),
                    username: Some("leonyaptor".to_string()),
                }),
                chat: TgChat { id: chat_id },
                text: Some("hello world".to_string()),
                voice: None,
                photo: None,
                audio: None,
                caption: None,
                date: 1700000000,
            }),
            callback_query: None,
            inline_query: None,
        }
    }

    fn make_callback_update(chat_id: i64) -> Update {
        Update {
            update_id: 101,
            message: None,
            callback_query: Some(CallbackQuery {
                id: "cb-123".to_string(),
                from: TgUser {
                    id: 1,
                    first_name: "Leo".to_string(),
                    username: Some("leonyaptor".to_string()),
                },
                message: Some(TgMessage {
                    message_id: 42,
                    from: None,
                    chat: TgChat { id: chat_id },
                    text: Some("Original message".to_string()),
                    voice: None,
                    photo: None,
                    audio: None,
                    caption: None,
                    date: 1700000000,
                }),
                data: Some("approve:action-1".to_string()),
            }),
            inline_query: None,
        }
    }

    #[test]
    fn message_update_to_inbound_message() {
        let update = make_message_update(123);
        let msg = update.to_inbound_message().unwrap();

        assert_eq!(msg.id, "42");
        assert_eq!(msg.channel, "telegram");
        assert_eq!(msg.sender, "leonyaptor");
        assert_eq!(msg.content, "hello world");
        assert!(msg.thread_id.is_none());
        assert_eq!(msg.metadata["message_id"], 42);
        assert_eq!(msg.metadata["chat_id"], 123);
    }

    #[test]
    fn callback_update_to_inbound_message() {
        let update = make_callback_update(123);
        let msg = update.to_inbound_message().unwrap();

        assert_eq!(msg.id, "cb-123");
        assert_eq!(msg.channel, "telegram");
        assert_eq!(msg.sender, "leonyaptor");
        assert_eq!(msg.content, "[callback] approve:action-1");
        assert_eq!(msg.thread_id.as_deref(), Some("42"));
        assert_eq!(msg.metadata["callback_query_id"], "cb-123");
        assert_eq!(msg.metadata["callback_data"], "approve:action-1");
        assert_eq!(msg.metadata["original_message_id"], 42);
    }

    #[test]
    fn empty_update_returns_none() {
        let update = Update {
            update_id: 102,
            message: None,
            callback_query: None,
            inline_query: None,
        };
        assert!(update.to_inbound_message().is_none());
    }

    #[test]
    fn chat_id_from_message() {
        let update = make_message_update(999);
        assert_eq!(update.chat_id(), Some(999));
    }

    #[test]
    fn chat_id_from_callback() {
        let update = make_callback_update(888);
        assert_eq!(update.chat_id(), Some(888));
    }

    #[test]
    fn chat_id_filtering() {
        let authorized_chat_id = 123_i64;
        let updates = [
            make_message_update(123),  // authorized
            make_message_update(999),  // unauthorized
            make_callback_update(123), // authorized
            make_callback_update(456), // unauthorized
        ];

        let authorized: Vec<InboundMessage> = updates
            .iter()
            .filter(|u| u.chat_id() == Some(authorized_chat_id))
            .filter_map(|u| u.to_inbound_message())
            .collect();

        assert_eq!(authorized.len(), 2);
        assert_eq!(authorized[0].content, "hello world");
        assert_eq!(authorized[1].content, "[callback] approve:action-1");
    }

    #[test]
    fn message_without_username_uses_first_name() {
        let update = Update {
            update_id: 103,
            message: Some(TgMessage {
                message_id: 50,
                from: Some(TgUser {
                    id: 1,
                    first_name: "Leo".to_string(),
                    username: None,
                }),
                chat: TgChat { id: 123 },
                text: Some("hi".to_string()),
                voice: None,
                photo: None,
                audio: None,
                caption: None,
                date: 1700000000,
            }),
            callback_query: None,
            inline_query: None,
        };
        let msg = update.to_inbound_message().unwrap();
        assert_eq!(msg.sender, "Leo");
    }

    #[test]
    fn message_without_from_uses_empty_sender() {
        let update = Update {
            update_id: 104,
            message: Some(TgMessage {
                message_id: 51,
                from: None,
                chat: TgChat { id: 123 },
                text: Some("anonymous".to_string()),
                voice: None,
                photo: None,
                audio: None,
                caption: None,
                date: 1700000000,
            }),
            callback_query: None,
            inline_query: None,
        };
        let msg = update.to_inbound_message().unwrap();
        assert_eq!(msg.sender, "");
    }

    #[test]
    fn voice_message_includes_metadata() {
        let update = Update {
            update_id: 105,
            message: Some(TgMessage {
                message_id: 60,
                from: Some(TgUser {
                    id: 1,
                    first_name: "Leo".to_string(),
                    username: Some("leonyaptor".to_string()),
                }),
                chat: TgChat { id: 123 },
                text: None,
                voice: Some(Voice {
                    file_id: "voice-file-abc".to_string(),
                    file_unique_id: "unique-abc".to_string(),
                    duration: 5,
                    mime_type: Some("audio/ogg".to_string()),
                    file_size: Some(12345),
                }),
                photo: None,
                audio: None,
                caption: None,
                date: 1700000000,
            }),
            callback_query: None,
            inline_query: None,
        };
        let msg = update.to_inbound_message().unwrap();

        assert_eq!(msg.metadata["voice"], true);
        assert_eq!(msg.metadata["file_id"], "voice-file-abc");
        assert_eq!(msg.metadata["duration_secs"], 5);
        assert_eq!(msg.metadata["mime_type"], "audio/ogg");
        // Content should be empty (voice messages have no text)
        assert_eq!(msg.content, "");
    }

    #[test]
    fn non_voice_message_has_no_voice_metadata() {
        let update = make_message_update(123);
        let msg = update.to_inbound_message().unwrap();

        assert!(msg.metadata.get("voice").is_none());
        assert_eq!(msg.content, "hello world");
    }

    #[test]
    fn photo_message_includes_metadata() {
        let update = Update {
            update_id: 106,
            message: Some(TgMessage {
                message_id: 70,
                from: Some(TgUser {
                    id: 1,
                    first_name: "Leo".to_string(),
                    username: Some("leonyaptor".to_string()),
                }),
                chat: TgChat { id: 123 },
                text: None,
                voice: None,
                photo: Some(vec![
                    PhotoSize {
                        file_id: "small-photo".to_string(),
                        file_unique_id: "small-unique".to_string(),
                        width: 320,
                        height: 240,
                        file_size: Some(12000),
                    },
                    PhotoSize {
                        file_id: "large-photo".to_string(),
                        file_unique_id: "large-unique".to_string(),
                        width: 1280,
                        height: 960,
                        file_size: Some(320000),
                    },
                ]),
                audio: None,
                caption: Some("Check this out!".to_string()),
                date: 1700000000,
            }),
            callback_query: None,
            inline_query: None,
        };
        let msg = update.to_inbound_message().unwrap();

        assert_eq!(msg.metadata["photo"], true);
        // Uses the last (largest) PhotoSize
        assert_eq!(msg.metadata["file_id"], "large-photo");
        assert_eq!(msg.metadata["caption"], "Check this out!");
        // Content is the caption
        assert_eq!(msg.content, "Check this out!");
    }

    #[test]
    fn photo_message_without_caption_uses_default_content() {
        let update = Update {
            update_id: 107,
            message: Some(TgMessage {
                message_id: 71,
                from: Some(TgUser {
                    id: 1,
                    first_name: "Leo".to_string(),
                    username: Some("leonyaptor".to_string()),
                }),
                chat: TgChat { id: 123 },
                text: None,
                voice: None,
                photo: Some(vec![PhotoSize {
                    file_id: "photo-no-cap".to_string(),
                    file_unique_id: "photo-unique".to_string(),
                    width: 800,
                    height: 600,
                    file_size: Some(50000),
                }]),
                audio: None,
                caption: None,
                date: 1700000000,
            }),
            callback_query: None,
            inline_query: None,
        };
        let msg = update.to_inbound_message().unwrap();

        assert_eq!(msg.metadata["photo"], true);
        assert_eq!(msg.content, "User sent a photo.");
        assert!(msg.metadata.get("caption").is_none());
    }

    #[test]
    fn audio_message_includes_metadata() {
        let update = Update {
            update_id: 108,
            message: Some(TgMessage {
                message_id: 80,
                from: Some(TgUser {
                    id: 1,
                    first_name: "Leo".to_string(),
                    username: Some("leonyaptor".to_string()),
                }),
                chat: TgChat { id: 123 },
                text: None,
                voice: None,
                photo: None,
                audio: Some(Audio {
                    file_id: "audio-file-xyz".to_string(),
                    file_unique_id: "audio-unique".to_string(),
                    duration: 180,
                    performer: Some("Artist".to_string()),
                    title: Some("Song Title".to_string()),
                    mime_type: Some("audio/mpeg".to_string()),
                    file_size: Some(2048000),
                }),
                caption: None,
                date: 1700000000,
            }),
            callback_query: None,
            inline_query: None,
        };
        let msg = update.to_inbound_message().unwrap();

        assert_eq!(msg.metadata["audio"], true);
        assert_eq!(msg.metadata["file_id"], "audio-file-xyz");
        assert_eq!(msg.metadata["duration_secs"], 180);
        assert_eq!(msg.metadata["mime_type"], "audio/mpeg");
        assert_eq!(msg.metadata["title"], "Song Title");
        assert_eq!(msg.content, "User sent an audio file.");
    }

    #[test]
    fn audio_message_with_caption() {
        let update = Update {
            update_id: 109,
            message: Some(TgMessage {
                message_id: 81,
                from: Some(TgUser {
                    id: 1,
                    first_name: "Leo".to_string(),
                    username: Some("leonyaptor".to_string()),
                }),
                chat: TgChat { id: 123 },
                text: None,
                voice: None,
                photo: None,
                audio: Some(Audio {
                    file_id: "audio-with-cap".to_string(),
                    file_unique_id: "awc-unique".to_string(),
                    duration: 60,
                    performer: None,
                    title: None,
                    mime_type: Some("audio/mpeg".to_string()),
                    file_size: Some(512000),
                }),
                caption: Some("Please transcribe this".to_string()),
                date: 1700000000,
            }),
            callback_query: None,
            inline_query: None,
        };
        let msg = update.to_inbound_message().unwrap();

        assert_eq!(msg.metadata["audio"], true);
        assert_eq!(msg.metadata["caption"], "Please transcribe this");
        // Caption used as content
        assert_eq!(msg.content, "Please transcribe this");
    }
}
