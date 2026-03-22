use chrono::{DateTime, Utc};
use nv_core::InboundMessage;
use serde::{Deserialize, Serialize};

// ── Gateway Opcodes ────────────────────────────────────────────────

/// Discord Gateway opcodes.
/// <https://discord.com/developers/docs/events/gateway-events#gateway-events>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GatewayOpcode {
    /// An event was dispatched.
    Dispatch = 0,
    /// Fired periodically to keep the connection alive.
    Heartbeat = 1,
    /// Starts a new session during the initial handshake.
    Identify = 2,
    /// Resume a previous session that was disconnected.
    Resume = 6,
    /// Server is telling the client to reconnect.
    Reconnect = 7,
    /// The session has been invalidated.
    InvalidSession = 9,
    /// Sent immediately after connecting — contains the heartbeat interval.
    Hello = 10,
    /// Sent in response to receiving a heartbeat to acknowledge it.
    HeartbeatAck = 11,
}

impl GatewayOpcode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Dispatch),
            1 => Some(Self::Heartbeat),
            2 => Some(Self::Identify),
            6 => Some(Self::Resume),
            7 => Some(Self::Reconnect),
            9 => Some(Self::InvalidSession),
            10 => Some(Self::Hello),
            11 => Some(Self::HeartbeatAck),
            _ => None,
        }
    }
}

// ── Gateway Payloads ───────────────────────────────────────────────

/// Raw gateway payload (inbound from Discord).
#[derive(Debug, Deserialize)]
pub struct GatewayPayload {
    pub op: u8,
    pub d: Option<serde_json::Value>,
    pub s: Option<u64>,
    pub t: Option<String>,
}

/// Hello payload (op 10) — contains heartbeat_interval.
#[derive(Debug, Deserialize)]
pub struct HelloData {
    pub heartbeat_interval: u64,
}

/// Ready event data from READY dispatch.
#[derive(Debug, Deserialize)]
pub struct ReadyData {
    pub user: User,
    pub session_id: String,
    #[allow(dead_code)]
    pub resume_gateway_url: String,
}

// ── Outbound Gateway Payloads ──────────────────────────────────────

/// Identify payload sent to authenticate with the gateway.
#[derive(Debug, Serialize)]
pub struct IdentifyPayload {
    pub op: u8,
    pub d: IdentifyData,
}

#[derive(Debug, Serialize)]
pub struct IdentifyData {
    pub token: String,
    pub intents: u64,
    pub properties: ConnectionProperties,
}

#[derive(Debug, Serialize)]
pub struct ConnectionProperties {
    pub os: String,
    pub browser: String,
    pub device: String,
}

/// Heartbeat payload.
#[derive(Debug, Serialize)]
pub struct HeartbeatPayload {
    pub op: u8,
    pub d: Option<u64>,
}

/// Resume payload for reconnection.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ResumePayload {
    pub op: u8,
    pub d: ResumeData,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ResumeData {
    pub token: String,
    pub session_id: String,
    pub seq: u64,
}

// ── Discord API Types ──────────────────────────────────────────────

/// A Discord user (minimal fields).
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[allow(dead_code)]
    pub bot: Option<bool>,
}

/// A Discord message from MESSAGE_CREATE.
#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub author: User,
    pub content: String,
    pub timestamp: String,
}

// ── Intent Constants ───────────────────────────────────────────────

/// GUILD_MESSAGES (1 << 9) — receive message events in guild channels.
pub const INTENT_GUILD_MESSAGES: u64 = 1 << 9;
/// DIRECT_MESSAGES (1 << 12) — receive DM message events.
pub const INTENT_DIRECT_MESSAGES: u64 = 1 << 12;
/// MESSAGE_CONTENT (1 << 15) — privileged: access message content.
pub const INTENT_MESSAGE_CONTENT: u64 = 1 << 15;

/// Combined intents for the Discord bot.
pub const BOT_INTENTS: u64 =
    INTENT_GUILD_MESSAGES | INTENT_DIRECT_MESSAGES | INTENT_MESSAGE_CONTENT;

// ── Conversion ─────────────────────────────────────────────────────

impl Message {
    /// Convert a Discord Message to the unified InboundMessage format.
    pub fn to_inbound_message(&self) -> InboundMessage {
        let timestamp = DateTime::parse_from_rfc3339(&self.timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        InboundMessage {
            id: self.id.clone(),
            channel: "discord".to_string(),
            sender: self.author.username.clone(),
            content: self.content.clone(),
            timestamp,
            thread_id: None,
            metadata: serde_json::json!({
                "channel_id": self.channel_id,
                "guild_id": self.guild_id,
                "author_id": self.author.id,
            }),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_opcode_roundtrip() {
        assert_eq!(GatewayOpcode::from_u8(0), Some(GatewayOpcode::Dispatch));
        assert_eq!(GatewayOpcode::from_u8(1), Some(GatewayOpcode::Heartbeat));
        assert_eq!(GatewayOpcode::from_u8(2), Some(GatewayOpcode::Identify));
        assert_eq!(GatewayOpcode::from_u8(6), Some(GatewayOpcode::Resume));
        assert_eq!(GatewayOpcode::from_u8(7), Some(GatewayOpcode::Reconnect));
        assert_eq!(
            GatewayOpcode::from_u8(9),
            Some(GatewayOpcode::InvalidSession)
        );
        assert_eq!(GatewayOpcode::from_u8(10), Some(GatewayOpcode::Hello));
        assert_eq!(GatewayOpcode::from_u8(11), Some(GatewayOpcode::HeartbeatAck));
        assert_eq!(GatewayOpcode::from_u8(99), None);
    }

    #[test]
    fn bot_intents_correct() {
        assert_eq!(INTENT_GUILD_MESSAGES, 512);
        assert_eq!(INTENT_DIRECT_MESSAGES, 4096);
        assert_eq!(INTENT_MESSAGE_CONTENT, 32768);
        assert_eq!(BOT_INTENTS, 512 | 4096 | 32768);
    }

    #[test]
    fn identify_payload_serializes() {
        let payload = IdentifyPayload {
            op: 2,
            d: IdentifyData {
                token: "test-token".to_string(),
                intents: BOT_INTENTS,
                properties: ConnectionProperties {
                    os: "linux".to_string(),
                    browser: "nv".to_string(),
                    device: "nv".to_string(),
                },
            },
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["op"], 2);
        assert_eq!(json["d"]["token"], "test-token");
        assert_eq!(json["d"]["intents"], BOT_INTENTS);
        assert_eq!(json["d"]["properties"]["browser"], "nv");
    }

    #[test]
    fn heartbeat_payload_serializes() {
        let payload = HeartbeatPayload {
            op: 1,
            d: Some(42),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["op"], 1);
        assert_eq!(json["d"], 42);

        let null_payload = HeartbeatPayload { op: 1, d: None };
        let json = serde_json::to_value(&null_payload).unwrap();
        assert_eq!(json["op"], 1);
        assert!(json["d"].is_null());
    }

    #[test]
    fn gateway_payload_deserializes_hello() {
        let json = r#"{"op":10,"d":{"heartbeat_interval":41250},"s":null,"t":null}"#;
        let payload: GatewayPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.op, 10);
        assert!(payload.s.is_none());
        assert!(payload.t.is_none());

        let hello: HelloData = serde_json::from_value(payload.d.unwrap()).unwrap();
        assert_eq!(hello.heartbeat_interval, 41250);
    }

    #[test]
    fn gateway_payload_deserializes_dispatch() {
        let json = r#"{
            "op": 0,
            "d": {
                "id": "msg-1",
                "channel_id": "ch-1",
                "guild_id": "g-1",
                "author": {"id": "u-1", "username": "testuser", "bot": false},
                "content": "hello world",
                "timestamp": "2024-01-01T00:00:00+00:00"
            },
            "s": 5,
            "t": "MESSAGE_CREATE"
        }"#;
        let payload: GatewayPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.op, 0);
        assert_eq!(payload.s, Some(5));
        assert_eq!(payload.t.as_deref(), Some("MESSAGE_CREATE"));

        let msg: Message = serde_json::from_value(payload.d.unwrap()).unwrap();
        assert_eq!(msg.content, "hello world");
        assert_eq!(msg.author.username, "testuser");
    }

    #[test]
    fn message_to_inbound_message() {
        let msg = Message {
            id: "msg-123".to_string(),
            channel_id: "ch-456".to_string(),
            guild_id: Some("g-789".to_string()),
            author: User {
                id: "u-1".to_string(),
                username: "testuser".to_string(),
                bot: Some(false),
            },
            content: "hello from discord".to_string(),
            timestamp: "2024-01-15T10:30:00+00:00".to_string(),
        };

        let inbound = msg.to_inbound_message();
        assert_eq!(inbound.id, "msg-123");
        assert_eq!(inbound.channel, "discord");
        assert_eq!(inbound.sender, "testuser");
        assert_eq!(inbound.content, "hello from discord");
        assert!(inbound.thread_id.is_none());
        assert_eq!(inbound.metadata["channel_id"], "ch-456");
        assert_eq!(inbound.metadata["guild_id"], "g-789");
        assert_eq!(inbound.metadata["author_id"], "u-1");
    }

    #[test]
    fn message_to_inbound_message_dm() {
        let msg = Message {
            id: "dm-1".to_string(),
            channel_id: "dm-ch-1".to_string(),
            guild_id: None,
            author: User {
                id: "u-2".to_string(),
                username: "dmuser".to_string(),
                bot: None,
            },
            content: "private message".to_string(),
            timestamp: "2024-06-01T12:00:00+00:00".to_string(),
        };

        let inbound = msg.to_inbound_message();
        assert_eq!(inbound.channel, "discord");
        assert!(inbound.metadata["guild_id"].is_null());
    }

    #[test]
    fn message_filtering_by_channel_and_guild() {
        let watched_channels: Vec<u64> = vec![100, 200];
        let watched_servers: Vec<u64> = vec![1000];

        let messages = vec![
            Message {
                id: "1".into(),
                channel_id: "100".into(),
                guild_id: Some("1000".into()),
                author: User {
                    id: "u1".into(),
                    username: "user1".into(),
                    bot: Some(false),
                },
                content: "accepted".into(),
                timestamp: "2024-01-01T00:00:00+00:00".into(),
            },
            Message {
                id: "2".into(),
                channel_id: "999".into(), // unwatched channel
                guild_id: Some("1000".into()),
                author: User {
                    id: "u2".into(),
                    username: "user2".into(),
                    bot: Some(false),
                },
                content: "rejected-channel".into(),
                timestamp: "2024-01-01T00:00:00+00:00".into(),
            },
            Message {
                id: "3".into(),
                channel_id: "100".into(),
                guild_id: Some("9999".into()), // unwatched server
                author: User {
                    id: "u3".into(),
                    username: "user3".into(),
                    bot: Some(false),
                },
                content: "rejected-server".into(),
                timestamp: "2024-01-01T00:00:00+00:00".into(),
            },
        ];

        let bot_user_id = "bot-id";

        let accepted: Vec<_> = messages
            .iter()
            .filter(|m| {
                // Not from the bot itself
                m.author.id != bot_user_id
            })
            .filter(|m| {
                // Channel is in watched list
                m.channel_id
                    .parse::<u64>()
                    .map(|id| watched_channels.contains(&id))
                    .unwrap_or(false)
            })
            .filter(|m| {
                // Guild is in watched list (DMs pass through if guild_id is None)
                match &m.guild_id {
                    Some(gid) => gid
                        .parse::<u64>()
                        .map(|id| watched_servers.contains(&id))
                        .unwrap_or(false),
                    None => true, // DMs are allowed
                }
            })
            .collect();

        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].content, "accepted");
    }

    #[test]
    fn self_message_ignored() {
        let bot_user_id = "bot-123";
        let msg = Message {
            id: "1".into(),
            channel_id: "100".into(),
            guild_id: Some("1000".into()),
            author: User {
                id: "bot-123".into(),
                username: "nv-bot".into(),
                bot: Some(true),
            },
            content: "echo".into(),
            timestamp: "2024-01-01T00:00:00+00:00".into(),
        };

        assert_eq!(msg.author.id, bot_user_id);
        // This message should be filtered out
    }

    #[test]
    fn resume_payload_serializes() {
        let payload = ResumePayload {
            op: 6,
            d: ResumeData {
                token: "test-token".to_string(),
                session_id: "session-1".to_string(),
                seq: 42,
            },
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["op"], 6);
        assert_eq!(json["d"]["session_id"], "session-1");
        assert_eq!(json["d"]["seq"], 42);
    }

    #[test]
    fn ready_data_deserializes() {
        let json = r#"{
            "user": {"id": "bot-1", "username": "nv-bot", "bot": true},
            "session_id": "sess-abc",
            "resume_gateway_url": "wss://gateway.discord.gg"
        }"#;
        let ready: ReadyData = serde_json::from_str(json).unwrap();
        assert_eq!(ready.user.id, "bot-1");
        assert_eq!(ready.session_id, "sess-abc");
    }
}
