//! BlueBubbles REST API response types.
//!
//! Maps directly to the v1 API JSON payloads. Only the fields Nova
//! needs are included — unknown fields are silently ignored via
//! `#[serde(default)]` and deny_unknown_fields is NOT set.

use serde::Deserialize;

/// Wrapper for paginated message list responses.
///
/// GET /api/v1/message returns `{ "status": 200, "data": [...] }`.
#[derive(Debug, Deserialize)]
pub struct BbMessageResponse {
    pub data: Vec<BbMessage>,
}

/// A single iMessage as returned by BlueBubbles.
#[derive(Debug, Clone, Deserialize)]
pub struct BbMessage {
    /// Unique message GUID (e.g. "p:0/...").
    pub guid: String,

    /// Message body text. Null for attachments-only messages.
    #[serde(default)]
    pub text: Option<String>,

    /// Unix timestamp in milliseconds when the message was created.
    #[serde(alias = "dateCreated")]
    pub date_created: i64,

    /// The sender handle (phone/email). Null for outgoing messages.
    pub handle: Option<BbHandle>,

    /// Chat GUID this message belongs to (e.g. "iMessage;-;+1234567890").
    #[serde(alias = "chats", default)]
    pub chats: Vec<BbChat>,

    /// Whether this message was sent by the BlueBubbles host (us).
    #[serde(alias = "isFromMe", default)]
    pub is_from_me: bool,
}

/// A chat (conversation) in BlueBubbles.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Fields kept for API completeness / future use
pub struct BbChat {
    /// Chat GUID (e.g. "iMessage;-;+1234567890").
    pub guid: String,

    /// Human-readable display name, if set.
    #[serde(alias = "displayName", default)]
    pub display_name: Option<String>,

    /// Participants in this chat.
    #[serde(default)]
    pub participants: Vec<BbHandle>,
}

/// A handle (contact address) in BlueBubbles.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Fields kept for API completeness / future use
pub struct BbHandle {
    /// The address (phone number or email).
    pub address: String,

    /// Service type: "iMessage" or "SMS".
    #[serde(default)]
    pub service: Option<String>,
}

/// Response from POST /api/v1/message/text.
#[derive(Debug, Deserialize)]
pub struct BbSendResponse {
    pub status: i32,
    #[serde(default)]
    pub message: Option<String>,
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_message_response() {
        let json = r#"{
            "status": 200,
            "data": [
                {
                    "guid": "p:0/AABBCCDD-1234",
                    "text": "Hello from iPhone",
                    "dateCreated": 1711000000000,
                    "handle": {
                        "address": "+15551234567",
                        "service": "iMessage"
                    },
                    "chats": [
                        {
                            "guid": "iMessage;-;+15551234567",
                            "displayName": null,
                            "participants": [
                                { "address": "+15551234567", "service": "iMessage" }
                            ]
                        }
                    ],
                    "isFromMe": false
                }
            ]
        }"#;

        let resp: BbMessageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);

        let msg = &resp.data[0];
        assert_eq!(msg.guid, "p:0/AABBCCDD-1234");
        assert_eq!(msg.text.as_deref(), Some("Hello from iPhone"));
        assert_eq!(msg.date_created, 1_711_000_000_000);
        assert!(!msg.is_from_me);

        let handle = msg.handle.as_ref().unwrap();
        assert_eq!(handle.address, "+15551234567");
        assert_eq!(handle.service.as_deref(), Some("iMessage"));

        assert_eq!(msg.chats.len(), 1);
        assert_eq!(msg.chats[0].guid, "iMessage;-;+15551234567");
    }

    #[test]
    fn deserialize_message_no_text() {
        let json = r#"{
            "guid": "p:0/EEFF0011-5678",
            "dateCreated": 1711000001000,
            "handle": { "address": "+15559876543" },
            "chats": [],
            "isFromMe": false
        }"#;

        let msg: BbMessage = serde_json::from_str(json).unwrap();
        assert!(msg.text.is_none());
        assert!(msg.handle.as_ref().unwrap().service.is_none());
    }

    #[test]
    fn deserialize_outgoing_message() {
        let json = r#"{
            "guid": "p:1/OUT-9999",
            "text": "Sent from Mac",
            "dateCreated": 1711000002000,
            "handle": null,
            "chats": [],
            "isFromMe": true
        }"#;

        let msg: BbMessage = serde_json::from_str(json).unwrap();
        assert!(msg.is_from_me);
        assert!(msg.handle.is_none());
    }

    #[test]
    fn deserialize_send_response() {
        let json = r#"{ "status": 200, "message": "Message sent!" }"#;
        let resp: BbSendResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.message.as_deref(), Some("Message sent!"));
    }

    #[test]
    fn deserialize_chat() {
        let json = r#"{
            "guid": "iMessage;-;+15551112222",
            "displayName": "Work Chat",
            "participants": [
                { "address": "+15551112222", "service": "iMessage" },
                { "address": "+15553334444", "service": "SMS" }
            ]
        }"#;

        let chat: BbChat = serde_json::from_str(json).unwrap();
        assert_eq!(chat.guid, "iMessage;-;+15551112222");
        assert_eq!(chat.display_name.as_deref(), Some("Work Chat"));
        assert_eq!(chat.participants.len(), 2);
        assert_eq!(chat.participants[1].service.as_deref(), Some("SMS"));
    }
}
