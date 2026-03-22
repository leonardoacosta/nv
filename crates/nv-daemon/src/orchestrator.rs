//! Non-blocking orchestrator that receives triggers, reacts instantly,
//! classifies them, and dispatches workers for processing.
//!
//! The orchestrator NEVER blocks on Claude — only workers do. Callback
//! routing (approve/edit/cancel) is handled inline since those are fast
//! operations that don't need AI.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use nv_core::types::{InlineKeyboard, OutboundMessage, Trigger};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::agent::ChannelRegistry;
use crate::nexus;
use crate::telegram::client::TelegramClient;
use crate::worker::{Priority, SharedDeps, WorkerPool, WorkerTask};

// ── Constants ───────────────────────────────────────────────────────

/// How often to check for expired pending actions.
const EXPIRY_CHECK_INTERVAL: Duration = Duration::from_secs(300);

// ── Trigger Classification ──────────────────────────────────────────

/// Classified trigger type for dispatch decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerClass {
    /// Direct commands ("create", "assign", "move", "close").
    Command,
    /// Information queries ("what", "status", "how many", "who").
    Query,
    /// Chat acknowledgments ("thanks", "ok", "got it") — respond inline.
    Chat,
    /// Scheduled digest or cron event.
    Digest,
    /// Callback from inline keyboard — handle inline, no worker needed.
    Callback,
    /// Nexus event — forward notification, no worker needed.
    NexusEvent,
}

/// Classify a trigger for routing.
///
/// Fast classification based on keywords and trigger type — no AI needed.
pub fn classify_trigger(trigger: &Trigger) -> TriggerClass {
    match trigger {
        Trigger::Cron(_) => TriggerClass::Digest,
        Trigger::NexusEvent(_) => TriggerClass::NexusEvent,
        Trigger::CliCommand(_) => TriggerClass::Command,
        Trigger::Message(msg) => {
            // Check for callback
            if msg.content.starts_with("[callback] ") {
                return TriggerClass::Callback;
            }

            let lower = msg.content.to_lowercase();
            let trimmed = lower.trim();

            // Chat acknowledgments (respond inline, no worker)
            if matches!(
                trimmed,
                "thanks" | "thank you" | "thx" | "ty"
                    | "ok" | "okay" | "k"
                    | "got it" | "cool" | "nice"
                    | "lol" | "haha" | "heh"
                    | "np" | "no worries"
                    | "yep" | "yup" | "yes" | "nope" | "no"
                    | "roger" | "copy" | "ack"
                    | "thumbs up" | "noted"
            ) {
                return TriggerClass::Chat;
            }

            // Commands — action verbs
            let command_keywords = [
                "create ", "assign ", "move ", "close ", "transition ",
                "comment on ", "update ", "add ", "set ", "delete ",
                "remove ", "deploy ", "restart ", "run ",
            ];
            for kw in &command_keywords {
                if lower.starts_with(kw) {
                    return TriggerClass::Command;
                }
            }

            // Queries — question words
            let query_keywords = [
                "what", "status", "how many", "who", "which",
                "where", "when", "how", "is there", "are there",
                "show me", "list ", "find ", "search ",
                "tell me", "get ", "check ",
            ];
            for kw in &query_keywords {
                if lower.starts_with(kw) || lower.contains(&format!(" {kw}")) {
                    return TriggerClass::Query;
                }
            }

            // Default to query (safer — gets full Claude treatment)
            TriggerClass::Query
        }
    }
}

/// Map a trigger classification to a worker priority.
fn class_to_priority(class: TriggerClass) -> Priority {
    match class {
        TriggerClass::Command => Priority::High,
        TriggerClass::Query => Priority::Normal,
        TriggerClass::Digest => Priority::Normal,
        TriggerClass::Chat => Priority::Low,
        TriggerClass::Callback => Priority::High,
        TriggerClass::NexusEvent => Priority::Normal,
    }
}

// ── Chat Responses ──────────────────────────────────────────────────

/// Quick inline response for chat acknowledgments. No Claude needed.
fn chat_response(content: &str) -> &'static str {
    let lower = content.to_lowercase();
    let trimmed = lower.trim();

    match trimmed {
        "thanks" | "thank you" | "thx" | "ty" => "Anytime.",
        "ok" | "okay" | "k" | "got it" | "copy" | "roger" | "ack" | "noted" => "Noted.",
        "cool" | "nice" => "Indeed.",
        "lol" | "haha" | "heh" => "ha",
        "yep" | "yup" | "yes" => "Understood.",
        "nope" | "no" => "Noted.",
        "np" | "no worries" => "All good.",
        "thumbs up" => "Right on.",
        _ => "Noted.",
    }
}

// ── Orchestrator ────────────────────────────────────────────────────

/// Non-blocking orchestrator that receives triggers and dispatches workers.
pub struct Orchestrator {
    trigger_rx: mpsc::UnboundedReceiver<Trigger>,
    worker_pool: WorkerPool,
    channels: ChannelRegistry,
    deps: Arc<SharedDeps>,
    /// Telegram client for reactions (separate from channel registry).
    telegram_client: Option<TelegramClient>,
    /// Default Telegram chat ID.
    telegram_chat_id: Option<i64>,
    /// UUID of the pending action currently being edited.
    editing_action_id: Option<Uuid>,
    /// Last time we ran the expiry sweep.
    last_expiry_check: Instant,
}

impl Orchestrator {
    /// Create a new orchestrator.
    pub fn new(
        trigger_rx: mpsc::UnboundedReceiver<Trigger>,
        worker_pool: WorkerPool,
        channels: ChannelRegistry,
        deps: Arc<SharedDeps>,
        telegram_client: Option<TelegramClient>,
        telegram_chat_id: Option<i64>,
    ) -> Self {
        Self {
            trigger_rx,
            worker_pool,
            channels,
            deps,
            telegram_client,
            telegram_chat_id,
            editing_action_id: None,
            last_expiry_check: Instant::now(),
        }
    }

    /// Main orchestrator loop — receives triggers, classifies, dispatches.
    ///
    /// Runs until the trigger channel closes (all senders dropped).
    pub async fn run(mut self) -> Result<()> {
        tracing::info!("orchestrator started, waiting for triggers");

        loop {
            let mut triggers = self.drain_triggers().await;
            if triggers.is_empty() {
                tracing::info!("trigger channel closed, shutting down orchestrator");
                break;
            }

            // Extract CLI response channels before processing
            let cli_response_txs = extract_cli_response_channels(&mut triggers);

            // Run periodic expiry sweep for stale pending actions
            if self.last_expiry_check.elapsed() >= EXPIRY_CHECK_INTERVAL {
                self.last_expiry_check = Instant::now();
                if let Some(tg) = self.channels.get("telegram") {
                    if let Some(tg_channel) =
                        tg.as_any().downcast_ref::<crate::telegram::TelegramChannel>()
                    {
                        if let Err(e) = crate::callbacks::check_expired_actions(
                            &tg_channel.client,
                            tg_channel.chat_id,
                            &self.deps.state,
                        )
                        .await
                        {
                            tracing::warn!(error = %e, "expiry sweep failed");
                        }
                    }
                }
            }

            // Classify the first trigger to decide routing
            let primary_class = triggers
                .first()
                .map(classify_trigger)
                .unwrap_or(TriggerClass::Query);

            tracing::debug!(
                class = ?primary_class,
                count = triggers.len(),
                "classified trigger batch"
            );

            // Handle inline cases (no worker needed)
            match primary_class {
                TriggerClass::Callback => {
                    self.handle_callbacks(&triggers).await;
                    continue;
                }
                TriggerClass::NexusEvent => {
                    self.handle_nexus_events(&triggers).await;
                    continue;
                }
                TriggerClass::Chat => {
                    self.handle_chat(&triggers).await;
                    continue;
                }
                _ => {}
            }

            // For Command/Query/Digest: react and dispatch to worker pool

            // Extract Telegram metadata for reactions
            let (tg_chat_id, tg_msg_id) = triggers
                .first()
                .and_then(|t| match t {
                    Trigger::Message(msg) => {
                        let chat_id = msg.metadata.get("chat_id").and_then(|v| v.as_i64());
                        let msg_id = msg.metadata.get("message_id").and_then(|v| v.as_i64());
                        Some((chat_id, msg_id))
                    }
                    _ => (None, None).into(),
                })
                .unwrap_or((None, None));

            // React with eyes immediately
            if let (Some(tg), Some(chat_id), Some(msg_id)) =
                (&self.telegram_client, tg_chat_id.or(self.telegram_chat_id), tg_msg_id)
            {
                let _ = tg.set_message_reaction(chat_id, msg_id, "\u{1F440}").await; // eyes emoji
            }

            // Dispatch to worker pool
            let task = WorkerTask {
                id: Uuid::new_v4(),
                triggers,
                priority: class_to_priority(primary_class),
                created_at: Instant::now(),
                telegram_chat_id: tg_chat_id.or(self.telegram_chat_id),
                telegram_message_id: tg_msg_id,
                cli_response_txs,
            };

            self.worker_pool.dispatch(task).await;
        }

        Ok(())
    }

    // ── Trigger Draining ────────────────────────────────────────────

    /// Block until at least one trigger arrives, then drain all queued triggers.
    async fn drain_triggers(&mut self) -> Vec<Trigger> {
        let first = self.trigger_rx.recv().await;
        let Some(first) = first else {
            return vec![];
        };

        let mut batch = vec![first];
        while let Ok(trigger) = self.trigger_rx.try_recv() {
            batch.push(trigger);
        }

        tracing::info!(count = batch.len(), "drained trigger batch");
        batch
    }

    // ── Inline Handlers ─────────────────────────────────────────────

    /// Handle callback triggers inline (approve/edit/cancel, nexus errors).
    async fn handle_callbacks(&mut self, triggers: &[Trigger]) {
        for trigger in triggers {
            if let Trigger::Message(msg) = trigger {
                if let Some(data) = msg.content.strip_prefix("[callback] ") {
                    let original_msg_id = msg.metadata.get("original_message_id")
                        .and_then(|v| v.as_i64());
                    let tg_chat_id = msg.metadata.get("chat_id")
                        .and_then(|v| v.as_i64());

                    // Nexus error callbacks
                    if let Some(rest) = data.strip_prefix("nexus_err:") {
                        if let Some(session_id) = rest.strip_prefix("view:") {
                            self.handle_nexus_view_error(session_id).await;
                        } else if let Some(session_id) = rest.strip_prefix("bug:") {
                            self.handle_nexus_create_bug(session_id).await;
                        }
                        continue;
                    }

                    // Jira action callbacks
                    if let Some(uuid_str) = data.strip_prefix("approve:") {
                        if let Some(jira_client) = &self.deps.jira_client {
                            if let Some(tg) = self.channels.get("telegram") {
                                if let Some(tg_channel) = tg.as_any().downcast_ref::<crate::telegram::TelegramChannel>() {
                                    let chat_id = tg_chat_id.unwrap_or(tg_channel.chat_id);
                                    if let Err(e) = crate::callbacks::handle_approve(
                                        uuid_str,
                                        jira_client,
                                        &tg_channel.client,
                                        chat_id,
                                        original_msg_id,
                                        &self.deps.state,
                                    ).await {
                                        tracing::error!(error = %e, "approve callback failed");
                                    }
                                }
                            }
                        }
                    } else if let Some(uuid_str) = data.strip_prefix("edit:") {
                        if let Some(tg) = self.channels.get("telegram") {
                            if let Some(tg_channel) = tg.as_any().downcast_ref::<crate::telegram::TelegramChannel>() {
                                let chat_id = tg_chat_id.unwrap_or(tg_channel.chat_id);
                                match crate::callbacks::handle_edit(
                                    uuid_str,
                                    &tg_channel.client,
                                    chat_id,
                                    &self.deps.state,
                                ).await {
                                    Ok(Some(uuid)) => {
                                        self.editing_action_id = Some(uuid);
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        tracing::error!(error = %e, "edit callback failed");
                                    }
                                }
                            }
                        }
                    } else if let Some(uuid_str) = data.strip_prefix("cancel:") {
                        if let Some(tg) = self.channels.get("telegram") {
                            if let Some(tg_channel) = tg.as_any().downcast_ref::<crate::telegram::TelegramChannel>() {
                                let chat_id = tg_chat_id.unwrap_or(tg_channel.chat_id);
                                if let Err(e) = crate::callbacks::handle_cancel(
                                    uuid_str,
                                    &tg_channel.client,
                                    chat_id,
                                    original_msg_id,
                                    &self.deps.state,
                                ).await {
                                    tracing::error!(error = %e, "cancel callback failed");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle Nexus events inline (send notification, no Claude needed).
    async fn handle_nexus_events(&self, triggers: &[Trigger]) {
        for trigger in triggers {
            if let Trigger::NexusEvent(event) = trigger {
                if let Some(msg) = nexus::notify::format_nexus_notification(event) {
                    if let Some(channel) = self.channels.get("telegram") {
                        if let Err(e) = channel.send_message(msg).await {
                            tracing::error!(error = %e, "failed to send Nexus notification");
                        }
                    }
                }
            }
        }
    }

    /// Handle chat acknowledgments inline (reply instantly, react thumbs up).
    async fn handle_chat(&self, triggers: &[Trigger]) {
        for trigger in triggers {
            if let Trigger::Message(msg) = trigger {
                let reply = chat_response(&msg.content);
                let reply_to = Some(msg.id.clone());

                let tg_chat_id = msg.metadata.get("chat_id").and_then(|v| v.as_i64());
                let tg_msg_id = msg.metadata.get("message_id").and_then(|v| v.as_i64());

                // Send reply
                if let Some(channel) = self.channels.get(&msg.channel) {
                    let _ = channel
                        .send_message(OutboundMessage {
                            channel: msg.channel.clone(),
                            content: reply.to_string(),
                            reply_to,
                            keyboard: None,
                        })
                        .await;
                }

                // React with thumbs up
                if let (Some(tg), Some(chat_id), Some(msg_id)) =
                    (&self.telegram_client, tg_chat_id, tg_msg_id)
                {
                    let _ = tg.set_message_reaction(chat_id, msg_id, "\u{1F44D}").await; // thumbs up
                }
            }
        }
    }

    // ── Nexus Error Handlers ────────────────────────────────────────

    async fn handle_nexus_view_error(&self, session_id: &str) {
        let Some(nexus_client) = &self.deps.nexus_client else {
            tracing::warn!("nexus_err:view callback but no Nexus client configured");
            return;
        };

        match nexus_client.query_session(session_id).await {
            Ok(Some(session)) => {
                let detail_text = nexus::notify::format_session_error_detail(&session);
                if let Some(channel) = self.channels.get("telegram") {
                    let msg = OutboundMessage {
                        channel: "telegram".into(),
                        content: detail_text,
                        reply_to: None,
                        keyboard: None,
                    };
                    if let Err(e) = channel.send_message(msg).await {
                        tracing::error!(error = %e, "failed to send error detail to Telegram");
                    }
                }
            }
            Ok(None) => {
                tracing::warn!(session_id, "nexus_err:view — session not found");
                self.send_error("telegram", &format!(
                    "Session {session_id} not found on any connected Nexus agent."
                )).await;
            }
            Err(e) => {
                tracing::error!(error = %e, session_id, "nexus_err:view — query failed");
                self.send_error("telegram", &format!(
                    "Failed to query session {session_id}: {e}"
                )).await;
            }
        }
    }

    async fn handle_nexus_create_bug(&self, session_id: &str) {
        let Some(nexus_client) = &self.deps.nexus_client else {
            tracing::warn!("nexus_err:bug callback but no Nexus client configured");
            return;
        };

        match nexus_client.query_session(session_id).await {
            Ok(Some(session)) => {
                let action = nexus::notify::create_bug_from_session_error(&session);

                if let Err(e) = self.deps.state.save_pending_action(
                    &crate::state::PendingAction {
                        id: action.id,
                        description: action.description.clone(),
                        payload: action.payload.clone(),
                        status: crate::state::PendingStatus::AwaitingConfirmation,
                        created_at: action.created_at,
                        telegram_message_id: None,
                        telegram_chat_id: None,
                    },
                ) {
                    tracing::error!(error = %e, "failed to save bug pending action");
                    return;
                }

                let keyboard = InlineKeyboard::confirm_action(&action.id.to_string());
                if let Some(channel) = self.channels.get("telegram") {
                    let msg = OutboundMessage {
                        channel: "telegram".into(),
                        content: format!(
                            "Create Jira bug from session error?\n\n{}\n\nApprove, edit, or cancel?",
                            action.description
                        ),
                        reply_to: None,
                        keyboard: Some(keyboard),
                    };
                    if let Err(e) = channel.send_message(msg).await {
                        tracing::error!(error = %e, "failed to send bug confirmation keyboard");
                    }
                }
            }
            Ok(None) => {
                tracing::warn!(session_id, "nexus_err:bug — session not found");
                self.send_error("telegram", &format!(
                    "Session {session_id} not found on any connected Nexus agent."
                )).await;
            }
            Err(e) => {
                tracing::error!(error = %e, session_id, "nexus_err:bug — query failed");
                self.send_error("telegram", &format!(
                    "Failed to query session {session_id}: {e}"
                )).await;
            }
        }
    }

    /// Send an error message to a channel.
    async fn send_error(&self, channel_name: &str, message: &str) {
        if let Some(channel) = self.channels.get(channel_name) {
            let msg = OutboundMessage {
                channel: channel_name.into(),
                content: format!("\u{26A0} {message}"),
                reply_to: None,
                keyboard: None,
            };
            if let Err(e) = channel.send_message(msg).await {
                tracing::error!(error = %e, "failed to send error to {}", channel_name);
            }
        }
    }
}

// ── CLI Response Channel Extraction ─────────────────────────────────

/// Extract oneshot response senders from CLI triggers.
fn extract_cli_response_channels(
    triggers: &mut [Trigger],
) -> Vec<tokio::sync::oneshot::Sender<String>> {
    let mut channels = Vec::new();
    for trigger in triggers.iter_mut() {
        if let Trigger::CliCommand(req) = trigger {
            if let Some(tx) = req.response_tx.take() {
                channels.push(tx);
            }
        }
    }
    channels
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::types::{CronEvent, InboundMessage, SessionEvent, SessionEventType};

    fn make_message(content: &str) -> Trigger {
        Trigger::Message(InboundMessage {
            id: "1".into(),
            channel: "telegram".into(),
            sender: "leo".into(),
            content: content.into(),
            timestamp: chrono::Utc::now(),
            thread_id: None,
            metadata: serde_json::json!({}),
        })
    }

    #[test]
    fn classify_command_triggers() {
        assert_eq!(classify_trigger(&make_message("create a bug for OO-123")), TriggerClass::Command);
        assert_eq!(classify_trigger(&make_message("assign OO-50 to Leo")), TriggerClass::Command);
        assert_eq!(classify_trigger(&make_message("move OO-99 to Done")), TriggerClass::Command);
        assert_eq!(classify_trigger(&make_message("close the sprint")), TriggerClass::Command);
    }

    #[test]
    fn classify_query_triggers() {
        assert_eq!(classify_trigger(&make_message("what's blocking the release?")), TriggerClass::Query);
        assert_eq!(classify_trigger(&make_message("status of OO-42")), TriggerClass::Query);
        assert_eq!(classify_trigger(&make_message("how many open bugs?")), TriggerClass::Query);
        assert_eq!(classify_trigger(&make_message("who is assigned to OO-10?")), TriggerClass::Query);
    }

    #[test]
    fn classify_chat_triggers() {
        assert_eq!(classify_trigger(&make_message("thanks")), TriggerClass::Chat);
        assert_eq!(classify_trigger(&make_message("ok")), TriggerClass::Chat);
        assert_eq!(classify_trigger(&make_message("got it")), TriggerClass::Chat);
        assert_eq!(classify_trigger(&make_message("cool")), TriggerClass::Chat);
        assert_eq!(classify_trigger(&make_message("lol")), TriggerClass::Chat);
        assert_eq!(classify_trigger(&make_message("noted")), TriggerClass::Chat);
    }

    #[test]
    fn classify_digest_triggers() {
        assert_eq!(
            classify_trigger(&Trigger::Cron(CronEvent::Digest)),
            TriggerClass::Digest
        );
    }

    #[test]
    fn classify_nexus_event_triggers() {
        assert_eq!(
            classify_trigger(&Trigger::NexusEvent(SessionEvent {
                agent_name: "builder".into(),
                session_id: "s-1".into(),
                event_type: SessionEventType::Completed,
                details: None,
            })),
            TriggerClass::NexusEvent
        );
    }

    #[test]
    fn classify_callback_triggers() {
        assert_eq!(
            classify_trigger(&make_message("[callback] approve:abc-123")),
            TriggerClass::Callback
        );
        assert_eq!(
            classify_trigger(&make_message("[callback] nexus_err:view:s-1")),
            TriggerClass::Callback
        );
    }

    #[test]
    fn chat_responses_are_short() {
        assert!(chat_response("thanks").len() <= 20);
        assert!(chat_response("ok").len() <= 20);
        assert!(chat_response("lol").len() <= 20);
    }

    #[test]
    fn priority_mapping() {
        assert_eq!(class_to_priority(TriggerClass::Command), Priority::High);
        assert_eq!(class_to_priority(TriggerClass::Query), Priority::Normal);
        assert_eq!(class_to_priority(TriggerClass::Digest), Priority::Normal);
        assert_eq!(class_to_priority(TriggerClass::Chat), Priority::Low);
    }
}
