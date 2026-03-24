use chrono::{DateTime, Utc};
use nv_core::InboundMessage;
use serde::{Deserialize, Serialize};

// ── OAuth2 Types ─────────────────────────────────────────────────

/// Response from Microsoft Identity Platform token endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    #[allow(dead_code)]
    pub token_type: String,
    /// Token lifetime in seconds (typically 3600).
    pub expires_in: u64,
}

// ── Subscription Types ───────────────────────────────────────────

/// Request body for POST /subscriptions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionRequest {
    pub change_type: String,
    pub notification_url: String,
    pub resource: String,
    pub expiration_date_time: String,
    pub client_state: Option<String>,
}

/// Response from POST /subscriptions.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionResponse {
    pub id: String,
    #[allow(dead_code)]
    pub resource: String,
    #[allow(dead_code)]
    pub change_type: String,
    pub expiration_date_time: String,
}

// ── Change Notification Types ────────────────────────────────────

/// Top-level change notification payload from MS Graph webhook.
#[derive(Debug, Clone, Deserialize)]
pub struct ChangeNotificationCollection {
    pub value: Vec<ChangeNotification>,
}

/// Individual change notification.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeNotification {
    #[allow(dead_code)]
    pub subscription_id: String,
    #[allow(dead_code)]
    pub change_type: String,
    pub resource: String,
    #[allow(dead_code)]
    pub client_state: Option<String>,
    #[allow(dead_code)]
    pub resource_data: Option<ResourceData>,
}

/// Embedded resource data in a change notification.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceData {
    pub id: Option<String>,
    #[serde(rename = "@odata.type")]
    pub odata_type: Option<String>,
    #[serde(rename = "@odata.id")]
    pub odata_id: Option<String>,
}

// ── Chat Message Types ───────────────────────────────────────────

/// A Teams channel or chat message from the MS Graph API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub message_type: Option<String>,
    pub created_date_time: Option<String>,
    pub from: Option<ChatMessageFrom>,
    pub body: ChatMessageBody,
    pub channel_identity: Option<ChannelIdentity>,
    pub chat_id: Option<String>,
}

/// Sender information for a chat message.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessageFrom {
    pub user: Option<ChatMessageUser>,
    pub application: Option<ChatMessageApplication>,
}

/// User identity in a chat message sender.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageUser {
    pub id: String,
    pub display_name: Option<String>,
}

/// Application identity in a chat message sender (for bot messages).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageApplication {
    pub id: String,
    pub display_name: Option<String>,
}

/// Body content of a chat message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageBody {
    pub content: String,
    pub content_type: Option<String>,
}

/// Channel identity — which team/channel the message belongs to.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelIdentity {
    pub team_id: String,
    pub channel_id: String,
}

/// A Teams channel from the list channels API.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamChannel {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
}

/// Wrapper for list responses from MS Graph.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct GraphListResponse<T> {
    pub value: Vec<T>,
}

// ── Teams Tool Types ─────────────────────────────────────────────────

/// User presence response from `/users/{user}/presence`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceResponse {
    /// Availability status: Available, Busy, DoNotDisturb, Away, Offline, etc.
    pub availability: String,
    /// Current activity: InACall, InAMeeting, Presenting, Available, Away, etc.
    pub activity: String,
}

/// Body content of a channel message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMessageBody {
    pub content: String,
    pub content_type: Option<String>,
}

/// Sender user info embedded in a channel message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMessageUser {
    pub display_name: Option<String>,
}

/// Sender container for a channel message.
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelMessageFrom {
    pub user: Option<ChannelMessageUser>,
}

/// A single message from the channel messages list API.
///
/// Used by `get_channel_messages()` — separate from `ChatMessage` which is used
/// by the inbound webhook relay and has additional fields.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMessage {
    pub id: String,
    pub created_date_time: Option<String>,
    pub body: ChannelMessageBody,
    pub from: Option<ChannelMessageFrom>,
}

// ── Conversion ───────────────────────────────────────────────────

impl ChatMessage {
    /// Convert a Teams ChatMessage to the unified InboundMessage format.
    pub fn to_inbound_message(&self) -> InboundMessage {
        let sender = self
            .from
            .as_ref()
            .and_then(|f| {
                f.user
                    .as_ref()
                    .and_then(|u| u.display_name.clone())
                    .or_else(|| {
                        f.application
                            .as_ref()
                            .and_then(|a| a.display_name.clone())
                    })
            })
            .unwrap_or_else(|| "unknown".to_string());

        let sender_id = self
            .from
            .as_ref()
            .and_then(|f| {
                f.user
                    .as_ref()
                    .map(|u| u.id.clone())
                    .or_else(|| f.application.as_ref().map(|a| a.id.clone()))
            })
            .unwrap_or_default();

        let timestamp = self
            .created_date_time
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let (team_id, channel_id) = self
            .channel_identity
            .as_ref()
            .map(|ci| (ci.team_id.clone(), ci.channel_id.clone()))
            .unwrap_or_default();

        // Extract plain text from HTML body if content_type is "html"
        let content = if self
            .body
            .content_type
            .as_deref()
            .map(|ct| ct.eq_ignore_ascii_case("html"))
            .unwrap_or(false)
        {
            strip_html_tags(&self.body.content)
        } else {
            self.body.content.clone()
        };

        InboundMessage {
            id: self.id.clone(),
            channel: "teams".to_string(),
            sender,
            content,
            timestamp,
            thread_id: self.chat_id.clone(),
            metadata: serde_json::json!({
                "team_id": team_id,
                "channel_id": channel_id,
                "sender_id": sender_id,
                "message_type": self.message_type,
            }),
        }
    }
}

/// Naive HTML tag stripper (sufficient for Teams message bodies).
///
/// Strips `<tag>` elements then decodes the five standard HTML entities:
/// `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&nbsp;`.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    // Decode standard HTML entities (no external crate needed for these five).
    result
        .trim()
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_token_response_deserializes() {
        let json = r#"{
            "access_token": "eyJ0eXAiOiJKV1...",
            "token_type": "Bearer",
            "expires_in": 3600
        }"#;
        let resp: OAuthTokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.token_type, "Bearer");
        assert_eq!(resp.expires_in, 3600);
        assert!(resp.access_token.starts_with("eyJ"));
    }

    #[test]
    fn subscription_request_serializes_camel_case() {
        let req = SubscriptionRequest {
            change_type: "created".to_string(),
            notification_url: "https://example.com/webhooks/teams".to_string(),
            resource: "/teams/getAllMessages".to_string(),
            expiration_date_time: "2024-01-01T01:00:00Z".to_string(),
            client_state: Some("nv-secret".to_string()),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["changeType"], "created");
        assert_eq!(json["notificationUrl"], "https://example.com/webhooks/teams");
        assert_eq!(json["expirationDateTime"], "2024-01-01T01:00:00Z");
        assert_eq!(json["clientState"], "nv-secret");
    }

    #[test]
    fn subscription_response_deserializes() {
        let json = r#"{
            "id": "sub-123",
            "resource": "/teams/getAllMessages",
            "changeType": "created",
            "expirationDateTime": "2024-01-01T01:00:00Z"
        }"#;
        let resp: SubscriptionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "sub-123");
        assert_eq!(resp.resource, "/teams/getAllMessages");
    }

    #[test]
    fn change_notification_deserializes() {
        let json = r##"{
            "value": [{
                "subscriptionId": "sub-123",
                "changeType": "created",
                "resource": "teams('team-1')/channels('ch-1')/messages('msg-1')",
                "clientState": "nv-secret",
                "resourceData": {
                    "id": "msg-1",
                    "@odata.type": "#Microsoft.Graph.chatMessage",
                    "@odata.id": "teams('team-1')/channels('ch-1')/messages('msg-1')"
                }
            }]
        }"##;
        let collection: ChangeNotificationCollection = serde_json::from_str(json).unwrap();
        assert_eq!(collection.value.len(), 1);
        let notif = &collection.value[0];
        assert_eq!(notif.subscription_id, "sub-123");
        assert_eq!(notif.change_type, "created");
        let rd = notif.resource_data.as_ref().unwrap();
        assert_eq!(rd.id.as_deref(), Some("msg-1"));
    }

    #[test]
    fn chat_message_deserializes() {
        let json = r#"{
            "id": "msg-1",
            "messageType": "message",
            "createdDateTime": "2024-06-15T10:30:00Z",
            "from": {
                "user": {
                    "id": "user-1",
                    "displayName": "John Doe"
                }
            },
            "body": {
                "content": "Hello from Teams!",
                "contentType": "text"
            },
            "channelIdentity": {
                "teamId": "team-1",
                "channelId": "ch-1"
            },
            "chatId": null
        }"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.id, "msg-1");
        assert_eq!(msg.body.content, "Hello from Teams!");
        let from = msg.from.as_ref().unwrap();
        assert_eq!(from.user.as_ref().unwrap().display_name.as_deref(), Some("John Doe"));
    }

    #[test]
    fn chat_message_to_inbound_message() {
        let msg = ChatMessage {
            id: "msg-123".to_string(),
            message_type: Some("message".to_string()),
            created_date_time: Some("2024-06-15T10:30:00Z".to_string()),
            from: Some(ChatMessageFrom {
                user: Some(ChatMessageUser {
                    id: "user-1".to_string(),
                    display_name: Some("John Doe".to_string()),
                }),
                application: None,
            }),
            body: ChatMessageBody {
                content: "Hello from Teams!".to_string(),
                content_type: Some("text".to_string()),
            },
            channel_identity: Some(ChannelIdentity {
                team_id: "team-1".to_string(),
                channel_id: "ch-1".to_string(),
            }),
            chat_id: None,
        };

        let inbound = msg.to_inbound_message();
        assert_eq!(inbound.id, "msg-123");
        assert_eq!(inbound.channel, "teams");
        assert_eq!(inbound.sender, "John Doe");
        assert_eq!(inbound.content, "Hello from Teams!");
        assert_eq!(inbound.metadata["team_id"], "team-1");
        assert_eq!(inbound.metadata["channel_id"], "ch-1");
        assert_eq!(inbound.metadata["sender_id"], "user-1");
    }

    #[test]
    fn chat_message_html_body_stripped() {
        let msg = ChatMessage {
            id: "msg-html".to_string(),
            message_type: Some("message".to_string()),
            created_date_time: None,
            from: None,
            body: ChatMessageBody {
                content: "<p>Hello <b>world</b>!</p>".to_string(),
                content_type: Some("html".to_string()),
            },
            channel_identity: None,
            chat_id: None,
        };

        let inbound = msg.to_inbound_message();
        assert_eq!(inbound.content, "Hello world!");
    }

    #[test]
    fn strip_html_tags_basic() {
        assert_eq!(strip_html_tags("<p>hello</p>"), "hello");
        assert_eq!(strip_html_tags("no tags"), "no tags");
        assert_eq!(
            strip_html_tags("<div><span>nested</span></div>"),
            "nested"
        );
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn team_channel_deserializes() {
        let json = r#"{
            "id": "ch-1",
            "displayName": "General",
            "description": "The main channel"
        }"#;
        let ch: TeamChannel = serde_json::from_str(json).unwrap();
        assert_eq!(ch.id, "ch-1");
        assert_eq!(ch.display_name, "General");
        assert_eq!(ch.description.as_deref(), Some("The main channel"));
    }

    #[test]
    fn graph_list_response_deserializes() {
        let json = r#"{
            "value": [
                {"id": "ch-1", "displayName": "General", "description": null},
                {"id": "ch-2", "displayName": "Random", "description": "Off-topic"}
            ]
        }"#;
        let resp: GraphListResponse<TeamChannel> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.value.len(), 2);
        assert_eq!(resp.value[0].display_name, "General");
        assert_eq!(resp.value[1].display_name, "Random");
    }
}
