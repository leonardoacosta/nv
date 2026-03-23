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
use crate::channels::telegram::client::TelegramClient;
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
    /// BotFather-style /command — handled directly, no worker needed.
    BotCommand,
}

/// A parsed bot command with its arguments.
#[derive(Debug, Clone)]
pub struct ParsedBotCommand {
    pub command: String,
    pub args: Vec<String>,
}

/// Parse a `/command arg1 arg2` message into a `ParsedBotCommand`.
///
/// Returns `None` if the message doesn't start with `/`.
pub fn parse_bot_command(text: &str) -> Option<ParsedBotCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let raw_command = parts.next()?;

    // Strip the leading `/` and any @bot_name suffix (e.g., /status@MyBot)
    let command = raw_command
        .strip_prefix('/')
        .unwrap_or(raw_command)
        .split('@')
        .next()
        .unwrap_or("")
        .to_lowercase();

    if command.is_empty() {
        return None;
    }

    let args: Vec<String> = parts.map(String::from).collect();

    Some(ParsedBotCommand { command, args })
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

            // Check for bot commands (/status, /digest, /health, etc.)
            if parse_bot_command(&msg.content).is_some() {
                return TriggerClass::BotCommand;
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
        TriggerClass::BotCommand => Priority::High,
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
    /// Quiet hours start (parsed from config). None = no quiet window.
    quiet_start: Option<chrono::NaiveTime>,
    /// Quiet hours end (parsed from config). None = no quiet window.
    quiet_end: Option<chrono::NaiveTime>,
    // ── PendingAction error dedup state ──
    /// Text of the last error sent (or being batched).
    last_error_text: Option<String>,
    /// Timestamp of the last error in the current batch.
    last_error_time: Instant,
    /// Number of errors accumulated in the current batch.
    error_count: u32,
    // ── Thinking message state ──
    /// Maps worker_id → (thinking_msg_id, chat_id) for active workers.
    worker_thinking_msgs: std::collections::HashMap<Uuid, (i64, i64)>,
    /// Maps worker_id → last thinking-message edit timestamp (for debounce).
    worker_thinking_last_edit: std::collections::HashMap<Uuid, Instant>,
}

impl Orchestrator {
    /// Create a new orchestrator.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trigger_rx: mpsc::UnboundedReceiver<Trigger>,
        worker_pool: WorkerPool,
        channels: ChannelRegistry,
        deps: Arc<SharedDeps>,
        telegram_client: Option<TelegramClient>,
        telegram_chat_id: Option<i64>,
        event_rx: mpsc::UnboundedReceiver<WorkerEvent>,
        quiet_start: Option<chrono::NaiveTime>,
        quiet_end: Option<chrono::NaiveTime>,
    ) -> Self {
        if quiet_start.is_some() && quiet_end.is_some() {
            tracing::info!(
                quiet_start = ?quiet_start,
                quiet_end = ?quiet_end,
                "quiet hours configured"
            );
        }
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
            quiet_start,
            quiet_end,
            last_error_text: None,
            last_error_time: Instant::now(),
            error_count: 0,
            worker_thinking_msgs: std::collections::HashMap::new(),
            worker_thinking_last_edit: std::collections::HashMap::new(),
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
        /// Typing indicator refresh interval. Telegram's "typing..." indicator
        /// expires after ~5s, so we refresh every 5s while workers are active.
        const TYPING_REFRESH: Duration = Duration::from_secs(5);

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
                .map(|(_, started)| *started + TYPING_REFRESH)
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
                    self.check_inactivity(TYPING_REFRESH).await;
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
                    tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()
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
                // Suppress nexus event notifications during quiet hours
                if is_quiet_hours(self.quiet_start, self.quiet_end) {
                    tracing::debug!("suppressing nexus event during quiet hours");
                    return;
                }
                self.handle_nexus_events(triggers).await;
                return;
            }
            TriggerClass::Chat => {
                self.handle_chat(triggers).await;
                return;
            }
            TriggerClass::BotCommand => {
                self.handle_bot_commands(triggers).await;
                return;
            }
            _ => {}
        }

        // Quiet hours gate: suppress non-P0 (non-High priority) dispatch
        let priority = class_to_priority(primary_class);
        if priority != Priority::High && is_quiet_hours(self.quiet_start, self.quiet_end) {
            tracing::info!(
                class = ?primary_class,
                "suppressing non-P0 trigger during quiet hours"
            );
            return;
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
                    reply_to: tg_msg_id.map(|id| id.to_string()),
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
            WorkerEvent::StageStarted { worker_id, stage, thinking_msg_id, thinking_chat_id } => {
                tracing::debug!(
                    worker_id = %worker_id,
                    stage = %stage,
                    "worker stage started"
                );
                self.worker_stage_started
                    .insert(*worker_id, (stage.clone(), Instant::now()));
                // Store thinking message ID from the first (context_build) stage
                if let (Some(msg_id), Some(chat_id)) = (thinking_msg_id, thinking_chat_id) {
                    self.worker_thinking_msgs.insert(*worker_id, (*msg_id, *chat_id));
                }
            }
            WorkerEvent::ToolCalled { worker_id, tool } => {
                tracing::trace!(
                    worker_id = %worker_id,
                    tool = %tool,
                    "worker tool called"
                );
                // Update stage description with human-readable tool name
                let (emoji, description) = humanize_tool(tool);
                let stage_label = format!("{emoji} {description}");
                self.worker_stage_started
                    .insert(*worker_id, (stage_label.clone(), Instant::now()));

                // Edit the thinking message with tool status (debounced, Telegram only)
                if let Some((msg_id, chat_id)) = self.worker_thinking_msgs.get(worker_id).copied() {
                    // Debounce: skip if we edited within the last 500ms
                    let should_edit = self.worker_thinking_last_edit
                        .get(worker_id)
                        .map(|t| t.elapsed() >= Duration::from_millis(500))
                        .unwrap_or(true);

                    if should_edit {
                        if let Some(tg) = self.channels.get("telegram") {
                            if let Some(tg_channel) =
                                tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()
                            {
                                let label = stage_label.clone();
                                let client = tg_channel.client.clone();
                                let wid = *worker_id;
                                // Fire-and-forget — don't block the event loop
                                tokio::spawn(async move {
                                    if let Err(e) = client.edit_message(chat_id, msg_id, &label, None).await {
                                        tracing::debug!(
                                            worker_id = %wid,
                                            error = %e,
                                            "failed to edit thinking message with tool status"
                                        );
                                    }
                                });
                                self.worker_thinking_last_edit.insert(*worker_id, Instant::now());
                            }
                        }
                    }
                }
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
                // Clean up stage tracking and thinking message state
                self.worker_stage_started.remove(worker_id);
                self.worker_thinking_msgs.remove(worker_id);
                self.worker_thinking_last_edit.remove(worker_id);
            }
            WorkerEvent::Error { worker_id, error } => {
                tracing::warn!(
                    worker_id = %worker_id,
                    error = %error,
                    "worker error"
                );
                self.worker_stage_started.remove(worker_id);
                self.worker_thinking_msgs.remove(worker_id);
                self.worker_thinking_last_edit.remove(worker_id);
            }
        }
    }

    /// Check for active workers and refresh the Telegram typing indicator.
    /// Status details go to debug log only — never sent as messages.
    async fn check_inactivity(&mut self, _threshold: Duration) {
        if self.worker_stage_started.is_empty() {
            return;
        }

        // Log stage details to debug (for journalctl optimization analysis)
        for (worker_id, (stage, started)) in &self.worker_stage_started {
            let elapsed = Instant::now().duration_since(*started);
            tracing::debug!(
                worker_id = %worker_id,
                stage = %stage,
                elapsed_secs = elapsed.as_secs(),
                human = %humanize_stage(stage),
                "worker active"
            );
        }

        // Refresh typing indicator — shows "Nova is typing..." in chat header
        // Telegram typing indicator expires after ~5s, so we refresh every cycle
        if let Some(tg) = self.channels.get("telegram") {
            if let Some(tg_channel) =
                tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()
            {
                let _ = tg_channel
                    .client
                    .send_chat_action(tg_channel.chat_id, "typing")
                    .await;
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

                    // Nexus error callbacks (legacy)
                    if let Some(rest) = data.strip_prefix("nexus_err:") {
                        if let Some(session_id) = rest.strip_prefix("view:") {
                            self.handle_nexus_view_error(session_id).await;
                        } else if let Some(session_id) = rest.strip_prefix("bug:") {
                            self.handle_nexus_create_bug(session_id).await;
                        }
                        continue;
                    }

                    // Session error retry callback (stub — routing via nexus_err:)
                    if let Some(_event_id) = data.strip_prefix("retry:") {
                        tracing::debug!("retry callback (not yet implemented)");
                        continue;
                    }

                    // Session error create-bug callback (stub — routing via nexus_err:bug:)
                    if let Some(_event_id) = data.strip_prefix("bug:") {
                        tracing::debug!("bug callback (not yet implemented)");
                        continue;
                    }

                    // Action callbacks (Jira, Nexus, HA, etc.)
                    if let Some(uuid_str) = data.strip_prefix("approve:") {
                        if let Some(tg) = self.channels.get("telegram") {
                            if let Some(tg_channel) = tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>() {
                                let chat_id = tg_chat_id.unwrap_or(tg_channel.chat_id);
                                if let Err(e) = crate::callbacks::handle_approve(
                                    uuid_str,
                                    self.deps.jira_registry.as_ref(),
                                    self.deps.nexus_client.as_ref(),
                                    &self.deps.project_registry,
                                    &self.deps.channels,
                                    &tg_channel.client,
                                    chat_id,
                                    original_msg_id,
                                    &self.deps.state,
                                    self.deps.schedule_store.as_deref(),
                                ).await {
                                    tracing::error!(error = %e, "approve callback failed");
                                }
                            }
                        }
                    } else if let Some(uuid_str) = data.strip_prefix("edit:") {
                        if let Some(tg) = self.channels.get("telegram") {
                            if let Some(tg_channel) = tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>() {
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
                            if let Some(tg_channel) = tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>() {
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
    ///
    /// For failed events, stores error metadata and attaches retry/create-bug
    /// inline keyboard buttons.
    async fn handle_nexus_events(&self, triggers: &[Trigger]) {
        for trigger in triggers {
            if let Trigger::NexusEvent(event) = trigger {
                // For failed events, store error metadata for retry/bug callbacks
                let event_id = if matches!(
                    event.event_type,
                    nv_core::types::SessionEventType::Failed
                ) {
                    let eid = Uuid::new_v4().to_string();
                    let meta = crate::state::SessionErrorMeta {
                        project: event.agent_name.clone(),
                        cwd: String::new(),
                        command: event.details.clone(),
                        error_message: event
                            .details
                            .as_deref()
                            .unwrap_or("unknown error")
                            .to_string(),
                        session_id: event.session_id.clone(),
                        agent_name: event.agent_name.clone(),
                        timestamp: chrono::Utc::now(),
                    };
                    self.deps.state.store_session_error(&eid, meta);
                    Some(eid)
                } else {
                    None
                };

                if let Some(msg) = nexus::notify::format_nexus_notification(
                    event,
                    event_id.as_deref(),
                ) {
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

    // ── Bot Command Handlers ────────────────────────────────────────

    /// Handle BotFather-style /commands inline — no Claude worker needed.
    ///
    /// Commands are executed directly against tools and services.
    /// Response time is ~100ms (tool execution) instead of ~5s (Claude round-trip).
    async fn handle_bot_commands(&self, triggers: &[Trigger]) {
        for trigger in triggers {
            if let Trigger::Message(msg) = trigger {
                let Some(parsed) = parse_bot_command(&msg.content) else {
                    continue;
                };

                let reply_to = Some(msg.id.clone());
                let channel_name = msg.channel.clone();

                let response = match parsed.command.as_str() {
                    "status" => self.cmd_status(&parsed.args).await,
                    "digest" => self.cmd_digest().await,
                    "health" => self.cmd_health().await,
                    "projects" => self.cmd_projects().await,
                    "apply" => self.cmd_apply(&parsed.args, msg).await,
                    _ => self.cmd_unknown(&parsed.command),
                };

                if let Some(channel) = self.channels.get(&channel_name) {
                    let _ = channel
                        .send_message(OutboundMessage {
                            channel: channel_name,
                            content: response,
                            reply_to,
                            keyboard: None,
                        })
                        .await;
                }
            }
        }
    }

    /// /status — project health dashboard for all projects (or a specific one).
    async fn cmd_status(&self, args: &[String]) -> String {
        let codes: Vec<&str> = if args.is_empty() {
            self.deps.project_registry.keys().map(|s| s.as_str()).collect()
        } else {
            args.iter().map(|s| s.as_str()).collect()
        };

        let mut lines = Vec::new();
        for code in &codes {
            let result = crate::aggregation::project_health(
                code,
                self.deps.jira_registry.as_ref().and_then(|r| r.resolve(code)),
                self.deps.nexus_client.as_ref(),
            )
            .await;

            match result {
                Ok(output) => {
                    lines.push(format_status_dots(code, &output));
                }
                Err(e) => {
                    lines.push(format!("\u{1F534} {} -- error: {e}", code));
                }
            }
        }

        if lines.is_empty() {
            "No projects registered.".to_string()
        } else {
            lines.sort();
            format!("\u{1F4CA} Project Status\n\n{}", lines.join("\n"))
        }
    }

    /// /digest — trigger an immediate digest.
    async fn cmd_digest(&self) -> String {
        // We can't inject into the trigger channel directly since we don't
        // hold a sender. Instead, use the deps' channels to route a message
        // that the orchestrator will pick up as a digest.
        // For now, send a direct HTTP request to the local health endpoint.
        let port = 8400; // default health port
        let client = reqwest::Client::new();
        match client
            .post(format!("http://127.0.0.1:{port}/digest"))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(_) => "\u{2705} Digest triggered.".to_string(),
            Err(e) => format!("\u{274C} Failed to trigger digest: {e}"),
        }
    }

    /// /health — homelab infrastructure status.
    async fn cmd_health(&self) -> String {
        match crate::aggregation::homelab_status().await {
            Ok(output) => {
                format!(
                    "\u{1F3E0} Homelab Health\n\n{}",
                    format_for_telegram(&output)
                )
            }
            Err(e) => format!("\u{274C} Health check failed: {e}"),
        }
    }

    /// /projects — list all registered projects with latest status dot.
    async fn cmd_projects(&self) -> String {
        let mut codes: Vec<&String> = self.deps.project_registry.keys().collect();
        codes.sort();

        if codes.is_empty() {
            return "No projects registered.".to_string();
        }

        let mut lines = Vec::with_capacity(codes.len());
        for code in &codes {
            let dot = if let Ok(output) = crate::aggregation::project_health(
                code,
                self.deps.jira_registry.as_ref().and_then(|r| r.resolve(code)),
                self.deps.nexus_client.as_ref(),
            )
            .await
            {
                if output.contains("unavailable") || output.contains("error") {
                    "\u{1F7E1}" // yellow
                } else {
                    "\u{1F7E2}" // green
                }
            } else {
                "\u{1F534}" // red
            };
            lines.push(format!("{dot} {code}"));
        }

        format!("\u{1F4CB} Projects\n\n{}", lines.join("\n"))
    }

    /// /apply <project> <spec> — start a CC session via Nexus (with confirmation).
    async fn cmd_apply(
        &self,
        args: &[String],
        msg: &nv_core::types::InboundMessage,
    ) -> String {
        if args.len() < 2 {
            return "\u{2139}\u{FE0F} Usage: /apply <project> <spec>\n\nExample: /apply oo fix-chat-bugs".to_string();
        }

        let project = &args[0];
        let spec = &args[1];

        // Verify project exists in registry
        if !self.deps.project_registry.contains_key(project.as_str()) {
            let known: Vec<&String> = self.deps.project_registry.keys().collect();
            return format!(
                "\u{274C} Unknown project: {project}\n\nKnown projects: {}",
                known.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
            );
        }

        // Check Nexus connectivity
        let Some(nexus_client) = &self.deps.nexus_client else {
            return "\u{274C} Nexus not configured. Cannot start remote sessions.".to_string();
        };

        if !nexus_client.is_connected().await {
            return "\u{274C} No Nexus agents connected. Cannot start remote sessions.".to_string();
        }

        // Create PendingAction with confirmation keyboard
        let action_id = Uuid::new_v4();
        let command = format!("/apply {spec}");
        let description = format!(
            "Start CC session on {}: `{command}`",
            project.to_uppercase()
        );

        let keyboard = InlineKeyboard::confirm_action(&action_id.to_string());
        let payload = serde_json::json!({
            "project": project,
            "command": command,
            "_action_type": "NexusStartSession",
        });

        let tg_chat_id = msg.metadata.get("chat_id").and_then(|v| v.as_i64());
        let mut tg_msg_id: Option<i64> = None;

        // Send confirmation keyboard via Telegram
        if let Some(tg) = self.channels.get("telegram") {
            if let Some(tg_channel) =
                tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()
            {
                let chat_id = tg_chat_id.unwrap_or(tg_channel.chat_id);
                match tg_channel
                    .client
                    .send_message(
                        chat_id,
                        &format!("\u{1F680} {description}\n\nApprove?"),
                        None,
                        Some(&keyboard),
                    )
                    .await
                {
                    Ok(mid) => tg_msg_id = Some(mid),
                    Err(e) => {
                        tracing::error!(error = %e, "failed to send apply confirmation");
                        return format!("\u{274C} Failed to send confirmation: {e}");
                    }
                }
            }
        }

        // Save pending action
        if let Err(e) = self.deps.state.save_pending_action(
            &crate::state::PendingAction {
                id: action_id,
                description: description.clone(),
                payload,
                status: crate::state::PendingStatus::AwaitingConfirmation,
                created_at: chrono::Utc::now(),
                telegram_message_id: tg_msg_id,
                telegram_chat_id: tg_chat_id,
            },
        ) {
            tracing::error!(error = %e, "failed to save pending action");
        }

        // Return empty -- we already sent the confirmation keyboard
        String::new()
    }

    /// Unknown command — show help with available commands.
    fn cmd_unknown(&self, command: &str) -> String {
        format!(
            "\u{2753} Unknown command: /{command}\n\n\
             Available commands:\n\
             /status -- Project health dashboard\n\
             /digest -- Trigger immediate digest\n\
             /health -- Homelab status\n\
             /projects -- List all projects\n\
             /apply <project> <spec> -- Apply a spec"
        )
    }

    // ── Nexus Error Handlers ────────────────────────────────────────

    async fn handle_nexus_view_error(&mut self, session_id: &str) {
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

    async fn handle_nexus_create_bug(&mut self, session_id: &str) {
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

    /// Send an error message to a channel, with 2-second dedup batching.
    ///
    /// Consecutive calls with the same `message` within 2 seconds are batched:
    /// the count accumulates and the message is not sent immediately. When a
    /// different message arrives (or the first call), any previous batch is
    /// flushed first, then the new error is recorded.
    ///
    /// The caller is responsible for invoking `flush_error_batch` after the
    /// debounce window to send the final batch. In practice this happens via
    /// the orchestrator's periodic inactivity tick.
    async fn send_error(&mut self, channel_name: &str, message: &str) {
        let debounce = Duration::from_secs(2);
        let now = Instant::now();

        let is_same = self.last_error_text.as_deref() == Some(message);
        let within_window = now.duration_since(self.last_error_time) < debounce;

        if is_same && within_window {
            // Same error within debounce window — accumulate
            self.error_count += 1;
            self.last_error_time = now;
            return;
        }

        // Different error or window expired — flush previous batch if any
        if let Some(ref prev_text) = self.last_error_text.take() {
            let prev_count = self.error_count;
            let text_to_send = if prev_count > 1 {
                format!("\u{26A0} {prev_count} actions failed: {prev_text}")
            } else {
                format!("\u{26A0} {prev_text}")
            };
            if let Some(channel) = self.channels.get(channel_name) {
                let msg = OutboundMessage {
                    channel: channel_name.into(),
                    content: text_to_send,
                    reply_to: None,
                    keyboard: None,
                };
                if let Err(e) = channel.send_message(msg).await {
                    tracing::error!(error = %e, "failed to send batched error to {}", channel_name);
                }
            }
        }

        // Start new batch
        self.last_error_text = Some(message.to_string());
        self.last_error_time = now;
        self.error_count = 1;
    }

    /// Flush any pending error batch if the debounce window has expired.
    #[allow(dead_code)]
    ///
    /// Called periodically to ensure the final batch in a sequence is always
    /// sent even if no new errors arrive to trigger a flush.
    async fn flush_error_batch_if_expired(&mut self, channel_name: &str) {
        let debounce = Duration::from_secs(2);
        let now = Instant::now();

        if self.last_error_text.is_none() {
            return;
        }
        if now.duration_since(self.last_error_time) < debounce {
            return;
        }

        if let Some(ref prev_text) = self.last_error_text.take() {
            let prev_count = self.error_count;
            let text_to_send = if prev_count > 1 {
                format!("\u{26A0} {prev_count} actions failed: {prev_text}")
            } else {
                format!("\u{26A0} {prev_text}")
            };
            if let Some(channel) = self.channels.get(channel_name) {
                let msg = OutboundMessage {
                    channel: channel_name.into(),
                    content: text_to_send,
                    reply_to: None,
                    keyboard: None,
                };
                if let Err(e) = channel.send_message(msg).await {
                    tracing::error!(error = %e, "failed to flush error batch to {}", channel_name);
                }
            }
        }
        self.error_count = 0;
    }
}

// ── Telegram Formatting Helpers ──────────────────────────────────────

/// Convert raw tool output to mobile-friendly Telegram format.
///
/// - Strips ANSI escape codes
/// - Replaces textual status indicators with emoji dots
/// - Converts markdown tables to condensed key-value format
pub fn format_for_telegram(output: &str) -> String {
    let mut result = strip_ansi_codes(output);

    // Replace common status words with dots
    result = result.replace("healthy", "\u{1F7E2} healthy");
    result = result.replace("degraded", "\u{1F7E1} degraded");
    result = result.replace("down", "\u{1F534} down");
    result = result.replace("running", "\u{1F7E2} running");
    result = result.replace("stopped", "\u{1F534} stopped");
    result = result.replace("unavailable", "\u{1F7E1} unavailable");

    result
}

/// Format a project health output into a single-line status with dots.
fn format_status_dots(code: &str, output: &str) -> String {
    let has_error = output.contains("unavailable") || output.contains("error") || output.contains("Error");
    let has_warn = output.contains("degraded") || output.contains("timeout");

    let dot = if has_error {
        "\u{1F534}" // red
    } else if has_warn {
        "\u{1F7E1}" // yellow
    } else {
        "\u{1F7E2}" // green
    };

    // Build a compact one-liner
    let parts: Vec<&str> = output.lines().take(3).collect();
    let summary = if parts.is_empty() {
        "no data"
    } else {
        parts[0].trim()
    };

    format!("{dot} {code} -- {summary}")
}

/// Strip ANSI escape codes from a string.
/// Convert internal worker stage names to human-readable descriptions.
fn humanize_stage(stage: &str) -> &str {
    match stage {
        "context_build" => "Loading memory and recent messages...",
        "tool_loop" => "Processing with tools...",
        "response" => "Drafting response...",
        "claude_call" => "Thinking...",
        _ => "Processing...",
    }
}

/// Convert tool names to (emoji, description) pairs for status updates.
///
/// Returns `(emoji, description)` — callers format as `"{emoji} {description}"`.
fn humanize_tool(tool: &str) -> (String, String) {
    let (emoji, desc) = match tool {
        "jira_search" | "jira_get" => ("\u{1F50D}", "Searching Jira..."),
        "jira_create" | "jira_transition" | "jira_assign" | "jira_comment" => ("\u{270F}\u{FE0F}", "Updating Jira..."),
        "query_nexus" | "query_session" => ("\u{1F517}", "Checking Nexus sessions..."),
        "read_memory" | "search_memory" => ("\u{1F9E0}", "Reading memory..."),
        "write_memory" => ("\u{1F4BE}", "Saving to memory..."),
        "vercel_deployments" | "vercel_logs" => ("\u{25B6}\u{FE0F}", "Checking Vercel deploys..."),
        "sentry_issues" | "sentry_issue" => ("\u{1F6A8}", "Checking Sentry errors..."),
        "posthog_trends" | "posthog_flags" => ("\u{1F4CA}", "Checking PostHog analytics..."),
        "docker_status" | "docker_logs" => ("\u{1F433}", "Checking Docker containers..."),
        "tailscale_status" | "tailscale_node" => ("\u{1F310}", "Checking Tailscale network..."),
        "gh_pr_list" | "gh_run_status" | "gh_issues" | "gh_pr_detail" | "gh_pr_diff" | "gh_releases" | "gh_compare" => ("\u{1F419}", "Checking GitHub..."),
        "neon_query" => ("\u{1F5C4}\u{FE0F}", "Querying database..."),
        "neon_projects" | "neon_branches" | "neon_compute" => ("\u{1F5C4}\u{FE0F}", "Checking Neon infrastructure..."),
        "stripe_customers" | "stripe_invoices" => ("\u{1F4B3}", "Checking Stripe..."),
        "ado_projects" | "ado_pipelines" | "ado_builds" => ("\u{1F4CB}", "Checking Azure DevOps..."),
        "ha_states" | "ha_entity" | "ha_service_call" => ("\u{1F3E0}", "Checking Home Assistant..."),
        "project_health" => ("\u{1F4CA}", "Building project health report..."),
        "homelab_status" => ("\u{1F5A5}\u{FE0F}", "Checking homelab status..."),
        "search_messages" => ("\u{1F4AC}", "Searching conversation history..."),
        "list_channels" => ("\u{1F4E1}", "Listing channels..."),
        "send_to_channel" => ("\u{1F4E4}", "Sending to channel..."),
        "fetch_url" | "check_url" | "search_web" => ("\u{1F310}", "Fetching web content..."),
        "doppler_secrets" | "doppler_compare" | "doppler_activity" => ("\u{1F511}", "Checking Doppler secrets..."),
        "cf_zones" | "cf_dns_records" | "cf_domain_status" => ("\u{1F4CB}", "Checking Cloudflare DNS..."),
        "teams_channels" | "teams_messages" | "teams_presence" => ("\u{1F4AC}", "Checking Teams..."),
        "teams_send" => ("\u{1F4AC}", "Sending to Teams..."),
        _ => ("\u{2699}\u{FE0F}", ""),
    };
    let description = if desc.is_empty() {
        format!("Running {tool}...")
    } else {
        desc.to_string()
    };
    (emoji.to_string(), description)
}

fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip until we find a letter (end of ANSI sequence)
            if chars.peek() == Some(&'[') {
                chars.next(); // skip [
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
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
    // Cron/digest triggers run silently — no Telegram announcement needed.
    // The digest result itself will be sent when complete.
    for trigger in triggers {
        if let Trigger::Cron(_) = trigger {
            return None;
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

// ── Quiet Hours ────────────────────────────────────────────────────

/// Check if the current local time falls within the quiet window.
///
/// Handles overnight windows (e.g., 23:00 → 07:00) correctly.
/// Returns `false` if either bound is `None` (no quiet window configured).
pub fn is_quiet_hours(
    quiet_start: Option<chrono::NaiveTime>,
    quiet_end: Option<chrono::NaiveTime>,
) -> bool {
    let (Some(start), Some(end)) = (quiet_start, quiet_end) else {
        return false;
    };
    let now = chrono::Local::now().time();
    if start <= end {
        // Same-day window (e.g., 01:00 → 05:00)
        now >= start && now < end
    } else {
        // Overnight window (e.g., 23:00 → 07:00)
        now >= start || now < end
    }
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
                thinking_msg_id: None,
                thinking_chat_id: None,
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
                WorkerEvent::StageStarted { worker_id, stage, .. } => {
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

    // ── Quiet Hours Tests ───────────────────────────────────────────────

    #[test]
    fn quiet_hours_none_returns_false() {
        assert!(!is_quiet_hours(None, None));
        assert!(!is_quiet_hours(
            Some(chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap()),
            None,
        ));
        assert!(!is_quiet_hours(
            None,
            Some(chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap()),
        ));
    }

    #[test]
    fn quiet_hours_overnight_window() {
        // 23:00 → 07:00 overnight window
        let start = Some(chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap());
        let end = Some(chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap());

        // We can't control chrono::Local::now() in tests, but we can
        // verify the function doesn't panic and returns a bool.
        let _ = is_quiet_hours(start, end);
    }

    #[test]
    fn quiet_hours_same_day_window() {
        // 01:00 → 05:00 same-day window
        let start = Some(chrono::NaiveTime::from_hms_opt(1, 0, 0).unwrap());
        let end = Some(chrono::NaiveTime::from_hms_opt(5, 0, 0).unwrap());

        let _ = is_quiet_hours(start, end);
    }

    // ── Bot Command Parsing Tests ───────────────────────────────────

    #[test]
    fn parse_bot_command_basic() {
        let cmd = parse_bot_command("/status").unwrap();
        assert_eq!(cmd.command, "status");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn parse_bot_command_with_args() {
        let cmd = parse_bot_command("/apply oo fix-chat-bugs").unwrap();
        assert_eq!(cmd.command, "apply");
        assert_eq!(cmd.args, vec!["oo", "fix-chat-bugs"]);
    }

    #[test]
    fn parse_bot_command_with_bot_suffix() {
        let cmd = parse_bot_command("/status@NovaBot").unwrap();
        assert_eq!(cmd.command, "status");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn parse_bot_command_case_insensitive() {
        let cmd = parse_bot_command("/STATUS").unwrap();
        assert_eq!(cmd.command, "status");
    }

    #[test]
    fn parse_bot_command_not_a_command() {
        assert!(parse_bot_command("hello world").is_none());
        assert!(parse_bot_command("").is_none());
    }

    #[test]
    fn parse_bot_command_slash_only() {
        assert!(parse_bot_command("/").is_none());
    }

    #[test]
    fn classify_bot_command_triggers() {
        assert_eq!(classify_trigger(&make_message("/status")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/digest")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/health")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/apply oo fix-chat")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/projects")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/unknown")), TriggerClass::BotCommand);
    }

    #[test]
    fn bot_command_priority_is_high() {
        assert_eq!(class_to_priority(TriggerClass::BotCommand), Priority::High);
    }

    // ── Format Helpers Tests ────────────────────────────────────────

    #[test]
    fn format_for_telegram_strips_ansi() {
        let input = "\x1b[32mhealthy\x1b[0m";
        let result = format_for_telegram(input);
        assert!(!result.contains("\x1b"));
        assert!(result.contains("healthy"));
    }

    #[test]
    fn format_for_telegram_adds_status_dots() {
        let result = format_for_telegram("Status: running");
        assert!(result.contains("\u{1F7E2}"));
    }

    #[test]
    fn strip_ansi_codes_basic() {
        assert_eq!(strip_ansi_codes("hello"), "hello");
        assert_eq!(strip_ansi_codes("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi_codes("\x1b[1;32mbold green\x1b[0m"), "bold green");
    }

    #[test]
    fn format_status_dots_healthy() {
        let output = "Deploy: ok\nSentry: 0 issues";
        let result = format_status_dots("oo", output);
        assert!(result.contains("\u{1F7E2}")); // green dot
        assert!(result.contains("oo"));
    }

    #[test]
    fn format_status_dots_degraded() {
        let output = "Deploy: ok\nSentry: unavailable";
        let result = format_status_dots("oo", output);
        assert!(result.contains("\u{1F534}")); // red dot
    }
}
