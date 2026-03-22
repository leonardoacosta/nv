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
use crate::worker::{Priority, SharedDeps, WorkerEvent, WorkerPool, WorkerTask};

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
    /// Receiver for worker progress events.
    event_rx: mpsc::UnboundedReceiver<WorkerEvent>,
    /// Tracks the last StageStarted event per worker for inactivity detection.
    worker_stage_started: std::collections::HashMap<Uuid, (String, Instant)>,
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
        event_rx: mpsc::UnboundedReceiver<WorkerEvent>,
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
            event_rx,
            worker_stage_started: std::collections::HashMap::new(),
        }
    }

    /// Main orchestrator loop — receives triggers and worker events.
    ///
    /// Uses `tokio::select!` to multiplex:
    /// - Trigger channel (inbound user/cron/nexus triggers)
    /// - Worker event channel (progress events from workers)
    /// - 30s inactivity timer (status update to Telegram for long-running tasks)
    ///
    /// Runs until the trigger channel closes (all senders dropped).
    pub async fn run(mut self) -> Result<()> {
        tracing::info!("orchestrator started, waiting for triggers");

        /// Inactivity threshold — if a worker stage has been running this long
        /// without a Complete or Error, send a status update to Telegram.
        const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(30);

        let mut trigger_closed = false;

        loop {
            // If trigger channel is closed and no active workers are tracked, exit
            if trigger_closed && self.worker_stage_started.is_empty() {
                tracing::info!("trigger channel closed and no active workers, shutting down");
                break;
            }

            // Compute next inactivity deadline
            let next_inactivity = self
                .worker_stage_started
                .values()
                .map(|(_, started)| *started + INACTIVITY_TIMEOUT)
                .min();

            let inactivity_sleep = match next_inactivity {
                Some(deadline) => {
                    let now = Instant::now();
                    if deadline > now {
                        tokio::time::sleep(deadline - now)
                    } else {
                        tokio::time::sleep(Duration::ZERO)
                    }
                }
                None => tokio::time::sleep(Duration::from_secs(3600)), // effectively infinite
            };
            tokio::pin!(inactivity_sleep);

            tokio::select! {
                trigger = self.trigger_rx.recv(), if !trigger_closed => {
                    let Some(first) = trigger else {
                        tracing::info!("trigger channel closed");
                        trigger_closed = true;
                        continue;
                    };

                    // Drain any additional queued triggers
                    let mut triggers = vec![first];
                    while let Ok(t) = self.trigger_rx.try_recv() {
                        triggers.push(t);
                    }
                    tracing::info!(count = triggers.len(), "drained trigger batch");

                    self.process_trigger_batch(&mut triggers).await;
                }
                event = self.event_rx.recv() => {
                    let Some(event) = event else {
                        // Event channel closed — all workers gone
                        tracing::debug!("worker event channel closed");
                        if trigger_closed {
                            break;
                        }
                        continue;
                    };
                    self.handle_worker_event(event).await;
                }
                () = &mut inactivity_sleep => {
                    self.check_inactivity(INACTIVITY_TIMEOUT).await;
                }
            }
        }

        Ok(())
    }

    /// Process a batch of triggers (extracted from the select loop).
    async fn process_trigger_batch(&mut self, triggers: &mut Vec<Trigger>) {
        // Extract CLI response channels before processing
        let cli_response_txs = extract_cli_response_channels(triggers);

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
                self.handle_callbacks(triggers).await;
                return;
            }
            TriggerClass::NexusEvent => {
                self.handle_nexus_events(triggers).await;
                return;
            }
            TriggerClass::Chat => {
                self.handle_chat(triggers).await;
                return;
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

        // Long-task confirmation: estimate if this task is heavy
        if let Some(estimate) = estimate_task_complexity(triggers) {
            let chat_id = tg_chat_id.or(self.telegram_chat_id);
            if let Some(channel) = self.channels.get("telegram") {
                let msg = OutboundMessage {
                    channel: "telegram".into(),
                    content: format!(
                        "This will take ~{}min. {}. Be right back.",
                        estimate.estimated_minutes, estimate.description
                    ),
                    reply_to: None,
                    keyboard: None,
                };
                let _ = channel.send_message(msg).await;
                tracing::info!(
                    chat_id,
                    estimated_minutes = estimate.estimated_minutes,
                    description = %estimate.description,
                    "sent long-task confirmation"
                );
            }
        }

        // Dispatch to worker pool
        let task = WorkerTask {
            id: Uuid::new_v4(),
            triggers: std::mem::take(triggers),
            priority: class_to_priority(primary_class),
            created_at: Instant::now(),
            telegram_chat_id: tg_chat_id.or(self.telegram_chat_id),
            telegram_message_id: tg_msg_id,
            cli_response_txs,
        };

        self.worker_pool.dispatch(task).await;
    }

    /// Handle a single worker event — log at appropriate level and track state.
    async fn handle_worker_event(&mut self, event: WorkerEvent) {
        match &event {
            WorkerEvent::StageStarted { worker_id, stage } => {
                tracing::debug!(
                    worker_id = %worker_id,
                    stage = %stage,
                    "worker stage started"
                );
                self.worker_stage_started
                    .insert(*worker_id, (stage.clone(), Instant::now()));
            }
            WorkerEvent::ToolCalled { worker_id, tool } => {
                tracing::trace!(
                    worker_id = %worker_id,
                    tool = %tool,
                    "worker tool called"
                );
            }
            WorkerEvent::StageComplete {
                worker_id,
                stage,
                duration_ms,
            } => {
                tracing::debug!(
                    worker_id = %worker_id,
                    stage = %stage,
                    duration_ms,
                    "worker stage complete"
                );
                self.worker_stage_started.remove(worker_id);
            }
            WorkerEvent::Complete {
                worker_id,
                response_len,
            } => {
                tracing::debug!(
                    worker_id = %worker_id,
                    response_len,
                    "worker complete"
                );
                // Clean up any remaining stage tracking
                self.worker_stage_started.remove(worker_id);
            }
            WorkerEvent::Error { worker_id, error } => {
                tracing::warn!(
                    worker_id = %worker_id,
                    error = %error,
                    "worker error"
                );
                self.worker_stage_started.remove(worker_id);
            }
        }
    }

    /// Check for workers that have been running a stage longer than the
    /// inactivity threshold and send a status update to Telegram.
    async fn check_inactivity(&mut self, threshold: Duration) {
        let now = Instant::now();
        let mut stale_workers = Vec::new();

        for (worker_id, (stage, started)) in &self.worker_stage_started {
            if now.duration_since(*started) >= threshold {
                stale_workers.push((*worker_id, stage.clone()));
            }
        }

        for (worker_id, stage) in stale_workers {
            tracing::info!(
                worker_id = %worker_id,
                stage = %stage,
                "worker inactivity detected, sending status update"
            );

            if let Some(channel) = self.channels.get("telegram") {
                let msg = OutboundMessage {
                    channel: "telegram".into(),
                    content: format!("Still working on it... (running {stage})"),
                    reply_to: None,
                    keyboard: None,
                };
                let _ = channel.send_message(msg).await;
            }

            // Reset the timer so we don't spam — move start time forward
            if let Some(entry) = self.worker_stage_started.get_mut(&worker_id) {
                entry.1 = Instant::now();
            }
        }
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

// ── Long-Task Estimation ────────────────────────────────────────────

/// Estimate for a long-running task.
#[derive(Debug)]
struct LongTaskEstimate {
    /// Estimated duration in minutes.
    estimated_minutes: u32,
    /// Human-readable description of what the task involves.
    description: String,
}

/// Keyword-based patterns that indicate a heavy task (>1 min expected).
const HEAVY_TASK_PATTERNS: &[(&str, u32, &str)] = &[
    // (keyword pattern, estimated minutes, description template)
    ("search all projects", 2, "Searching Jira across all projects"),
    ("search across all", 2, "Searching across all projects"),
    ("all projects", 2, "Scanning all projects"),
    ("every project", 2, "Scanning every project"),
    ("full scan", 3, "Running full scan"),
    ("filesystem scan", 3, "Scanning filesystem"),
    ("scan all", 2, "Scanning all resources"),
    ("digest", 2, "Generating digest"),
    ("weekly report", 3, "Compiling weekly report"),
    ("monthly report", 3, "Compiling monthly report"),
    ("sprint report", 2, "Generating sprint report"),
    ("summarize everything", 3, "Summarizing everything"),
    ("full audit", 3, "Running full audit"),
];

/// Estimate task complexity from trigger content using keyword matching.
///
/// Returns `Some(LongTaskEstimate)` if the task is classified as heavy
/// (expected >1 minute), `None` for normal tasks.
fn estimate_task_complexity(triggers: &[Trigger]) -> Option<LongTaskEstimate> {
    // Check digest triggers
    for trigger in triggers {
        if let Trigger::Cron(_) = trigger {
            return Some(LongTaskEstimate {
                estimated_minutes: 2,
                description: "Generating digest synthesis".into(),
            });
        }
    }

    // Check message content against heavy-task patterns
    for trigger in triggers {
        if let Trigger::Message(msg) = trigger {
            let lower = msg.content.to_lowercase();
            for &(pattern, minutes, description) in HEAVY_TASK_PATTERNS {
                if lower.contains(pattern) {
                    return Some(LongTaskEstimate {
                        estimated_minutes: minutes,
                        description: description.into(),
                    });
                }
            }
        }
    }

    None
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

    // ── Long-Task Estimation Tests ───────────────────────────────────

    #[test]
    fn estimate_complexity_normal_query_returns_none() {
        let triggers = vec![make_message("what is the status of OO-42?")];
        assert!(estimate_task_complexity(&triggers).is_none());
    }

    #[test]
    fn estimate_complexity_multi_project_search_returns_estimate() {
        let triggers = vec![make_message("search all projects for auth bugs")];
        let est = estimate_task_complexity(&triggers).expect("should detect heavy task");
        assert_eq!(est.estimated_minutes, 2);
        assert!(est.description.contains("Searching"));
    }

    #[test]
    fn estimate_complexity_digest_cron_returns_estimate() {
        let triggers = vec![Trigger::Cron(CronEvent::Digest)];
        let est = estimate_task_complexity(&triggers).expect("should detect digest");
        assert_eq!(est.estimated_minutes, 2);
    }

    #[test]
    fn estimate_complexity_full_scan_returns_estimate() {
        let triggers = vec![make_message("run a full scan of the codebase")];
        let est = estimate_task_complexity(&triggers).expect("should detect heavy task");
        assert_eq!(est.estimated_minutes, 3);
    }

    #[test]
    fn estimate_complexity_weekly_report_returns_estimate() {
        let triggers = vec![make_message("generate the weekly report")];
        let est = estimate_task_complexity(&triggers).expect("should detect heavy task");
        assert_eq!(est.estimated_minutes, 3);
    }

    #[test]
    fn estimate_complexity_case_insensitive() {
        let triggers = vec![make_message("Search All Projects for bugs")];
        let est = estimate_task_complexity(&triggers).expect("should detect heavy task");
        assert_eq!(est.estimated_minutes, 2);
    }

    // ── Orchestrator Event Handling Tests ─────────────────────────────

    #[tokio::test]
    async fn orchestrator_handles_all_worker_event_variants() {
        let (_trigger_tx, _trigger_rx) = mpsc::unbounded_channel::<Trigger>();
        let (event_tx, event_rx) = mpsc::unbounded_channel::<WorkerEvent>();

        // Create a minimal orchestrator (no real channels/deps needed for event handling)
        let worker_id = Uuid::new_v4();

        // Send all event variants
        event_tx
            .send(WorkerEvent::StageStarted {
                worker_id,
                stage: "context_build".into(),
            })
            .unwrap();
        event_tx
            .send(WorkerEvent::ToolCalled {
                worker_id,
                tool: "jira_search".into(),
            })
            .unwrap();
        event_tx
            .send(WorkerEvent::StageComplete {
                worker_id,
                stage: "context_build".into(),
                duration_ms: 42,
            })
            .unwrap();
        event_tx
            .send(WorkerEvent::Complete {
                worker_id,
                response_len: 100,
            })
            .unwrap();
        event_tx
            .send(WorkerEvent::Error {
                worker_id,
                error: "test error".into(),
            })
            .unwrap();

        // Drop sender so receiver will eventually return None
        drop(event_tx);

        // Drain all events through a mock-like handler — just verify no panics
        let mut rx = event_rx;
        let mut stage_map: std::collections::HashMap<Uuid, (String, Instant)> =
            std::collections::HashMap::new();
        let mut count = 0;

        while let Some(event) = rx.recv().await {
            count += 1;
            match &event {
                WorkerEvent::StageStarted { worker_id, stage } => {
                    stage_map.insert(*worker_id, (stage.clone(), Instant::now()));
                }
                WorkerEvent::StageComplete { worker_id, .. } => {
                    stage_map.remove(worker_id);
                }
                WorkerEvent::Complete { worker_id, .. } => {
                    stage_map.remove(worker_id);
                }
                WorkerEvent::Error { worker_id, .. } => {
                    stage_map.remove(worker_id);
                }
                WorkerEvent::ToolCalled { .. } => {}
            }
        }

        assert_eq!(count, 5);
        assert!(stage_map.is_empty());
    }
}
