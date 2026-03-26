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

    /// Session error keyboard with Retry and Create Bug buttons.
    ///
    /// Layout: `[🔄 Retry] [🐛 Create Bug]`
    pub fn session_error(event_id: &str) -> Self {
        Self {
            rows: vec![vec![
                InlineButton {
                    text: "\u{1F504} Retry".to_string(),
                    callback_data: format!("retry:{event_id}"),
                },
                InlineButton {
                    text: "\u{1F41B} Create Bug".to_string(),
                    callback_data: format!("bug:{event_id}"),
                },
            ]],
        }
    }

    /// Reminder action keyboard attached to fired reminder notifications.
    ///
    /// Layout: `[Mark Done | Snooze | Backlog]`
    pub fn reminder_actions(reminder_id: i64) -> Self {
        Self {
            rows: vec![vec![
                InlineButton {
                    text: "Mark Done".to_string(),
                    callback_data: format!("reminder_done:{reminder_id}"),
                },
                InlineButton {
                    text: "Snooze".to_string(),
                    callback_data: format!("reminder_snooze:{reminder_id}"),
                },
                InlineButton {
                    text: "Backlog".to_string(),
                    callback_data: format!("reminder_backlog:{reminder_id}"),
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
    /// 7am daily morning briefing: open obligation summary.
    MorningBriefing,
    /// A user-created recurring schedule fired from the scheduler loop.
    UserSchedule { name: String, action: String },
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
    /// Proactive cross-channel send (user-confirmed). Distinct from ChannelReply
    /// which is used for reply routing.
    ChannelSend,
    HaServiceCall,
    NexusStartSession,
    NexusStopSession,
    /// Start a CC subprocess session (CcSessionManager).
    CcStartSession,
    /// Stop a CC subprocess session (CcSessionManager).
    CcStopSession,
    /// Create a new user-defined recurring schedule in SQLite.
    ScheduleAdd,
    /// Modify an existing user-defined recurring schedule (cron expr or enabled state).
    ScheduleModify,
    /// Delete a user-defined recurring schedule by name.
    ScheduleRemove,
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

// ── Obligation Types ─────────────────────────────────────────────────

/// Lifecycle status of a tracked obligation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObligationStatus {
    /// Newly detected, no action taken.
    Open,
    /// Work has started on this obligation.
    InProgress,
    /// Obligation has been fulfilled.
    Done,
    /// Deliberately ignored or not applicable.
    Dismissed,
}

impl ObligationStatus {
    /// Canonical string value stored in SQLite.
    pub fn as_str(&self) -> &'static str {
        match self {
            ObligationStatus::Open => "open",
            ObligationStatus::InProgress => "in_progress",
            ObligationStatus::Done => "done",
            ObligationStatus::Dismissed => "dismissed",
        }
    }
}

impl std::fmt::Display for ObligationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ObligationStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(ObligationStatus::Open),
            "in_progress" => Ok(ObligationStatus::InProgress),
            "done" => Ok(ObligationStatus::Done),
            "dismissed" => Ok(ObligationStatus::Dismissed),
            other => Err(anyhow::anyhow!("unknown ObligationStatus: {other}")),
        }
    }
}

/// Who is responsible for fulfilling the obligation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObligationOwner {
    /// Nova (the AI daemon) is responsible.
    Nova,
    /// Leo (the human user) is responsible.
    Leo,
}

impl ObligationOwner {
    /// Canonical string value stored in SQLite.
    pub fn as_str(&self) -> &'static str {
        match self {
            ObligationOwner::Nova => "nova",
            ObligationOwner::Leo => "leo",
        }
    }
}

impl std::fmt::Display for ObligationOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ObligationOwner {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "nova" => Ok(ObligationOwner::Nova),
            "leo" => Ok(ObligationOwner::Leo),
            other => Err(anyhow::anyhow!("unknown ObligationOwner: {other}")),
        }
    }
}

/// A tracked obligation detected from an inbound message.
///
/// Obligations represent commitments or actions that were identified in
/// messages across any channel. They are persisted in messages.db and
/// survive daemon restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Obligation {
    /// UUID primary key.
    pub id: String,
    /// Channel the obligation was detected in (e.g. "telegram", "discord").
    pub source_channel: String,
    /// Excerpt or identifier of the source message.
    pub source_message: Option<String>,
    /// The specific action or commitment detected.
    pub detected_action: String,
    /// Optional project code this obligation belongs to (e.g. "NV", "OO").
    pub project_code: Option<String>,
    /// Priority 0-4 (0 = highest/critical, 4 = backlog).
    pub priority: i32,
    /// Current lifecycle status.
    pub status: ObligationStatus,
    /// Who is responsible for this obligation.
    pub owner: ObligationOwner,
    /// Optional reasoning for the owner assignment.
    pub owner_reason: Option<String>,
    /// ISO 8601 UTC creation timestamp.
    pub created_at: String,
    /// ISO 8601 UTC last-updated timestamp.
    pub updated_at: String,
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

    #[test]
    fn reminder_actions_keyboard_layout() {
        let kb = InlineKeyboard::reminder_actions(42);
        assert_eq!(kb.rows.len(), 1);
        assert_eq!(kb.rows[0].len(), 3);

        assert_eq!(kb.rows[0][0].text, "Mark Done");
        assert_eq!(kb.rows[0][0].callback_data, "reminder_done:42");

        assert_eq!(kb.rows[0][1].text, "Snooze");
        assert_eq!(kb.rows[0][1].callback_data, "reminder_snooze:42");

        assert_eq!(kb.rows[0][2].text, "Backlog");
        assert_eq!(kb.rows[0][2].callback_data, "reminder_backlog:42");
    }

    #[test]
    fn session_error_keyboard_layout() {
        let kb = InlineKeyboard::session_error("evt-abc123");
        assert_eq!(kb.rows.len(), 1);
        assert_eq!(kb.rows[0].len(), 2);

        assert!(kb.rows[0][0].text.contains("Retry"));
        assert_eq!(kb.rows[0][0].callback_data, "retry:evt-abc123");

        assert!(kb.rows[0][1].text.contains("Create Bug"));
        assert_eq!(kb.rows[0][1].callback_data, "bug:evt-abc123");
    }
}
