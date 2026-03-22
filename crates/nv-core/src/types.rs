use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Messages ────────────────────────────────────────────────────────

/// Unified inbound message from any channel.
///
/// The `metadata` field carries channel-specific data (e.g. Telegram
/// `message_id`, callback query data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub id: String,
    pub channel: String,
    pub sender: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub thread_id: Option<String>,
    pub metadata: serde_json::Value,
}

/// Message to send through a channel.
///
/// The `keyboard` field supports inline keyboards (Telegram) or
/// reactions (Discord).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub channel: String,
    pub content: String,
    pub reply_to: Option<String>,
    pub keyboard: Option<InlineKeyboard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineKeyboard {
    pub rows: Vec<Vec<InlineButton>>,
}

impl InlineKeyboard {
    /// Standard 3-button action confirmation keyboard.
    ///
    /// Layout: `[Approve] [Edit] [Cancel]`
    pub fn confirm_action(action_id: &str) -> Self {
        Self {
            rows: vec![vec![
                InlineButton {
                    text: "Approve".to_string(),
                    callback_data: format!("approve:{action_id}"),
                },
                InlineButton {
                    text: "Edit".to_string(),
                    callback_data: format!("edit:{action_id}"),
                },
                InlineButton {
                    text: "Cancel".to_string(),
                    callback_data: format!("cancel:{action_id}"),
                },
            ]],
        }
    }

    /// Build a keyboard from pending actions -- one button per action.
    pub fn from_actions(actions: &[PendingAction]) -> Self {
        Self {
            rows: actions
                .iter()
                .map(|a| {
                    vec![InlineButton {
                        text: a.description.clone(),
                        callback_data: format!("action:{}", a.id),
                    }]
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineButton {
    pub text: String,
    pub callback_data: String,
}

// ── Trigger ─────────────────────────────────────────────────────────

/// The unified event type pushed into the agent loop's `mpsc` channel.
#[derive(Debug)]
pub enum Trigger {
    /// Inbound message from any channel.
    Message(InboundMessage),
    /// Scheduled cron event (digest, cleanup).
    Cron(CronEvent),
    /// Event from a Nexus agent session.
    NexusEvent(SessionEvent),
    /// Command from the CLI.
    CliCommand(CliRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CronEvent {
    Digest,
    MemoryCleanup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub agent_name: String,
    pub session_id: String,
    pub event_type: SessionEventType,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEventType {
    Started,
    Completed,
    Failed,
    Progress,
}

#[derive(Debug)]
pub struct CliRequest {
    pub command: CliCommand,
    pub response_tx: Option<tokio::sync::oneshot::Sender<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CliCommand {
    Status,
    Ask(String),
    DigestNow,
}

// ── Agent Response ──────────────────────────────────────────────────

/// What the agent loop produces after processing a trigger through
/// Claude. Determines routing: replies go to channels, actions go to
/// pending confirmation, digests go to Telegram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentResponse {
    /// Direct reply to a channel message.
    Reply {
        channel: String,
        content: String,
        reply_to: Option<String>,
        keyboard: Option<InlineKeyboard>,
    },
    /// Action requiring confirmation.
    Action(PendingAction),
    /// Proactive digest to send to Telegram.
    Digest {
        content: String,
        suggested_actions: Vec<PendingAction>,
    },
    /// Answer to a CLI query.
    QueryAnswer(String),
    /// No response needed (message was informational).
    NoOp,
}

// ── Pending Action ──────────────────────────────────────────────────

/// An action drafted by Claude that requires confirmation via Telegram
/// inline keyboard before execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    pub id: Uuid,
    pub description: String,
    pub action_type: ActionType,
    pub payload: serde_json::Value,
    pub status: ActionStatus,
    pub created_at: DateTime<Utc>,
    /// Telegram message ID where the confirmation keyboard was sent.
    #[serde(default)]
    pub telegram_message_id: Option<i64>,
    /// Telegram chat ID where the confirmation keyboard was sent.
    #[serde(default)]
    pub telegram_chat_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    JiraCreate,
    JiraTransition,
    JiraAssign,
    JiraComment,
    ChannelReply,
    HaServiceCall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Failed,
    Cancelled,
    Expired,
}

// ── Query Types ─────────────────────────────────────────────────────

/// Source types for query answer citations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Jira,
    Memory,
    Nexus,
}

/// A source citation in a query answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCitation {
    pub source_type: SourceType,
    pub reference: String,
    pub snippet: String,
}

/// A follow-up action suggested after a query answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUpAction {
    pub index: u8,
    pub label: String,
    pub action_type: ActionType,
    pub payload: serde_json::Value,
}

/// Structured query answer from the synthesis pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAnswer {
    pub text: String,
    pub sources: Vec<SourceCitation>,
    pub followups: Vec<FollowUpAction>,
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbound_message_round_trip() {
        let msg = InboundMessage {
            id: "msg-1".into(),
            channel: "telegram".into(),
            sender: "leo".into(),
            content: "hello".into(),
            timestamp: Utc::now(),
            thread_id: Some("t-1".into()),
            metadata: serde_json::json!({"message_id": 42}),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let restored: InboundMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.id, "msg-1");
        assert_eq!(restored.channel, "telegram");
        assert_eq!(restored.sender, "leo");
        assert_eq!(restored.content, "hello");
        assert_eq!(restored.thread_id.as_deref(), Some("t-1"));
        assert_eq!(restored.metadata["message_id"], 42);
        assert_eq!(restored.timestamp, msg.timestamp);
    }

    #[test]
    fn outbound_message_with_keyboard_serializes() {
        let msg = OutboundMessage {
            channel: "telegram".into(),
            content: "Pick one".into(),
            reply_to: Some("msg-1".into()),
            keyboard: Some(InlineKeyboard {
                rows: vec![vec![
                    InlineButton {
                        text: "Approve".into(),
                        callback_data: "approve_123".into(),
                    },
                    InlineButton {
                        text: "Reject".into(),
                        callback_data: "reject_123".into(),
                    },
                ]],
            }),
        };

        let json = serde_json::to_string_pretty(&msg).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["channel"], "telegram");
        assert_eq!(value["keyboard"]["rows"][0][0]["text"], "Approve");
        assert_eq!(
            value["keyboard"]["rows"][0][1]["callback_data"],
            "reject_123"
        );
    }

    #[test]
    fn pending_action_defaults() {
        let action = PendingAction {
            id: Uuid::new_v4(),
            description: "Create JIRA ticket".into(),
            action_type: ActionType::JiraCreate,
            payload: serde_json::json!({"project": "PROJ", "summary": "Bug fix"}),
            status: ActionStatus::Pending,
            created_at: Utc::now(),
            telegram_message_id: None,
            telegram_chat_id: None,
        };

        // UUID is valid (non-nil)
        assert!(!action.id.is_nil());
        assert_eq!(action.status, ActionStatus::Pending);

        // Round-trip through JSON
        let json = serde_json::to_string(&action).unwrap();
        let restored: PendingAction = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, action.id);
        assert_eq!(restored.status, ActionStatus::Pending);
        assert_eq!(restored.description, "Create JIRA ticket");
    }
}
