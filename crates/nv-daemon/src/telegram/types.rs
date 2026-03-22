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

/// A single update from the Telegram Bot API.
#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<TgMessage>,
    pub callback_query: Option<CallbackQuery>,
}

/// A Telegram message (subset of fields NV needs).
#[derive(Debug, Deserialize)]
pub struct TgMessage {
    pub message_id: i64,
    pub from: Option<TgUser>,
    pub chat: TgChat,
    pub text: Option<String>,
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
    pub fn to_inbound_message(&self) -> Option<InboundMessage> {
        if let Some(msg) = &self.message {
            Some(InboundMessage {
                id: msg.message_id.to_string(),
                channel: "telegram".to_string(),
                sender: msg
                    .from
                    .as_ref()
                    .map(|u| u.username.clone().unwrap_or_else(|| u.first_name.clone()))
                    .unwrap_or_default(),
                content: msg.text.clone().unwrap_or_default(),
                timestamp: DateTime::from_timestamp(msg.date, 0).unwrap_or_else(Utc::now),
                thread_id: None,
                metadata: serde_json::json!({
                    "message_id": msg.message_id,
                    "chat_id": msg.chat.id,
                }),
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
                date: 1700000000,
            }),
            callback_query: None,
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
                    date: 1700000000,
                }),
                data: Some("approve:action-1".to_string()),
            }),
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
        let updates = vec![
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
                date: 1700000000,
            }),
            callback_query: None,
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
                date: 1700000000,
            }),
            callback_query: None,
        };
        let msg = update.to_inbound_message().unwrap();
        assert_eq!(msg.sender, "");
    }
}
