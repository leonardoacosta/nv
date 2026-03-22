use chrono::{DateTime, Utc};
use nv_core::InboundMessage;
use serde::Deserialize;

// ── Mail Message Types ──────────────────────────────────────────

/// A mail message from the MS Graph API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMailMessage {
    pub id: String,
    pub subject: Option<String>,
    pub body_preview: Option<String>,
    pub body: Option<MailBody>,
    pub from: Option<MailRecipient>,
    pub received_date_time: Option<String>,
    pub conversation_id: Option<String>,
    #[allow(dead_code)]
    pub is_read: Option<bool>,
}

/// Body content of an email message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailBody {
    pub content: String,
    pub content_type: Option<String>,
}

/// Email recipient (from, to, cc, etc.).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailRecipient {
    pub email_address: EmailAddress,
}

/// Email address with optional display name.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    pub address: String,
    pub name: Option<String>,
}

/// A mail folder from the MS Graph API.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMailFolder {
    pub id: String,
    pub display_name: String,
    #[allow(dead_code)]
    pub total_item_count: Option<i64>,
    #[allow(dead_code)]
    pub unread_item_count: Option<i64>,
}

/// Wrapper for list responses from MS Graph.
#[derive(Debug, Clone, Deserialize)]
pub struct GraphMailListResponse<T> {
    pub value: Vec<T>,
}

/// Request body for POST /me/sendMail.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMailRequest {
    pub message: SendMailMessage,
    pub save_to_sent_items: bool,
}

/// The message portion of a sendMail request.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMailMessage {
    pub subject: String,
    pub body: SendMailBody,
    pub to_recipients: Vec<SendMailRecipient>,
}

/// Body for outbound email.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMailBody {
    pub content_type: String,
    pub content: String,
}

/// Recipient for outbound email.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMailRecipient {
    pub email_address: SendMailAddress,
}

/// Address for outbound email.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SendMailAddress {
    pub address: String,
}

// ── Conversion ──────────────────────────────────────────────────

impl GraphMailMessage {
    /// Convert an MS Graph mail message to the unified InboundMessage format.
    pub fn to_inbound_message(&self, body_text: &str) -> InboundMessage {
        let sender = self
            .from
            .as_ref()
            .map(|r| {
                r.email_address
                    .name
                    .clone()
                    .unwrap_or_else(|| r.email_address.address.clone())
            })
            .unwrap_or_else(|| "unknown".to_string());

        let sender_address = self
            .from
            .as_ref()
            .map(|r| r.email_address.address.clone())
            .unwrap_or_default();

        let timestamp = self
            .received_date_time
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let subject = self.subject.clone().unwrap_or_default();

        InboundMessage {
            id: self.id.clone(),
            channel: "email".to_string(),
            sender,
            content: if subject.is_empty() {
                body_text.to_string()
            } else {
                format!("[{subject}] {body_text}")
            },
            timestamp,
            thread_id: self.conversation_id.clone(),
            metadata: serde_json::json!({
                "message_id": self.id,
                "sender_address": sender_address,
                "subject": self.subject,
                "conversation_id": self.conversation_id,
            }),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_mail_message_deserializes() {
        let json = r#"{
            "id": "AAMkAGI2...",
            "subject": "Test Email",
            "bodyPreview": "Hello world",
            "body": {
                "content": "<html><body><p>Hello world</p></body></html>",
                "contentType": "html"
            },
            "from": {
                "emailAddress": {
                    "address": "john@example.com",
                    "name": "John Doe"
                }
            },
            "receivedDateTime": "2024-06-15T10:30:00Z",
            "conversationId": "conv-123",
            "isRead": false
        }"#;
        let msg: GraphMailMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.id, "AAMkAGI2...");
        assert_eq!(msg.subject.as_deref(), Some("Test Email"));
        assert_eq!(msg.body_preview.as_deref(), Some("Hello world"));
        let body = msg.body.as_ref().unwrap();
        assert!(body.content.contains("<p>Hello world</p>"));
        assert_eq!(body.content_type.as_deref(), Some("html"));
        let from = msg.from.as_ref().unwrap();
        assert_eq!(from.email_address.address, "john@example.com");
        assert_eq!(from.email_address.name.as_deref(), Some("John Doe"));
        assert_eq!(msg.received_date_time.as_deref(), Some("2024-06-15T10:30:00Z"));
        assert_eq!(msg.conversation_id.as_deref(), Some("conv-123"));
        assert_eq!(msg.is_read, Some(false));
    }

    #[test]
    fn graph_mail_message_minimal_deserializes() {
        let json = r#"{
            "id": "msg-1",
            "subject": null,
            "bodyPreview": null,
            "body": null,
            "from": null,
            "receivedDateTime": null,
            "conversationId": null,
            "isRead": null
        }"#;
        let msg: GraphMailMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.id, "msg-1");
        assert!(msg.subject.is_none());
        assert!(msg.from.is_none());
    }

    #[test]
    fn graph_mail_folder_deserializes() {
        let json = r#"{
            "id": "folder-123",
            "displayName": "Inbox",
            "totalItemCount": 42,
            "unreadItemCount": 5
        }"#;
        let folder: GraphMailFolder = serde_json::from_str(json).unwrap();
        assert_eq!(folder.id, "folder-123");
        assert_eq!(folder.display_name, "Inbox");
        assert_eq!(folder.total_item_count, Some(42));
        assert_eq!(folder.unread_item_count, Some(5));
    }

    #[test]
    fn graph_mail_list_response_deserializes() {
        let json = r#"{
            "value": [
                {
                    "id": "msg-1",
                    "subject": "Hello",
                    "bodyPreview": "Hi there",
                    "body": null,
                    "from": null,
                    "receivedDateTime": "2024-06-15T10:30:00Z",
                    "conversationId": null,
                    "isRead": true
                }
            ]
        }"#;
        let resp: GraphMailListResponse<GraphMailMessage> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.value.len(), 1);
        assert_eq!(resp.value[0].id, "msg-1");
    }

    #[test]
    fn to_inbound_message_with_subject() {
        let msg = GraphMailMessage {
            id: "msg-123".to_string(),
            subject: Some("Important".to_string()),
            body_preview: None,
            body: None,
            from: Some(MailRecipient {
                email_address: EmailAddress {
                    address: "john@example.com".to_string(),
                    name: Some("John Doe".to_string()),
                },
            }),
            received_date_time: Some("2024-06-15T10:30:00Z".to_string()),
            conversation_id: Some("conv-1".to_string()),
            is_read: Some(false),
        };

        let inbound = msg.to_inbound_message("Hello there");
        assert_eq!(inbound.id, "msg-123");
        assert_eq!(inbound.channel, "email");
        assert_eq!(inbound.sender, "John Doe");
        assert_eq!(inbound.content, "[Important] Hello there");
        assert_eq!(inbound.thread_id.as_deref(), Some("conv-1"));
        assert_eq!(inbound.metadata["sender_address"], "john@example.com");
        assert_eq!(inbound.metadata["message_id"], "msg-123");
    }

    #[test]
    fn to_inbound_message_without_subject() {
        let msg = GraphMailMessage {
            id: "msg-456".to_string(),
            subject: None,
            body_preview: None,
            body: None,
            from: Some(MailRecipient {
                email_address: EmailAddress {
                    address: "jane@example.com".to_string(),
                    name: None,
                },
            }),
            received_date_time: None,
            conversation_id: None,
            is_read: None,
        };

        let inbound = msg.to_inbound_message("Body text");
        assert_eq!(inbound.sender, "jane@example.com");
        assert_eq!(inbound.content, "Body text");
    }

    #[test]
    fn to_inbound_message_no_from() {
        let msg = GraphMailMessage {
            id: "msg-789".to_string(),
            subject: Some("Test".to_string()),
            body_preview: None,
            body: None,
            from: None,
            received_date_time: None,
            conversation_id: None,
            is_read: None,
        };

        let inbound = msg.to_inbound_message("Content");
        assert_eq!(inbound.sender, "unknown");
    }

    #[test]
    fn send_mail_request_serializes_camel_case() {
        let req = SendMailRequest {
            message: SendMailMessage {
                subject: "Re: Test".to_string(),
                body: SendMailBody {
                    content_type: "Text".to_string(),
                    content: "Reply body".to_string(),
                },
                to_recipients: vec![SendMailRecipient {
                    email_address: SendMailAddress {
                        address: "test@example.com".to_string(),
                    },
                }],
            },
            save_to_sent_items: true,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["saveToSentItems"], true);
        assert_eq!(json["message"]["subject"], "Re: Test");
        assert_eq!(json["message"]["body"]["contentType"], "Text");
        assert_eq!(json["message"]["toRecipients"][0]["emailAddress"]["address"], "test@example.com");
    }
}
