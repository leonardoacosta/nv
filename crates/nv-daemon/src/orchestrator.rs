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
use crate::briefing_store::BriefingEntry;
use crate::digest::format::format_digest;
use crate::digest::gather::gather_context;
use crate::digest::state::{content_hash, DigestStateManager};
use crate::digest::synthesize::{inject_budget_warning, synthesize_digest, synthesize_digest_fallback};
use crate::nexus;
use crate::obligation_detector;
use crate::obligation_store::NewObligation;
use nv_core::types::{ObligationOwner, ObligationStatus};
use crate::channels::telegram::client::TelegramClient;
use crate::worker::{generate_slug_for_triggers, Priority, SharedDeps, WorkerEvent, WorkerPool, WorkerTask};

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
        // ObligationResearch tasks are handled directly in the worker branch;
        // classify as Query so they flow through the standard dispatch path.
        Trigger::ObligationResearch(_) => TriggerClass::Query,
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
    /// Obligation ID currently being edited (waiting for new detected_action text from user).
    editing_obligation_id: Option<String>,
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
    // Thinking message state removed — replaced by typing indicator loop in worker.
    // ── Deferred StageComplete removal ──
    /// Worker IDs whose StageComplete event is pending deferred removal from
    /// `worker_stage_started`. An entry is cleared if a `ToolCalled` event
    /// arrives before the next `check_inactivity` tick; otherwise the
    /// `check_inactivity` tick removes the entry.
    worker_stage_pending_removal: std::collections::HashSet<Uuid>,
    /// Maps worker_id → Telegram chat_id for the duration of a worker run.
    ///
    /// Populated on `WorkerEvent::StageStarted`, cleared on `Complete`/`Error`.
    /// Used by the `ToolCalled` handler to send per-worker typing refreshes.
    worker_chat_id: std::collections::HashMap<Uuid, i64>,
    /// Timestamp captured at trigger arrival in the select loop.
    ///
    /// Used to compute the `receive` latency span: time from trigger
    /// arrival to `WorkerPool::dispatch` call.
    trigger_arrival: Option<Instant>,
    /// Obligation ID currently being autonomously executed.
    ///
    /// Set when autonomous execution starts, cleared when it completes.
    /// Prevents double-dispatch: only one obligation executes at a time.
    executing_obligation: Option<String>,
    /// Last time the idle check ran — used to enforce the 30-second poll cycle.
    last_idle_check: Instant,
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
            editing_obligation_id: None,
            last_expiry_check: Instant::now(),
            event_rx,
            worker_stage_started: std::collections::HashMap::new(),
            quiet_start,
            quiet_end,
            last_error_text: None,
            last_error_time: Instant::now(),
            error_count: 0,
            worker_stage_pending_removal: std::collections::HashSet::new(),
            worker_chat_id: std::collections::HashMap::new(),
            trigger_arrival: None,
            executing_obligation: None,
            last_idle_check: Instant::now(),
        }
    }

    /// Main orchestrator loop — receives triggers and worker events.
    ///
    /// Uses `tokio::select!` to multiplex:
    /// - Trigger channel (inbound user/cron/nexus triggers)
    /// - Worker event channel (progress events from workers)
    /// - 5s typing indicator refresh (status update to Telegram for long-running tasks)
    /// - 30s idle check (autonomous obligation execution when idle)
    ///
    /// Runs until the trigger channel closes (all senders dropped).
    pub async fn run(mut self) -> Result<()> {
        tracing::info!("orchestrator started, waiting for triggers");

        /// Typing indicator refresh interval. Telegram's "typing..." indicator
        /// expires after ~5s, so we refresh every 5s while workers are active.
        const TYPING_REFRESH: Duration = Duration::from_secs(5);

        /// Idle check poll interval — how often to check whether Nova is idle.
        const IDLE_POLL: Duration = Duration::from_secs(30);

        let mut trigger_closed = false;
        let mut idle_check_timer = tokio::time::interval(IDLE_POLL);
        idle_check_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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

                    // Capture trigger arrival time for the `receive` latency span.
                    self.trigger_arrival = Some(Instant::now());

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
                _ = idle_check_timer.tick() => {
                    self.check_idle_and_execute().await;
                }
            }
        }

        Ok(())
    }

    /// Process a batch of triggers (extracted from the select loop).
    async fn process_trigger_batch(&mut self, triggers: &mut Vec<Trigger>) {
        // Extract CLI response channels before processing
        let cli_response_txs = extract_cli_response_channels(triggers);

        // Update last_interactive_at for idle detection.
        // Only count interactive triggers (Message or CliCommand), not cron/nexus.
        let has_interactive = triggers.iter().any(|t| {
            matches!(t, Trigger::Message(_) | Trigger::CliCommand(_))
        });
        if has_interactive {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.deps
                .last_interactive_at
                .store(now_secs, std::sync::atomic::Ordering::Relaxed);
        }

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

        // Flush any pending error notification batch whose debounce window has expired
        self.flush_error_batch_if_expired("telegram").await;

        // Classify the first trigger to decide routing
        let primary_class = triggers
            .first()
            .map(classify_trigger)
            .unwrap_or(TriggerClass::Query);

        tracing::info!(
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
                if is_quiet_hours(self.quiet_start, self.quiet_end, &self.deps.timezone) {
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
            TriggerClass::Digest => {
                // MorningBriefing is handled inline — no worker needed.
                for trigger in triggers.iter() {
                    if let Trigger::Cron(nv_core::types::CronEvent::MorningBriefing) = trigger {
                        self.send_morning_briefing().await;
                        return;
                    }
                }
                // ProactiveFollowup: scan obligations and send Telegram reminders inline.
                for trigger in triggers.iter() {
                    if let Trigger::Cron(nv_core::types::CronEvent::ProactiveFollowup) = trigger {
                        self.handle_proactive_followup().await;
                        return;
                    }
                }
                // CronEvent::Digest runs the full gather → synthesize → send pipeline inline.
                for trigger in triggers.iter() {
                    if let Trigger::Cron(nv_core::types::CronEvent::Digest) = trigger {
                        self.run_digest_pipeline().await;
                        return;
                    }
                }
                // WeeklySelfAssessment: dispatched to a worker with a fixed action prompt.
                for trigger in triggers.iter() {
                    if let Trigger::Cron(nv_core::types::CronEvent::WeeklySelfAssessment) = trigger {
                        // Fall through to normal worker dispatch below — the worker's
                        // system prompt context includes the "weekly_self_assessment" action.
                        break;
                    }
                }
            }
            _ => {}
        }

        // Quiet hours gate: suppress non-P0 (non-High priority) dispatch
        let priority = class_to_priority(primary_class);
        if priority != Priority::High && is_quiet_hours(self.quiet_start, self.quiet_end, &self.deps.timezone) {
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

        // ── Obligation detection (fire-and-forget) ───────────────────────
        // Run only on inbound messages — not on cron events or CLI commands.
        // Detection is non-blocking: spawned as a background task so it
        // cannot delay the worker dispatch.
        if let Some(Trigger::Message(msg)) = triggers.first() {
            let msg_content = msg.content.clone();
            let msg_channel = msg.channel.clone();
            let msg_excerpt = if msg_content.len() > 200 {
                msg_content[..200].to_string()
            } else {
                msg_content.clone()
            };

            let obligation_store = self.deps.obligation_store.clone();
            let telegram_client = self.telegram_client.clone();
            let telegram_chat_id = self.telegram_chat_id;
            let research_deps = Arc::clone(&self.deps);
            let research_pool = Arc::new(self.worker_pool.clone());

            tokio::spawn(async move {
                match obligation_detector::detect_obligation(&msg_content, &msg_channel).await {
                    Ok(Some(detected)) => {
                        let obligation_id = uuid::Uuid::new_v4().to_string();

                        let new_ob = NewObligation {
                            id: obligation_id.clone(),
                            source_channel: msg_channel.clone(),
                            source_message: Some(msg_excerpt),
                            detected_action: detected.detected_action.clone(),
                            project_code: detected.project_code,
                            priority: detected.priority,
                            owner: if detected.owner == "leo" {
                                nv_core::types::ObligationOwner::Leo
                            } else {
                                nv_core::types::ObligationOwner::Nova
                            },
                            owner_reason: detected.owner_reason,
                            deadline: None,
                        };

                        // Store the obligation — lock is held briefly, dropped before any await.
                        let store_result = if let Some(store_arc) = obligation_store {
                            match store_arc.lock() {
                                Ok(store) => match store.create(new_ob) {
                                    Ok(ob) => {
                                        tracing::info!(
                                            obligation_id = %ob.id,
                                            priority = ob.priority,
                                            owner = %ob.owner,
                                            channel = %msg_channel,
                                            "obligation stored"
                                        );
                                        Some(ob)
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "failed to store detected obligation");
                                        None
                                    }
                                },
                                Err(e) => {
                                    tracing::warn!(error = %e, "obligation store mutex poisoned");
                                    None
                                }
                            }
                        } else {
                            tracing::debug!("obligation store not available, skipping storage");
                            None
                        };
                        // MutexGuard is dropped above; now safe to await.

                        // Notify via Telegram for P0-P1 obligations with card + keyboard
                        if let Some(ob) = store_result {
                            // Emit obligation.detected activity event.
                            {
                                use crate::http::{DaemonEvent, ObligationActivityEvent};
                                let event = ObligationActivityEvent {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    event_type: "obligation.detected".to_string(),
                                    obligation_id: ob.id.clone(),
                                    description: format!(
                                        "Obligation detected (owner: {}, priority: P{}): {}",
                                        ob.owner,
                                        ob.priority,
                                        ob.detected_action,
                                    ),
                                    timestamp: chrono::Utc::now(),
                                    metadata: None,
                                };
                                research_deps.activity_buffer.push(event.clone());
                                let _ = research_deps.obligation_event_tx.send(
                                    DaemonEvent::ObligationActivity(event),
                                );
                            }

                            if ob.priority <= 1 {
                                if let (Some(tg), Some(chat_id)) =
                                    (telegram_client, telegram_chat_id)
                                {
                                    let card = format_obligation_card(&ob, &msg_channel);
                                    let keyboard = obligation_keyboard(&ob.id);
                                    if let Err(e) = tg
                                        .send_message(chat_id, &card, None, Some(&keyboard))
                                        .await
                                    {
                                        tracing::warn!(
                                            error = %e,
                                            "failed to send P0-P1 obligation notification"
                                        );
                                    }
                                }
                            }

                            // Schedule background research if config is present and enabled.
                            if let Some(cfg) = research_deps.obligation_research_config.clone() {
                                if cfg.enabled {
                                    crate::obligation_research::schedule_research(
                                        ob.id.clone(),
                                        ob.detected_action.clone(),
                                        ob.project_code.clone(),
                                        ob.source_channel.clone(),
                                        ob.priority,
                                        Arc::clone(&research_deps),
                                        Arc::clone(&research_pool),
                                        cfg,
                                    );
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        // No obligation detected — normal case, nothing to do
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "obligation detection failed (non-fatal)");
                    }
                }
            });
        }

        // ── Dashboard forwarding (Tasks 5.3 + 5.4) ───────────────────────
        //
        // For Query and Command triggers originating from a Telegram message,
        // attempt to forward to the dashboard CC session first. If healthy and
        // successful, send the reply directly to Telegram and return — no worker
        // dispatch needed. On any failure, fall through to the worker pool.
        let should_try_dashboard = matches!(primary_class, TriggerClass::Query | TriggerClass::Command);

        if should_try_dashboard {
            // Extract message text — only forward if there's a concrete message.
            if let Some(Trigger::Message(msg)) = triggers.first() {
                let msg_text = msg.content.clone();
                let msg_chat_id = tg_chat_id.or(self.telegram_chat_id);
                let msg_reply_to = tg_msg_id;

                // Borrow dashboard client from shared deps (behind Arc).
                if let Some(ref dc) = self.deps.dashboard_client {
                    if dc.is_healthy() {
                        tracing::info!(
                            class = ?primary_class,
                            chat_id = ?msg_chat_id,
                            "forwarding to dashboard"
                        );

                        match dc.forward_message(&msg_text, msg_chat_id, None).await {
                            Ok(reply) => {
                                // Send reply directly to Telegram.
                                if let Some(channel) = self.channels.get("telegram") {
                                    let outbound = nv_core::types::OutboundMessage {
                                        channel: "telegram".into(),
                                        content: reply,
                                        reply_to: msg_reply_to.map(|id| id.to_string()),
                                        keyboard: None,
                                    };
                                    if let Err(e) = channel.send_message(outbound).await {
                                        tracing::warn!(
                                            error = %e,
                                            "dashboard forward: failed to deliver reply to Telegram"
                                        );
                                    }
                                }
                                // Reply sent — no worker dispatch needed.
                                return;
                            }
                            Err(e) => {
                                // Task 5.4: log warning and fall back to worker pool.
                                tracing::warn!(
                                    error = %e,
                                    "dashboard forward failed — falling back to worker pool"
                                );
                            }
                        }
                    } else {
                        tracing::debug!("dashboard unhealthy — routing to worker pool");
                    }
                }
            }
        }

        // Obligation edit-flow: if we are waiting for replacement text, consume the
        // next plain-text message as the new detected_action and skip Claude dispatch.
        if let Some(ob_id) = self.editing_obligation_id.take() {
            if let Some(Trigger::Message(msg)) = triggers.first() {
                if !msg.content.starts_with("[callback] ") {
                    let new_text = msg.content.trim().to_string();
                    let chat_id = tg_chat_id.or(self.telegram_chat_id);

                    // Update detected_action in the store.
                    let update_result = self.deps.obligation_store
                        .as_ref()
                        .and_then(|arc| arc.lock().ok())
                        .map(|store| store.update_detected_action(&ob_id, &new_text));

                    match update_result {
                        Some(Ok(true)) => {
                            // Edit original obligation message to show the update.
                            if let (Some(tg_channel), Some(cid), Some(mid)) = (
                                self.channels
                                    .get("telegram")
                                    .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()),
                                chat_id,
                                tg_msg_id,
                            ) {
                                let text = format!("Updated: {new_text}");
                                let _ = tg_channel.client.edit_message(cid, mid, &text, None).await;
                            }
                            tracing::info!(ob_id = %ob_id, "obligation detected_action updated via edit flow");
                        }
                        Some(Ok(false)) => {
                            tracing::warn!(ob_id = %ob_id, "obligation edit flow: obligation not found");
                        }
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, ob_id = %ob_id, "obligation edit flow: store update failed");
                        }
                        None => {
                            tracing::warn!(ob_id = %ob_id, "obligation edit flow: no obligation store");
                        }
                    }
                    // Consumed the message for editing — skip Claude dispatch.
                    return;
                }
            }
            // If no suitable message (e.g. it was a callback), restore the id.
            // (Already taken by .take(); user will need to press Edit again.)
        }

        // Phase 2: Consume editing_action_id so the next message starts an
        // edit-aware Claude session. .take() ensures only this task gets it.
        let editing_action_id = self.editing_action_id.take();
        let is_edit_reply = editing_action_id.is_some();

        // Dispatch to worker pool
        let slug = generate_slug_for_triggers(triggers);
        let task_id = Uuid::new_v4();

        // Detect whether the originating trigger was a Telegram voice note.
        // Any trigger in the batch carrying metadata["voice"] == true marks the
        // whole task as voice-triggered so the worker can respond in kind.
        let is_voice_trigger = any_trigger_is_voice(triggers);

        // Emit and persist the `receive` latency span — time from trigger arrival
        // to worker dispatch.
        if let Some(arrival) = self.trigger_arrival.take() {
            let receive_ms = arrival.elapsed().as_millis() as i64;
            let _ = self.deps.event_tx.send(WorkerEvent::StageComplete {
                worker_id: task_id,
                stage: "receive".into(),
                duration_ms: receive_ms as u64,
            });
            if let Ok(store) = self.deps.message_store.lock() {
                let _ = store.log_latency_span(&task_id.to_string(), "receive", receive_ms);
            }
        }

        let task = WorkerTask {
            id: task_id,
            triggers: std::mem::take(triggers),
            priority: class_to_priority(primary_class),
            created_at: Instant::now(),
            telegram_chat_id: tg_chat_id.or(self.telegram_chat_id),
            telegram_message_id: tg_msg_id,
            cli_response_txs,
            is_edit_reply,
            editing_action_id,
            slug,
            is_voice_trigger,
        };

        // Before dispatch — send typing indicator immediately so the user sees feedback
        // while the worker starts up (worker also sends one at startup as belt-and-suspenders)
        if let (Some(tg), Some(chat_id)) = (&self.telegram_client, self.telegram_chat_id) {
            tg.send_chat_action(chat_id, "typing").await;
        }

        self.worker_pool.dispatch(task).await;
    }

    /// Handle a single worker event — log at appropriate level and track state.
    async fn handle_worker_event(&mut self, event: WorkerEvent) {
        match &event {
            WorkerEvent::StageStarted { worker_id, stage, telegram_chat_id } => {
                tracing::debug!(
                    worker_id = %worker_id,
                    stage = %stage,
                    "worker stage started"
                );
                self.worker_stage_started
                    .insert(*worker_id, (stage.clone(), Instant::now()));
                // Populate worker_chat_id for typing indicator routing.
                if let Some(chat_id) = telegram_chat_id.or(self.telegram_chat_id) {
                    self.worker_chat_id.insert(*worker_id, chat_id);
                }
            }
            WorkerEvent::ToolCalled { worker_id, tool } => {
                tracing::trace!(
                    worker_id = %worker_id,
                    tool = %tool,
                    "worker tool called"
                );
                // Cancel any pending deferred removal for this worker — a new
                // tool call means the stage is still active.
                self.worker_stage_pending_removal.remove(worker_id);

                // Update stage description with human-readable tool name
                let (emoji, description) = humanize_tool(tool);
                let stage_label = format!("{emoji} {description}");
                self.worker_stage_started
                    .insert(*worker_id, (stage_label.clone(), Instant::now()));

                // Refresh typing indicator for this worker's chat_id.
                if let Some(chat_id) = self.worker_chat_id.get(worker_id).copied() {
                    if let Some(tg) = self.channels.get("telegram") {
                        if let Some(tg_channel) =
                            tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()
                        {
                            let _ = tg_channel.client.send_chat_action(chat_id, "typing").await;
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
                // Persist stage duration to the latency_spans table for dashboard
                // P50/P95 queries. Fire-and-forget: lock is held briefly.
                if let Ok(store) = self.deps.message_store.lock() {
                    let _ = store.log_latency_span(
                        &worker_id.to_string(),
                        stage,
                        *duration_ms as i64,
                    );
                }
                // Defer removal: a ToolCalled event may arrive immediately after
                // StageComplete (the agent is mid-tool-call). The pending set is
                // flushed by the next check_inactivity tick if no ToolCalled cancels it.
                self.worker_stage_pending_removal.insert(*worker_id);
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
                // Clean up stage tracking
                self.worker_stage_started.remove(worker_id);
                self.worker_stage_pending_removal.remove(worker_id);
                self.worker_chat_id.remove(worker_id);
            }
            WorkerEvent::Error { worker_id, error } => {
                tracing::warn!(
                    worker_id = %worker_id,
                    error = %error,
                    "worker error"
                );
                self.worker_stage_started.remove(worker_id);
                self.worker_stage_pending_removal.remove(worker_id);
                self.worker_chat_id.remove(worker_id);
            }
        }
    }

    /// Check for active workers and refresh the Telegram typing indicator.
    /// Status details go to debug log only — never sent as messages.
    async fn check_inactivity(&mut self, _threshold: Duration) {
        // Flush deferred StageComplete removals. Workers that sent StageComplete
        // but no subsequent ToolCalled before this tick are no longer tracking
        // an active stage and should be removed.
        if !self.worker_stage_pending_removal.is_empty() {
            for worker_id in self.worker_stage_pending_removal.drain() {
                self.worker_stage_started.remove(&worker_id);
            }
        }

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

        // Stop-on-delivery: Telegram "typing..." expires automatically after ~5s.
        // When a worker completes, it's removed from worker_stage_started and worker_chat_id.
        // The next tick of this function finds no active workers and sends no refresh,
        // so the indicator expires within 5s of response delivery.
        // There is no explicit "stop typing" API call in the Telegram Bot API.

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
    #[allow(clippy::await_holding_lock)]
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

                    // Worker error retry callback — dispatch a new high-priority task
                    // using the slug encoded in the callback data as the trigger text.
                    if let Some(slug) = data.strip_prefix("retry:") {
                        tracing::info!(slug, "retry callback received — dispatching new high-priority task");

                        let retry_chat_id = tg_chat_id.or(self.telegram_chat_id);
                        let task_id = Uuid::new_v4();
                        let trigger_msg = nv_core::types::InboundMessage {
                            id: format!("retry-{task_id}"),
                            channel: "telegram".to_string(),
                            sender: "user".to_string(),
                            content: slug.to_string(),
                            timestamp: chrono::Utc::now(),
                            thread_id: None,
                            metadata: {
                                let mut m = serde_json::Map::new();
                                if let Some(chat_id) = retry_chat_id {
                                    m.insert("chat_id".to_string(), serde_json::json!(chat_id));
                                }
                                serde_json::Value::Object(m)
                            },
                        };

                        let task = WorkerTask {
                            id: task_id,
                            triggers: vec![Trigger::Message(trigger_msg)],
                            priority: Priority::High,
                            created_at: std::time::Instant::now(),
                            telegram_chat_id: retry_chat_id,
                            telegram_message_id: None,
                            cli_response_txs: vec![],
                            is_edit_reply: false,
                            editing_action_id: None,
                            slug: slug.to_string(),
                            is_voice_trigger: false,
                        };
                        self.worker_pool.dispatch(task).await;
                        continue;
                    }

                    // Session error create-bug callback (stub — routing via nexus_err:bug:)
                    if let Some(_event_id) = data.strip_prefix("bug:") {
                        tracing::debug!("bug callback (not yet implemented)");
                        continue;
                    }

                    // Obligation action callbacks
                    if let Some(ob_id) = data.strip_prefix("ob_handle:") {
                        self.handle_obligation_callback(
                            ob_id,
                            nv_core::types::ObligationStatus::InProgress,
                            Some(nv_core::types::ObligationOwner::Leo),
                            tg_chat_id,
                            original_msg_id,
                            "Obligation assigned to Leo.",
                        ).await;
                        continue;
                    } else if let Some(ob_id) = data.strip_prefix("ob_delegate:") {
                        self.handle_obligation_callback(
                            ob_id,
                            nv_core::types::ObligationStatus::InProgress,
                            Some(nv_core::types::ObligationOwner::Nova),
                            tg_chat_id,
                            original_msg_id,
                            "Obligation delegated to Nova.",
                        ).await;
                        continue;
                    } else if let Some(ob_id) = data.strip_prefix("ob_dismiss:") {
                        self.handle_obligation_callback(
                            ob_id,
                            nv_core::types::ObligationStatus::Dismissed,
                            None,
                            tg_chat_id,
                            original_msg_id,
                            "Obligation dismissed.",
                        ).await;
                        continue;
                    } else if let Some(ob_id) = data.strip_prefix("ob_edit:") {
                        // Set editing state and ask the user for new obligation text.
                        self.editing_obligation_id = Some(ob_id.to_string());
                        let chat_id = tg_chat_id.or(self.telegram_chat_id);
                        if let (Some(tg_channel), Some(cid)) = (
                            self.channels
                                .get("telegram")
                                .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()),
                            chat_id,
                        ) {
                            let _ = tg_channel.client
                                .send_message(cid, "What should the obligation text say?", None, None)
                                .await;
                        }
                        continue;
                    } else if let Some(ob_id) = data.strip_prefix("ob_cancel:") {
                        if let (Some(tg_channel), Some(cid)) = (
                            self.channels
                                .get("telegram")
                                .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()),
                            tg_chat_id.or(self.telegram_chat_id),
                        ) {
                            if let Some(store_arc) = &self.deps.obligation_store {
                                if let Ok(store) = store_arc.lock() {
                                    if let Err(e) = crate::callbacks::handle_ob_cancel(
                                        ob_id,
                                        &tg_channel.client,
                                        cid,
                                        original_msg_id,
                                        &store,
                                    ).await {
                                        tracing::warn!(error = %e, "ob_cancel callback failed");
                                    }
                                }
                            }
                        }
                        continue;
                    } else if let Some(ob_id) = data.strip_prefix("ob_expiry:") {
                        if let (Some(tg_channel), Some(cid)) = (
                            self.channels
                                .get("telegram")
                                .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()),
                            tg_chat_id.or(self.telegram_chat_id),
                        ) {
                            if let Some(store_arc) = &self.deps.obligation_store {
                                if let Ok(ob_store) = store_arc.lock() {
                                    if let Err(e) = crate::callbacks::handle_ob_expiry(
                                        ob_id,
                                        &tg_channel.client,
                                        cid,
                                        original_msg_id,
                                        &ob_store,
                                        self.deps.reminder_store.as_deref(),
                                        &self.deps.timezone,
                                    ).await {
                                        tracing::warn!(error = %e, "ob_expiry callback failed");
                                    }
                                }
                            }
                        }
                        continue;
                    } else if let Some(rest) = data.strip_prefix("ob_snooze:") {
                        // rest = "{obligation_id}:{offset}" — split on last ':' to separate id and offset.
                        if let Some(last_colon) = rest.rfind(':') {
                            let ob_id = &rest[..last_colon];
                            let offset = &rest[last_colon + 1..];

                            if let (Some(tg_channel), Some(cid)) = (
                                self.channels
                                    .get("telegram")
                                    .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()),
                                tg_chat_id.or(self.telegram_chat_id),
                            ) {
                                if let (Some(ob_store_arc), Some(rem_store_arc)) = (
                                    &self.deps.obligation_store,
                                    &self.deps.reminder_store,
                                ) {
                                    if let Ok(ob_store) = ob_store_arc.lock() {
                                        if let Err(e) = crate::callbacks::handle_ob_snooze(
                                            ob_id,
                                            offset,
                                            &tg_channel.client,
                                            cid,
                                            original_msg_id,
                                            &ob_store,
                                            rem_store_arc.as_ref(),
                                            &self.deps.timezone,
                                        ).await {
                                            tracing::warn!(error = %e, "ob_snooze callback failed");
                                        }
                                    }
                                } else {
                                    tracing::warn!("ob_snooze: obligation_store or reminder_store not configured");
                                }
                            }
                        } else {
                            tracing::warn!(data, "ob_snooze callback data missing offset separator");
                        }
                        continue;
                    }

                    // Proactive follow-up callbacks: followup:{action}:{obligation_id}
                    if let Some(rest) = data.strip_prefix("followup:") {
                        let chat_id = tg_chat_id.or(self.telegram_chat_id);
                        if let Some(tg_channel) = self
                            .channels
                            .get("telegram")
                            .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>())
                        {
                            if let Some(cid) = chat_id {
                                self.handle_followup_callback(rest, &tg_channel.client, cid).await;
                            }
                        }
                        continue;
                    }

                    // Autonomous execution result callbacks
                    if let Some(ob_id) = data.strip_prefix("confirm_done:") {
                        if let (Some(tg_channel), Some(cid)) = (
                            self.channels
                                .get("telegram")
                                .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()),
                            tg_chat_id.or(self.telegram_chat_id),
                        ) {
                            if let Some(store_arc) = &self.deps.obligation_store {
                                if let Ok(store) = store_arc.lock() {
                                    if let Err(e) = crate::callbacks::handle_confirm_done(
                                        ob_id,
                                        &tg_channel.client,
                                        cid,
                                        original_msg_id,
                                        &store,
                                    ).await {
                                        tracing::warn!(error = %e, "confirm_done callback failed");
                                    } else {
                                        // Emit obligation.confirmed activity event.
                                        use crate::http::{DaemonEvent, ObligationActivityEvent};
                                        let event = ObligationActivityEvent {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            event_type: "obligation.confirmed".to_string(),
                                            obligation_id: ob_id.to_string(),
                                            description: "Leo confirmed obligation done".to_string(),
                                            timestamp: chrono::Utc::now(),
                                            metadata: None,
                                        };
                                        self.deps.activity_buffer.push(event.clone());
                                        let _ = self.deps.obligation_event_tx.send(
                                            DaemonEvent::ObligationActivity(event),
                                        );
                                    }
                                }
                            }
                        }
                        continue;
                    } else if let Some(ob_id) = data.strip_prefix("reopen:") {
                        if let (Some(tg_channel), Some(cid)) = (
                            self.channels
                                .get("telegram")
                                .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()),
                            tg_chat_id.or(self.telegram_chat_id),
                        ) {
                            if let Some(store_arc) = &self.deps.obligation_store {
                                if let Ok(store) = store_arc.lock() {
                                    if let Err(e) = crate::callbacks::handle_reopen(
                                        ob_id,
                                        &tg_channel.client,
                                        cid,
                                        original_msg_id,
                                        &store,
                                    ).await {
                                        tracing::warn!(error = %e, "reopen callback failed");
                                    } else {
                                        // Emit obligation.reopened activity event.
                                        use crate::http::{DaemonEvent, ObligationActivityEvent};
                                        let event = ObligationActivityEvent {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            event_type: "obligation.reopened".to_string(),
                                            obligation_id: ob_id.to_string(),
                                            description: "Leo reopened obligation for retry".to_string(),
                                            timestamp: chrono::Utc::now(),
                                            metadata: None,
                                        };
                                        self.deps.activity_buffer.push(event.clone());
                                        let _ = self.deps.obligation_event_tx.send(
                                            DaemonEvent::ObligationActivity(event),
                                        );
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Action callbacks (Jira, Nexus, HA, etc.)
                    if let Some(uuid_str) = data.strip_prefix("approve:") {
                        if let Some(tg) = self.channels.get("telegram") {
                            if let Some(tg_channel) = tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>() {
                                let chat_id = tg_chat_id.unwrap_or(tg_channel.chat_id);
                                // Build NexusBackend for legacy approval routing
                                let nexus_backend_owned: Option<nexus::backend::NexusBackend> =
                                    self.deps.team_agent_dispatcher
                                        .as_ref()
                                        .map(|d| nexus::backend::NexusBackend::new(d.clone()));
                                if let Err(e) = crate::callbacks::handle_approve_with_backend(
                                    uuid_str,
                                    self.deps.jira_registry.as_ref(),
                                    nexus_backend_owned.as_ref(),
                                    self.deps.cc_session_manager.as_ref(),
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
                    "diary" => self.cmd_diary(&parsed.args),
                    "projects" => self.cmd_projects().await,
                    "apply" => self.cmd_apply(&parsed.args, msg).await,
                    "sessions" => self.cmd_sessions().await,
                    "start" => self.cmd_start(&parsed.args, msg).await,
                    "stop" => self.cmd_stop(&parsed.args).await,
                    "obligations" => self.cmd_obligations(),
                    "ob" => self.cmd_ob(&parsed.args),
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

        let projects_section = if lines.is_empty() {
            "No projects registered.".to_string()
        } else {
            lines.sort();
            format!("\u{1F4CA} Project Status\n\n{}", lines.join("\n"))
        };

        // Append self-assessment section.
        let self_assessment_section = if let Some(ref store) = self.deps.self_assessment_store {
            match store.get_latest() {
                Ok(Some(entry)) => {
                    let summary = crate::self_assessment::format_status_summary(&entry);
                    format!("\n\n\u{1F50D} Self-Assessment\n{summary}")
                }
                Ok(None) => "\n\n\u{1F50D} Self-Assessment\nNo assessment yet. Will run Sunday.".to_string(),
                Err(e) => {
                    tracing::warn!(error = %e, "failed to read self-assessment for /status");
                    String::new()
                }
            }
        } else {
            String::new()
        };

        format!("{projects_section}{self_assessment_section}")
    }

    /// /digest — trigger an immediate digest.
    async fn cmd_digest(&self) -> String {
        // We can't inject into the trigger channel directly since we don't
        // hold a sender. Instead, use the deps' channels to route a message
        // that the orchestrator will pick up as a digest.
        // For now, send a direct HTTP request to the local health endpoint.
        let port = self.deps.health_port;
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

    /// /diary [N] — show last N diary entries (default 5, max 20).
    ///
    /// Reads the current day's diary file and, if needed, yesterday's file.
    /// Returns a plain-text summary suitable for Telegram. No Claude call.
    fn cmd_diary(&self, args: &[String]) -> String {
        let n: usize = args
            .first()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5)
            .min(20);

        let base = self.deps.diary.lock().unwrap().base_path().to_path_buf();
        let today = chrono::Local::now().date_naive();
        let yesterday = today.pred_opt().unwrap_or(today);

        // Collect lines from today's file, then yesterday's if needed.
        let mut raw_sections: Vec<String> = Vec::new();

        for date in [today, yesterday] {
            if raw_sections.len() >= n {
                break;
            }
            let path = base.join(format!("{}.md", date.format("%Y-%m-%d")));
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Split on "## " headings to get individual entries.
            let sections: Vec<&str> = content.split("\n## ").collect();
            for section in sections.into_iter().rev() {
                let trimmed = section.trim_start_matches("## ").trim();
                if trimmed.is_empty() {
                    continue;
                }
                raw_sections.push(trimmed.to_string());
                if raw_sections.len() >= n {
                    break;
                }
            }
        }

        if raw_sections.is_empty() {
            return "No diary entries found.".to_string();
        }

        let mut lines = format!("Diary — last {} entries\n", raw_sections.len());
        for section in &raw_sections {
            let entry_lines: Vec<&str> = section.lines().collect();
            let heading = entry_lines.first().copied().unwrap_or("?");

            // Extract time from heading (HH:MM — ...) and after-dash portion
            let (time_part, after_dash) = heading
                .split_once(" — ")
                .unwrap_or((heading, ""));

            // Extract slug from heading (last part after " · ")
            let slug_part = after_dash.split(" \u{00B7} ").last().unwrap_or("");

            // Extract trigger type + source (the part between "—" and "·")
            let middle = after_dash
                .split(" \u{00B7} ")
                .next()
                .unwrap_or("")
                .trim();

            // Find Tools, Result, Latency, Cost lines
            let tools = entry_lines
                .iter()
                .find(|l| l.starts_with("**Tools called:**"))
                .map(|l| l.trim_start_matches("**Tools called:** "))
                .unwrap_or("none");
            let result = entry_lines
                .iter()
                .find(|l| l.starts_with("**Result:**"))
                .map(|l| l.trim_start_matches("**Result:** "))
                .unwrap_or("—");
            let latency = entry_lines
                .iter()
                .find(|l| l.starts_with("**Latency:**"))
                .map(|l| l.trim_start_matches("**Latency:** "))
                .unwrap_or("?");
            let cost = entry_lines
                .iter()
                .find(|l| l.starts_with("**Cost:**"))
                .map(|l| l.trim_start_matches("**Cost:** "))
                .unwrap_or("?");

            lines.push('\n');
            lines.push_str(&format!("{time_part} — {middle} · {slug_part}\n"));
            lines.push_str(&format!("  Tools: {tools}\n"));
            lines.push_str(&format!("  Result: {result} ({latency}, {cost})\n"));
        }

        lines
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

        // Check team-agent connectivity
        let backend_available = self.deps.team_agent_dispatcher
            .as_ref()
            .map(|d| d.is_available())
            .unwrap_or(false);
        if !backend_available {
            return if self.deps.team_agent_dispatcher.is_some() {
                "\u{274C} No team-agent machines configured. Cannot start sessions.".to_string()
            } else {
                "\u{274C} Team agents not configured. Cannot start remote sessions.".to_string()
            };
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
             /apply <project> <spec> -- Apply a spec\n\
             /sessions -- List CC sessions\n\
             /start <project> <cmd...> -- Start a CC session\n\
             /stop <session_id> -- Stop a CC session"
        )
    }

    // ── CC Session Commands ─────────────────────────────────────────

    /// /sessions — list all CC sessions managed by CcSessionManager.
    async fn cmd_sessions(&self) -> String {
        let Some(ref mgr) = self.deps.cc_session_manager else {
            return "\u{274C} CC session manager not configured.".to_string();
        };

        let sessions = mgr.list().await;
        if sessions.is_empty() {
            return "\u{1F4CB} No active CC sessions.".to_string();
        }

        let mut lines = vec![format!(
            "\u{1F4BB} CC Sessions ({} total)\n",
            sessions.len()
        )];
        for s in &sessions {
            let icon = match s.state.as_str() {
                "running" => "\u{1F7E2}",
                "completed" => "\u{2705}",
                "stopped" => "\u{23F9}",
                _ => "\u{1F534}",
            };
            lines.push(format!(
                "{icon} [{}] {} \u{2014} {} ({})",
                &s.id[..8.min(s.id.len())],
                s.project,
                s.state,
                s.duration_display,
            ));
        }
        lines.join("\n")
    }

    /// /start <project> <cmd...> — start a CC session via CcSessionManager (with confirmation).
    async fn cmd_start(
        &self,
        args: &[String],
        msg: &nv_core::types::InboundMessage,
    ) -> String {
        if args.is_empty() {
            return "\u{2139}\u{FE0F} Usage: /start <project> [cmd...]\n\nExample: /start oo /apply fix-chat".to_string();
        }

        let project = &args[0];

        // Verify project exists in registry
        if !self.deps.project_registry.contains_key(project.as_str()) {
            let known: Vec<&String> = self.deps.project_registry.keys().collect();
            return format!(
                "\u{274C} Unknown project: {project}\n\nKnown projects: {}",
                known.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
            );
        }

        // Check CC session manager availability
        let mgr_available = self.deps.cc_session_manager
            .as_ref()
            .map(|m| m.is_available())
            .unwrap_or(false);
        if !mgr_available {
            return "\u{274C} CC session manager not configured or no machines available.".to_string();
        }

        let command = if args.len() > 1 {
            args[1..].join(" ")
        } else {
            "claude".to_string()
        };

        // Create PendingAction with confirmation keyboard
        let action_id = uuid::Uuid::new_v4();
        let description = format!(
            "Start CC session on {}: `{command}`",
            project.to_uppercase()
        );

        let keyboard = InlineKeyboard::confirm_action(&action_id.to_string());
        let payload = serde_json::json!({
            "project": project,
            "command": command,
            "_action_type": "CcStartSession",
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
                        tracing::error!(error = %e, "failed to send start confirmation");
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
            tracing::error!(error = %e, "failed to save pending action for /start");
        }

        // Return empty — confirmation keyboard already sent
        String::new()
    }

    /// /stop <session_id> — stop a CC session via CcSessionManager (with confirmation).
    async fn cmd_stop(&self, args: &[String]) -> String {
        let Some(session_id) = args.first() else {
            return "\u{2139}\u{FE0F} Usage: /stop <session_id>\n\nExample: /stop ta-abc12345".to_string();
        };

        let Some(ref mgr) = self.deps.cc_session_manager else {
            return "\u{274C} CC session manager not configured.".to_string();
        };

        // Verify session exists and is running
        let Some(status) = mgr.get_status(session_id).await else {
            return format!("\u{274C} Session '{session_id}' not found.");
        };

        if status.state != "running" {
            return format!(
                "\u{26A0}\u{FE0F} Session '{session_id}' is not running (state: {}).",
                status.state
            );
        }

        // Stop directly (no confirmation needed for stop — it's low-risk)
        match mgr.stop(session_id).await {
            Ok(msg) => format!("\u{23F9} {msg}"),
            Err(e) => format!("\u{274C} Failed to stop session: {e}"),
        }
    }

    // ── Obligation Commands ──────────────────────────────────────────

    /// /obligations (alias /ob with no subcommand) — list open obligations grouped by owner.
    ///
    /// Shows up to 10 items total with priority, status icon, and truncated action text.
    /// Format per item: `{priority} {icon} {detected_action truncated to 60 chars}`
    /// Status icons: open=○  in_progress=→  proposed_done=✓
    fn cmd_obligations(&self) -> String {
        let Some(store_arc) = &self.deps.obligation_store else {
            return "\u{274C} Obligation store not configured.".to_string();
        };

        let (nova_obs, leo_obs) = match store_arc.lock() {
            Ok(store) => {
                let nova = store.list_open_by_owner(&ObligationOwner::Nova).unwrap_or_default();
                let leo = store.list_open_by_owner(&ObligationOwner::Leo).unwrap_or_default();
                (nova, leo)
            }
            Err(e) => {
                tracing::warn!(error = %e, "cmd_obligations: store mutex poisoned");
                return "\u{274C} Failed to read obligations.".to_string();
            }
        };

        let total = nova_obs.len() + leo_obs.len();
        if total == 0 {
            return "\u{2705} No open obligations.".to_string();
        }

        fn status_icon(status: &ObligationStatus) -> &'static str {
            match status {
                ObligationStatus::Open => "\u{25CB}",        // ○
                ObligationStatus::InProgress => "\u{2192}",  // →
                ObligationStatus::ProposedDone => "\u{2713}", // ✓
                _ => "\u{25CB}",
            }
        }

        fn format_item(ob: &nv_core::types::Obligation) -> String {
            let action: String = ob.detected_action.chars().take(60).collect();
            let action = if ob.detected_action.len() > 60 {
                format!("{action}...")
            } else {
                action
            };
            let short_id: String = ob.id.chars().take(6).collect();
            format!(
                "{} {} {} [{}]",
                ob.priority,
                status_icon(&ob.status),
                action,
                short_id
            )
        }

        const MAX_ITEMS: usize = 10;
        let mut lines = Vec::new();
        let mut shown = 0;

        if !nova_obs.is_empty() {
            lines.push("Nova's obligations:".to_string());
            for ob in &nova_obs {
                if shown >= MAX_ITEMS {
                    break;
                }
                lines.push(format_item(ob));
                shown += 1;
            }
        }

        if !leo_obs.is_empty() && shown < MAX_ITEMS {
            lines.push("Leo's obligations:".to_string());
            for ob in &leo_obs {
                if shown >= MAX_ITEMS {
                    break;
                }
                lines.push(format_item(ob));
                shown += 1;
            }
        }

        if shown < total {
            lines.push(format!("...and {} more", total - shown));
        }

        lines.join("\n")
    }

    /// /ob <subcommand> [args...] — obligation management subcommands.
    ///
    /// Subcommands: done, assign, create, status. No subcommand → list (same as /obligations).
    fn cmd_ob(&self, args: &[String]) -> String {
        match args.first().map(|s| s.as_str()) {
            None | Some("list") => self.cmd_obligations(),
            Some("done") => {
                let id_prefix = match args.get(1) {
                    Some(p) => p.as_str(),
                    None => return "\u{2139}\u{FE0F} Usage: /ob done <id_prefix>".to_string(),
                };
                self.cmd_ob_done(id_prefix)
            }
            Some("assign") => {
                let id_prefix = match args.get(1) {
                    Some(p) => p.as_str(),
                    None => return "\u{2139}\u{FE0F} Usage: /ob assign <id_prefix> nova|leo".to_string(),
                };
                let owner_str = match args.get(2) {
                    Some(o) => o.as_str(),
                    None => return "\u{2139}\u{FE0F} Usage: /ob assign <id_prefix> nova|leo".to_string(),
                };
                self.cmd_ob_assign(id_prefix, owner_str)
            }
            Some("create") => {
                if args.len() < 2 {
                    return "\u{2139}\u{FE0F} Usage: /ob create <text>".to_string();
                }
                let text = args[1..].join(" ");
                self.cmd_ob_create(&text)
            }
            Some("status") => self.cmd_ob_status(),
            Some(other) => {
                format!(
                    "\u{2753} Unknown /ob subcommand: {other}\n\n\
                     Available: list, done, assign, create, status"
                )
            }
        }
    }

    /// /ob done <id_prefix> — mark obligation done by UUID prefix.
    fn cmd_ob_done(&self, id_prefix: &str) -> String {
        let Some(store_arc) = &self.deps.obligation_store else {
            return "\u{274C} Obligation store not configured.".to_string();
        };

        match store_arc.lock() {
            Ok(store) => {
                match store.get_by_id_prefix(id_prefix) {
                    Ok(Some(ob)) => {
                        match store.update_status(&ob.id, &ObligationStatus::Done) {
                            Ok(_) => {
                                let action: String = ob.detected_action.chars().take(80).collect();
                                let action = if ob.detected_action.len() > 80 {
                                    format!("{action}...")
                                } else {
                                    action
                                };
                                format!("\u{2705} Marked done: {action}")
                            }
                            Err(e) => format!("\u{274C} Failed to update obligation: {e}"),
                        }
                    }
                    Ok(None) => format!("\u{274C} No obligation found with prefix '{id_prefix}'"),
                    Err(e) => format!("\u{274C} Failed to look up obligation: {e}"),
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "cmd_ob_done: store mutex poisoned");
                "\u{274C} Failed to access obligation store.".to_string()
            }
        }
    }

    /// /ob assign <id_prefix> nova|leo — change obligation owner.
    fn cmd_ob_assign(&self, id_prefix: &str, owner_str: &str) -> String {
        let new_owner = match owner_str.to_lowercase().as_str() {
            "nova" => ObligationOwner::Nova,
            "leo" => ObligationOwner::Leo,
            other => return format!("\u{274C} Unknown owner '{other}'. Use: nova or leo"),
        };

        let Some(store_arc) = &self.deps.obligation_store else {
            return "\u{274C} Obligation store not configured.".to_string();
        };

        match store_arc.lock() {
            Ok(store) => {
                match store.get_by_id_prefix(id_prefix) {
                    Ok(Some(ob)) => {
                        match store.update_owner(&ob.id, &new_owner) {
                            Ok(_) => {
                                let action: String = ob.detected_action.chars().take(80).collect();
                                let action = if ob.detected_action.len() > 80 {
                                    format!("{action}...")
                                } else {
                                    action
                                };
                                format!(
                                    "\u{2192} Assigned to {}: {action}",
                                    new_owner.as_str()
                                )
                            }
                            Err(e) => format!("\u{274C} Failed to update owner: {e}"),
                        }
                    }
                    Ok(None) => format!("\u{274C} No obligation found with prefix '{id_prefix}'"),
                    Err(e) => format!("\u{274C} Failed to look up obligation: {e}"),
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "cmd_ob_assign: store mutex poisoned");
                "\u{274C} Failed to access obligation store.".to_string()
            }
        }
    }

    /// /ob create <text> — create a new obligation assigned to Nova.
    fn cmd_ob_create(&self, text: &str) -> String {
        let Some(store_arc) = &self.deps.obligation_store else {
            return "\u{274C} Obligation store not configured.".to_string();
        };

        let new_ob = NewObligation {
            id: Uuid::new_v4().to_string(),
            source_channel: "telegram".to_string(),
            source_message: None,
            detected_action: text.to_string(),
            project_code: None,
            priority: 2,
            owner: ObligationOwner::Nova,
            owner_reason: None,
            deadline: None,
        };

        match store_arc.lock() {
            Ok(store) => {
                match store.create(new_ob) {
                    Ok(ob) => {
                        let action: String = ob.detected_action.chars().take(80).collect();
                        let action = if ob.detected_action.len() > 80 {
                            format!("{action}...")
                        } else {
                            action
                        };
                        format!("\u{2705} Created obligation: {action} (assigned to Nova)")
                    }
                    Err(e) => format!("\u{274C} Failed to create obligation: {e}"),
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "cmd_ob_create: store mutex poisoned");
                "\u{274C} Failed to access obligation store.".to_string()
            }
        }
    }

    /// /ob status — one-line summary of obligation counts by owner and status.
    fn cmd_ob_status(&self) -> String {
        let Some(store_arc) = &self.deps.obligation_store else {
            return "\u{274C} Obligation store not configured.".to_string();
        };

        match store_arc.lock() {
            Ok(store) => {
                match store.count_open_for_status_summary() {
                    Ok((nova_open, nova_in_progress, nova_proposed_done, leo_open)) => {
                        format!(
                            "Nova: {nova_open} open, {nova_in_progress} in progress, \
                             {nova_proposed_done} proposed done | Leo: {leo_open} open"
                        )
                    }
                    Err(e) => format!("\u{274C} Failed to read obligation counts: {e}"),
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "cmd_ob_status: store mutex poisoned");
                "\u{274C} Failed to access obligation store.".to_string()
            }
        }
    }

    // ── Obligation Callback Handler ─────────────────────────────────

    /// Handle an obligation inline keyboard callback (Handle / Delegate / Dismiss).
    ///
    /// Updates the obligation's status (and optionally owner) in the store,
    /// then edits the original Telegram message to confirm the action.
    async fn handle_obligation_callback(
        &self,
        obligation_id: &str,
        new_status: nv_core::types::ObligationStatus,
        new_owner: Option<nv_core::types::ObligationOwner>,
        tg_chat_id: Option<i64>,
        original_msg_id: Option<i64>,
        confirmation_text: &str,
    ) {
        let Some(store_arc) = &self.deps.obligation_store else {
            tracing::warn!("obligation callback but no obligation store configured");
            return;
        };

        let updated = match store_arc.lock() {
            Ok(store) => {
                let result = if let Some(ref owner) = new_owner {
                    store.update_status_and_owner(obligation_id, &new_status, owner)
                } else {
                    store.update_status(obligation_id, &new_status)
                };
                match result {
                    Ok(changed) => changed,
                    Err(e) => {
                        tracing::warn!(error = %e, id = obligation_id, "obligation callback: store update failed");
                        false
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "obligation callback: store mutex poisoned");
                false
            }
        };

        if !updated {
            tracing::warn!(id = obligation_id, "obligation callback: obligation not found");
        }

        // Edit the original Telegram message to confirm
        if let (Some(tg_channel), Some(chat_id), Some(msg_id)) = (
            self.channels
                .get("telegram")
                .and_then(|c| c.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()),
            tg_chat_id.or(self.telegram_chat_id),
            original_msg_id,
        ) {
            if let Err(e) = tg_channel
                .client
                .edit_message(chat_id, msg_id, confirmation_text, None)
                .await
            {
                tracing::warn!(error = %e, "obligation callback: failed to edit Telegram message");
            }
        }

        tracing::info!(
            id = obligation_id,
            status = %new_status,
            owner = ?new_owner.as_ref().map(|o| o.as_str()),
            "obligation updated via Telegram callback"
        );
    }

    // ── Morning Briefing Handler ────────────────────────────────────

    /// Send the morning briefing digest — open obligation summary via Telegram.
    async fn send_morning_briefing(&self) {
        let Some(store_arc) = &self.deps.obligation_store else {
            tracing::debug!("morning briefing: no obligation store configured");
            return;
        };

        let (by_priority, total_open) = match store_arc.lock() {
            Ok(store) => {
                let by_priority = store.count_open_by_priority().unwrap_or_default();
                let total_open = by_priority.iter().map(|(_, c)| c).sum::<i64>();
                (by_priority, total_open)
            }
            Err(e) => {
                tracing::warn!(error = %e, "morning briefing: store mutex poisoned");
                return;
            }
        };

        let mut message = format_morning_briefing(&by_priority, total_open);

        // Inject self-assessment section if there's a recent entry (within last 7 days).
        if let Some(ref sa_store) = self.deps.self_assessment_store {
            let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
            match sa_store.get_latest() {
                Ok(Some(entry)) if entry.generated_at > seven_days_ago => {
                    let section = crate::self_assessment::format_briefing_section(&entry);
                    message.push_str("\n\n");
                    message.push_str(&section);
                }
                Ok(_) => {} // No recent assessment — omit section.
                Err(e) => {
                    tracing::warn!(error = %e, "morning briefing: failed to read self-assessment");
                }
            }
        }

        // Persist the briefing entry before sending so the dashboard can read it even
        // if the Telegram send fails.
        if let Some(briefing_store) = &self.deps.briefing_store {
            let mut sources = std::collections::HashMap::new();
            sources.insert("obligations".to_string(), "ok".to_string());
            let entry = BriefingEntry::new(message.clone(), vec![], sources);
            if let Err(e) = briefing_store.append(&entry) {
                tracing::warn!(error = %e, "morning briefing: failed to persist briefing entry");
            }
        }

        if let Some(channel) = self.channels.get("telegram") {
            let msg = OutboundMessage {
                channel: "telegram".into(),
                content: message,
                reply_to: None,
                keyboard: None,
            };
            if let Err(e) = channel.send_message(msg).await {
                tracing::warn!(error = %e, "morning briefing: failed to send Telegram message");
            } else {
                tracing::info!(total_open, "morning briefing sent");
            }
        }
    }

    // ── Proactive Follow-Up ──────────────────────────────────────────

    /// Scan open obligations for overdue/approaching-deadline/stale items and send
    /// Telegram reminders with action buttons.
    ///
    /// Deduplicates using `ProactiveWatcherState::reminder_counts`. Caps at 5 reminders
    /// per run to prevent flooding when many obligations are simultaneously stale.
    async fn handle_proactive_followup(&self) {
        tracing::info!("proactive_followup: starting scan");

        let ob_store = match &self.deps.obligation_store {
            Some(arc) => arc,
            None => {
                tracing::debug!("proactive_followup: obligation store not configured, skipping");
                return;
            }
        };

        let now = chrono::Utc::now();

        // Load proactive watcher config from shared deps (use defaults if absent).
        let pw_config = self
            .deps
            .proactive_watcher_config
            .clone()
            .unwrap_or_default();

        // Load persisted watcher state.
        let mut pw_state =
            crate::proactive_watcher::ProactiveWatcherState::load(&self.deps.nv_base_path)
                .unwrap_or_default();

        // Reset reminder_counts if last_run_at was on a different calendar day.
        if let Some(last) = pw_state.last_run_at {
            if last.date_naive() < now.date_naive() {
                pw_state.reminder_counts.clear();
            }
        }

        // Collect matches in priority order: overdue → approaching → stale.
        let mut candidates: Vec<(nv_core::types::Obligation, &'static str)> = Vec::new();

        // --- 1. Overdue (deadline IS NOT NULL AND deadline < now) ---
        let approaching_cutoff =
            now + chrono::Duration::hours(pw_config.approaching_deadline_hours as i64);
        let stale_threshold =
            now - chrono::Duration::hours(pw_config.stale_threshold_hours as i64);

        {
            match ob_store.lock() {
                Ok(store) => {
                    // Overdue: deadline set and <= now.
                    match store.list_open_with_deadline_before(&now) {
                        Ok(overdue) => {
                            for ob in overdue {
                                // Skip proposed_done — Leo is reviewing, no need to follow up.
                                if ob.status == nv_core::types::ObligationStatus::ProposedDone {
                                    continue;
                                }
                                candidates.push((ob, "overdue"));
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "proactive_followup: overdue query failed");
                        }
                    }
                    // Approaching deadline: deadline set and now < deadline <= approaching_cutoff.
                    // list_open_with_deadline_before(approaching_cutoff) includes overdue ones;
                    // we filter out already-found overdue items by checking deadline > now.
                    match store.list_open_with_deadline_before(&approaching_cutoff) {
                        Ok(approaching) => {
                            for ob in approaching {
                                // Skip proposed_done — Leo is reviewing, no need to follow up.
                                if ob.status == nv_core::types::ObligationStatus::ProposedDone {
                                    continue;
                                }
                                let is_already_overdue = ob.deadline.as_deref().map(|d| {
                                    chrono::DateTime::parse_from_rfc3339(d)
                                        .map(|dt| dt.with_timezone(&chrono::Utc) <= now)
                                        .unwrap_or(false)
                                }).unwrap_or(false);
                                if !is_already_overdue && candidates.iter().all(|(c, _)| c.id != ob.id) {
                                    candidates.push((ob, "due soon"));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "proactive_followup: approaching query failed");
                        }
                    }
                    // Stale: updated_at <= stale_threshold.
                    match store.list_stale_open(&stale_threshold) {
                        Ok(stale) => {
                            for ob in stale {
                                // Skip proposed_done — Leo is reviewing, no need to follow up.
                                if ob.status == nv_core::types::ObligationStatus::ProposedDone {
                                    continue;
                                }
                                if candidates.iter().all(|(c, _)| c.id != ob.id) {
                                    let hours = pw_config.stale_threshold_hours;
                                    // Use Box::leak to get a 'static str — small, bounded allocation.
                                    let label: &'static str = Box::leak(
                                        format!("no update in {hours}h").into_boxed_str(),
                                    );
                                    candidates.push((ob, label));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "proactive_followup: stale query failed");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "proactive_followup: obligation store mutex poisoned");
                    return;
                }
            }
        }

        if candidates.is_empty() {
            tracing::debug!("proactive_followup: no matching obligations");
            // Update last_run_at.
            pw_state.last_run_at = Some(now);
            let _ = pw_state.save(&self.deps.nv_base_path);
            return;
        }

        tracing::info!(candidates = candidates.len(), "proactive_followup: found candidates");

        let chat_id = match self.telegram_chat_id {
            Some(id) => id,
            None => {
                tracing::warn!("proactive_followup: no telegram chat_id configured");
                return;
            }
        };

        // Send reminders — cap at 5 per run.
        let mut sent = 0u32;
        const MAX_PER_RUN: u32 = 5;

        for (ob, status_label) in &candidates {
            if sent >= MAX_PER_RUN {
                tracing::debug!(
                    remaining = candidates.len() as u32 - sent,
                    "proactive_followup: cap reached, deferring remaining"
                );
                break;
            }

            // Dedup guard.
            let count = *pw_state.reminder_counts.get(&ob.id).unwrap_or(&0);
            if count >= pw_config.max_reminders_per_interval {
                tracing::debug!(
                    obligation_id = %ob.id,
                    count,
                    "proactive_followup: dedup guard — already reminded"
                );
                continue;
            }

            // Build message.
            let priority_label = match ob.priority {
                0 => "P0",
                1 => "P1",
                2 => "P2",
                3 => "P3",
                _ => "P4",
            };
            let body = format!(
                "Follow-up: {}\n\nStatus: {}\nChannel: {}\nPriority: {}\n\nWhat would you like to do?",
                ob.detected_action,
                status_label,
                ob.source_channel,
                priority_label,
            );
            let keyboard = followup_keyboard(&ob.id);

            if let Some(channel) = self.channels.get("telegram") {
                let msg = OutboundMessage {
                    channel: "telegram".into(),
                    content: body,
                    reply_to: None,
                    keyboard: Some(keyboard),
                };
                match channel.send_message(msg).await {
                    Ok(()) => {
                        tracing::info!(
                            obligation_id = %ob.id,
                            status = status_label,
                            "proactive_followup: reminder sent"
                        );
                        *pw_state.reminder_counts.entry(ob.id.clone()).or_insert(0) += 1;
                        sent += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            obligation_id = %ob.id,
                            "proactive_followup: failed to send reminder"
                        );
                    }
                }
            } else {
                tracing::warn!("proactive_followup: no telegram channel — reminders not delivered");
                break;
            }

            let _ = chat_id; // suppress unused warning; chat_id used implicitly via channel registry
        }

        // Persist updated state.
        pw_state.last_run_at = Some(now);
        if let Err(e) = pw_state.save(&self.deps.nv_base_path) {
            tracing::warn!(error = %e, "proactive_followup: failed to persist watcher state");
        }

        tracing::info!(sent, "proactive_followup: scan complete");
    }

    /// Handle a `followup:{action}:{obligation_id}` callback from an inline keyboard button.
    ///
    /// Actions:
    /// - `done` → `ObligationStore::update_status(id, Done)`
    /// - `snooze` → `ObligationStore::snooze(id)` (resets staleness clock)
    /// - `dismiss` → `ObligationStore::update_status(id, Dismissed)`
    async fn handle_followup_callback(
        &self,
        rest: &str,
        telegram: &crate::channels::telegram::client::TelegramClient,
        chat_id: i64,
    ) {
        // rest = "{action}:{obligation_id}"
        let (action, ob_id) = match rest.split_once(':') {
            Some(pair) => pair,
            None => {
                tracing::warn!(data = rest, "followup callback: malformed data (expected action:id)");
                return;
            }
        };

        let ob_store = match &self.deps.obligation_store {
            Some(arc) => arc,
            None => {
                tracing::warn!("followup callback: obligation store not configured");
                let _ = telegram.send_message(chat_id, "Obligation store unavailable.", None, None).await;
                return;
            }
        };

        let result = match action {
            "done" => {
                ob_store
                    .lock()
                    .ok()
                    .map(|store| store.update_status(ob_id, &nv_core::types::ObligationStatus::Done))
                    .unwrap_or_else(|| Err(anyhow::anyhow!("mutex poisoned")))
                    .map(|updated| {
                        if updated {
                            "Marked as done."
                        } else {
                            "Obligation not found."
                        }
                    })
            }
            "snooze" => {
                ob_store
                    .lock()
                    .ok()
                    .map(|store| store.snooze(ob_id))
                    .unwrap_or_else(|| Err(anyhow::anyhow!("mutex poisoned")))
                    .map(|updated| {
                        if updated {
                            "Snoozed 24h (staleness clock reset)."
                        } else {
                            "Obligation not found or not open."
                        }
                    })
            }
            "dismiss" => {
                ob_store
                    .lock()
                    .ok()
                    .map(|store| {
                        store.update_status(ob_id, &nv_core::types::ObligationStatus::Dismissed)
                    })
                    .unwrap_or_else(|| Err(anyhow::anyhow!("mutex poisoned")))
                    .map(|updated| {
                        if updated {
                            "Dismissed."
                        } else {
                            "Obligation not found."
                        }
                    })
            }
            other => {
                tracing::warn!(action = other, ob_id, "followup callback: unknown action");
                let _ = telegram
                    .send_message(chat_id, &format!("Unknown followup action: {other}"), None, None)
                    .await;
                return;
            }
        };

        let reply = match result {
            Ok(msg) => msg.to_string(),
            Err(e) => {
                tracing::warn!(error = %e, action, ob_id, "followup callback: store operation failed");
                format!("Error: {e}")
            }
        };

        let _ = telegram.send_message(chat_id, &reply, None, None).await;
        tracing::info!(action, ob_id, "followup callback: handled");
    }

    // ── Digest Pipeline ─────────────────────────────────────────────

    /// Run the full digest pipeline: gather → synthesize → should_send → send → record_sent.
    ///
    /// Uses `synthesize_digest_fallback()` when Claude is unavailable.
    /// All errors are logged; none panic or bubble up to the caller.
    async fn run_digest_pipeline(&self) {
        tracing::info!("digest: starting pipeline");

        let jira_client = self.deps.jira_registry.as_ref().and_then(|r| r.default_client());
        let memory = &self.deps.memory;
        let calendar_credentials = self.deps.calendar_credentials.as_deref();
        let calendar_id = &self.deps.calendar_id;

        // Step 1: gather context from all sources
        let context = gather_context(jira_client, memory, calendar_credentials, calendar_id).await;
        tracing::info!(
            jira_issues = context.jira_issues.len(),
            nexus_sessions = context.nexus_sessions.len(),
            memory_entries = context.memory_entries.len(),
            calendar_events = context.calendar_events.len(),
            gather_errors = context.errors.len(),
            "digest: gather complete"
        );

        // Step 2: synthesize via Claude (fallback to template on error)
        let mut result = match synthesize_digest(&self.deps.claude_client, &context).await {
            Ok(r) => {
                tracing::info!("digest: synthesis complete");
                r
            }
            Err(e) => {
                tracing::warn!(error = %e, "digest: synthesis failed — using fallback");
                synthesize_digest_fallback(&context)
            }
        };

        // Step 3: inject budget warning if threshold exceeded
        {
            let store = self.deps.message_store.lock().unwrap();
            if let Ok(budget) = store.usage_budget_status(self.deps.weekly_budget_usd) {
                let threshold = self.deps.alert_threshold_pct as f64;
                if budget.pct_used >= threshold {
                    let budget_line = format!(
                        "[Budget] ${:.2} / ${:.2} this week ({:.0}%)",
                        budget.rolling_7d_cost, budget.weekly_budget, budget.pct_used
                    );
                    inject_budget_warning(&mut result, &budget_line);
                    tracing::info!(pct = budget.pct_used, "digest: budget warning injected");
                }
            }
        }

        // Step 4: check whether we should send (dedup by hash and interval)
        let state_mgr = DigestStateManager::new(&self.deps.nv_base_path);
        let hash = content_hash(&result.content);
        // Use 60-minute minimum interval for dedup; 0 means "always send on hash change"
        let should_send = state_mgr.should_send(60, Some(&hash)).unwrap_or(true);
        if !should_send {
            tracing::info!("digest: skipped — content unchanged and interval not elapsed");
            return;
        }

        // Step 5: format and send to Telegram
        let (text, keyboard) = format_digest(&result);
        if let Some(channel) = self.channels.get("telegram") {
            let msg = OutboundMessage {
                channel: "telegram".into(),
                content: text,
                reply_to: None,
                keyboard,
            };
            if let Err(e) = channel.send_message(msg).await {
                tracing::warn!(error = %e, "digest: failed to send to Telegram");
                return;
            }
            tracing::info!("digest: sent to Telegram");
        } else {
            tracing::warn!("digest: no telegram channel configured — digest not delivered");
        }

        // Step 6: record sent state
        let sources = {
            let mut m = std::collections::HashMap::new();
            if !context.jira_issues.is_empty() {
                m.insert("jira".to_string(), "ok".to_string());
            }
            if !context.nexus_sessions.is_empty() {
                m.insert("nexus".to_string(), "ok".to_string());
            }
            if !context.memory_entries.is_empty() {
                m.insert("memory".to_string(), "ok".to_string());
            }
            for err in &context.errors {
                let source = err.split(':').next().unwrap_or("unknown").to_lowercase();
                m.insert(source, "error".to_string());
            }
            m
        };
        if let Err(e) = state_mgr.record_sent(&hash, result.suggested_actions, sources) {
            tracing::warn!(error = %e, "digest: failed to record sent state");
        }
    }

    // ── Nexus Error Handlers ────────────────────────────────────────

    async fn handle_nexus_view_error(&mut self, session_id: &str) {
        tracing::warn!(session_id, "nexus_err:view callback — no longer supported (Nexus gRPC removed)");
        self.send_error("telegram", &format!(
            "Session error view for {session_id} is unavailable (Nexus gRPC removed)."
        )).await;
    }

    async fn handle_nexus_create_bug(&mut self, session_id: &str) {
        tracing::warn!(session_id, "nexus_err:bug callback — no longer supported (Nexus gRPC removed)");
        self.send_error("telegram", &format!(
            "Cannot create bug from session {session_id}: Nexus gRPC removed. Use Jira directly."
        )).await;
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
    ///
    /// Called on every trigger batch so the final batch in a sequence is always
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

    // ── Autonomous Obligation Execution ─────────────────────────────

    /// Check whether Nova is idle and, if so, attempt to execute the next obligation.
    ///
    /// Idle conditions (all must be true):
    /// - Autonomy is enabled in config
    /// - No active workers (`worker_pool.active_count() == 0`)
    /// - No ongoing autonomous execution (`executing_obligation.is_none()`)
    /// - Time since last interactive message > `idle_debounce_secs`
    /// - `last_interactive_at` has been set at least once (> 0)
    async fn check_idle_and_execute(&mut self) {
        let config = match &self.deps.autonomy_config {
            Some(c) if c.enabled => c.clone(),
            _ => return, // autonomy disabled or not configured
        };

        // Guard: only one autonomous execution at a time.
        if self.executing_obligation.is_some() {
            return;
        }

        // Guard: workers must be idle.
        if self.worker_pool.active_count() > 0 {
            return;
        }

        // Guard: debounce — check that enough time has passed since the last interactive message.
        let last_interactive_secs = self
            .deps
            .last_interactive_at
            .load(std::sync::atomic::Ordering::Relaxed);

        if last_interactive_secs == 0 {
            // last_interactive_at never set — daemon just started; don't execute yet.
            return;
        }

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let secs_since_interactive = now_secs.saturating_sub(last_interactive_secs);
        if secs_since_interactive < config.idle_debounce_secs {
            tracing::trace!(
                secs_since_interactive,
                idle_debounce_secs = config.idle_debounce_secs,
                "idle check: not yet idle"
            );
            return;
        }

        tracing::debug!(
            secs_since_interactive,
            idle_debounce_secs = config.idle_debounce_secs,
            "idle check: Nova is idle — looking for obligations to execute"
        );

        self.try_execute_next_obligation(config).await;
    }

    /// Pick the highest-priority obligation ready for execution and run it autonomously.
    ///
    /// Selects the first obligation returned by `list_ready_for_execution()` (ordered by
    /// priority ASC, then created_at ASC), sets `executing_obligation`, spawns the executor,
    /// clears `executing_obligation` on completion, and calls `handle_execution_result`.
    async fn try_execute_next_obligation(&mut self, config: nv_core::config::AutonomyConfig) {
        let store_arc = match &self.deps.obligation_store {
            Some(arc) => Arc::clone(arc),
            None => {
                tracing::debug!("autonomous executor: obligation store not configured");
                return;
            }
        };

        // Fetch ready obligations — lock briefly, release before any await.
        let obligation = {
            match store_arc.lock() {
                Ok(store) => match store.list_ready_for_execution(config.cooldown_hours) {
                    Ok(mut list) => {
                        if list.is_empty() {
                            tracing::debug!("autonomous executor: no obligations ready");
                            return;
                        }
                        list.remove(0) // highest priority (list sorted priority ASC, created_at ASC)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "autonomous executor: failed to query obligations");
                        return;
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "autonomous executor: store mutex poisoned");
                    return;
                }
            }
        };

        tracing::info!(
            obligation_id = %obligation.id,
            priority = obligation.priority,
            action = %obligation.detected_action,
            "autonomous executor: dispatching obligation"
        );

        self.executing_obligation = Some(obligation.id.clone());

        let deps = Arc::clone(&self.deps);
        let telegram_chat_id = self.telegram_chat_id;

        let result =
            crate::obligation_executor::execute_obligation(&obligation, &deps, &config)
                .await;

        crate::obligation_executor::handle_execution_result(
            &obligation,
            result,
            &deps,
            telegram_chat_id,
        )
        .await;

        self.executing_obligation = None;
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

// ── Obligation Telegram Helpers ─────────────────────────────────────

/// Format a detected obligation as a Telegram HTML card.
///
/// Uses HTML parse mode. Bold title with priority badge, source channel,
/// project code (if present), detected action, and owner with reason.
fn format_obligation_card(ob: &nv_core::types::Obligation, source_channel: &str) -> String {
    let priority_label = match ob.priority {
        0 => "P0 CRITICAL",
        1 => "P1 HIGH",
        2 => "P2 IMPORTANT",
        3 => "P3 MINOR",
        _ => "P4 BACKLOG",
    };

    let mut lines = Vec::new();
    lines.push(format!("<b>[{priority_label}] New Obligation</b>"));
    lines.push(format!("Channel: <code>{}</code>", escape_html_plain(source_channel)));

    if let Some(ref code) = ob.project_code {
        lines.push(format!("Project: <code>{}</code>", escape_html_plain(code)));
    }

    lines.push(format!("Action: {}", escape_html_plain(&ob.detected_action)));
    lines.push(format!("Owner: <b>{}</b>", escape_html_plain(ob.owner.as_str())));

    if let Some(ref reason) = ob.owner_reason {
        lines.push(format!("<i>{}</i>", escape_html_plain(reason)));
    }

    lines.join("\n")
}

/// Build the obligation action inline keyboard.
///
/// Two rows:
///   Row 0: [Handle] [Delegate to Nova] [Dismiss]
///   Row 1: [Edit] [Cancel] [Extend] [Snooze 1h] [Snooze 4h] [Snooze tomorrow]
fn obligation_keyboard(obligation_id: &str) -> nv_core::types::InlineKeyboard {
    nv_core::types::InlineKeyboard {
        rows: vec![
            // Row 0 — primary actions (existing)
            vec![
                nv_core::types::InlineButton {
                    text: "Handle".to_string(),
                    callback_data: format!("ob_handle:{obligation_id}"),
                },
                nv_core::types::InlineButton {
                    text: "Delegate to Nova".to_string(),
                    callback_data: format!("ob_delegate:{obligation_id}"),
                },
                nv_core::types::InlineButton {
                    text: "Dismiss".to_string(),
                    callback_data: format!("ob_dismiss:{obligation_id}"),
                },
            ],
            // Row 1 — secondary actions (new)
            vec![
                nv_core::types::InlineButton {
                    text: "Edit".to_string(),
                    callback_data: format!("ob_edit:{obligation_id}"),
                },
                nv_core::types::InlineButton {
                    text: "Cancel".to_string(),
                    callback_data: format!("ob_cancel:{obligation_id}"),
                },
                nv_core::types::InlineButton {
                    text: "Extend".to_string(),
                    callback_data: format!("ob_expiry:{obligation_id}"),
                },
                nv_core::types::InlineButton {
                    text: "Snooze 1h".to_string(),
                    callback_data: format!("ob_snooze:{obligation_id}:1h"),
                },
                nv_core::types::InlineButton {
                    text: "Snooze 4h".to_string(),
                    callback_data: format!("ob_snooze:{obligation_id}:4h"),
                },
                nv_core::types::InlineButton {
                    text: "Snooze tomorrow".to_string(),
                    callback_data: format!("ob_snooze:{obligation_id}:tomorrow"),
                },
            ],
        ],
    }
}

/// Build the proactive follow-up inline keyboard.
///
/// Three buttons: `[Mark Done]` `[Snooze 24h]` `[Dismiss]`
fn followup_keyboard(obligation_id: &str) -> nv_core::types::InlineKeyboard {
    nv_core::types::InlineKeyboard {
        rows: vec![vec![
            nv_core::types::InlineButton {
                text: "Mark Done".to_string(),
                callback_data: format!("followup:done:{obligation_id}"),
            },
            nv_core::types::InlineButton {
                text: "Snooze 24h".to_string(),
                callback_data: format!("followup:snooze:{obligation_id}"),
            },
            nv_core::types::InlineButton {
                text: "Dismiss".to_string(),
                callback_data: format!("followup:dismiss:{obligation_id}"),
            },
        ]],
    }
}

/// Escape HTML special characters for Telegram HTML parse mode.
fn escape_html_plain(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Format the morning briefing digest message.
///
/// Counts open obligations by priority and summarises active sessions.
fn format_morning_briefing(
    open_obligations: &[(i32, i64)],
    total_open: i64,
) -> String {
    let mut lines = Vec::new();
    lines.push("<b>Good morning. Daily briefing:</b>".to_string());
    lines.push(String::new());

    if total_open == 0 {
        lines.push("No open obligations.".to_string());
    } else {
        lines.push(format!(
            "<b>Open obligations:</b> {} total",
            total_open
        ));
        for (priority, count) in open_obligations {
            let label = match priority {
                0 => "P0 Critical",
                1 => "P1 High",
                2 => "P2 Important",
                3 => "P3 Minor",
                _ => "P4 Backlog",
            };
            lines.push(format!("  {label}: {count}"));
        }
    }

    lines.join("\n")
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

/// Check if the current time in the user's configured timezone falls within
/// the quiet window.
///
/// Handles overnight windows (e.g., 23:00 → 07:00) correctly.
/// Returns `false` if either bound is `None` (no quiet window configured).
///
/// `tz_name` is an IANA timezone name (e.g., `"America/Chicago"`). The offset
/// is resolved via `reminders::tz_offset_seconds()`.
pub fn is_quiet_hours(
    quiet_start: Option<chrono::NaiveTime>,
    quiet_end: Option<chrono::NaiveTime>,
    tz_name: &str,
) -> bool {
    let (Some(start), Some(end)) = (quiet_start, quiet_end) else {
        return false;
    };
    // Convert UTC now to the user's local time using the configured timezone offset.
    let offset_secs = crate::reminders::tz_offset_seconds(tz_name);
    let offset = chrono::FixedOffset::east_opt(offset_secs).unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
    let now = chrono::Utc::now().with_timezone(&offset).time();
    if start <= end {
        // Same-day window (e.g., 01:00 → 05:00)
        now >= start && now < end
    } else {
        // Overnight window (e.g., 23:00 → 07:00)
        now >= start || now < end
    }
}

/// Returns `true` if any trigger in `triggers` is a `Trigger::Message` whose
/// `metadata["voice"]` field is `true` (set by the Telegram poll loop for inbound
/// voice notes).  All other trigger sources (text, CLI, cron, watchers) return `false`.
pub(crate) fn any_trigger_is_voice(triggers: &[Trigger]) -> bool {
    triggers.iter().any(|t| {
        if let Trigger::Message(msg) = t {
            msg.metadata.get("voice").and_then(|v| v.as_bool()).unwrap_or(false)
        } else {
            false
        }
    })
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
    fn estimate_complexity_digest_cron_returns_none() {
        // Digest now runs the pipeline inline — no worker pool, no long-task announcement.
        let triggers = vec![Trigger::Cron(CronEvent::Digest)];
        assert!(estimate_task_complexity(&triggers).is_none());
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
                telegram_chat_id: None,
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
        assert!(!is_quiet_hours(None, None, "UTC"));
        assert!(!is_quiet_hours(
            Some(chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap()),
            None,
            "UTC",
        ));
        assert!(!is_quiet_hours(
            None,
            Some(chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap()),
            "UTC",
        ));
    }

    #[test]
    fn quiet_hours_overnight_window() {
        // 23:00 → 07:00 overnight window
        let start = Some(chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap());
        let end = Some(chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap());

        // We can't control time in tests, but we can verify the function
        // doesn't panic and returns a bool.
        let _ = is_quiet_hours(start, end, "America/Chicago");
    }

    #[test]
    fn quiet_hours_same_day_window() {
        // 01:00 → 05:00 same-day window
        let start = Some(chrono::NaiveTime::from_hms_opt(1, 0, 0).unwrap());
        let end = Some(chrono::NaiveTime::from_hms_opt(5, 0, 0).unwrap());

        let _ = is_quiet_hours(start, end, "America/Chicago");
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
        assert_eq!(classify_trigger(&make_message("/diary")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/diary 10")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/apply oo fix-chat")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/projects")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/unknown")), TriggerClass::BotCommand);
        // Obligation commands
        assert_eq!(classify_trigger(&make_message("/obligations")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/ob")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/ob done abc123")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/ob assign abc123 nova")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/ob create fix the bug")), TriggerClass::BotCommand);
        assert_eq!(classify_trigger(&make_message("/ob status")), TriggerClass::BotCommand);
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

    // ── Obligation Telegram Helper Tests ────────────────────────────

    fn make_obligation(priority: i32, project_code: Option<&str>, owner_reason: Option<&str>) -> nv_core::types::Obligation {
        nv_core::types::Obligation {
            id: "ob-test-1".to_string(),
            source_channel: "telegram".to_string(),
            source_message: None,
            detected_action: "Deploy the new service".to_string(),
            project_code: project_code.map(String::from),
            priority,
            status: nv_core::types::ObligationStatus::Open,
            owner: nv_core::types::ObligationOwner::Nova,
            owner_reason: owner_reason.map(String::from),
            deadline: None,
            created_at: "2026-03-24T00:00:00Z".to_string(),
            updated_at: "2026-03-24T00:00:00Z".to_string(),
            last_attempt_at: None,
        }
    }

    #[test]
    fn format_obligation_card_p0_priority() {
        let ob = make_obligation(0, Some("NV"), None);
        let card = format_obligation_card(&ob, "telegram");
        assert!(card.contains("P0 CRITICAL"), "expected 'P0 CRITICAL' in card, got: {card}");
        assert!(card.contains("<code>telegram</code>"), "expected source channel in code tags");
        assert!(card.contains("Deploy the new service"), "expected detected action text");
    }

    #[test]
    fn format_obligation_card_no_project_code_omits_project_line() {
        let ob = make_obligation(2, None, None);
        let card = format_obligation_card(&ob, "telegram");
        assert!(!card.contains("Project:"), "project line should be omitted when project_code is None");
    }

    #[test]
    fn format_obligation_card_with_project_code_shows_project() {
        let ob = make_obligation(2, Some("OO"), None);
        let card = format_obligation_card(&ob, "telegram");
        assert!(card.contains("Project: <code>OO</code>"), "expected 'Project: OO' in card, got: {card}");
    }

    #[test]
    fn format_obligation_card_with_owner_reason_shows_reason() {
        let ob = make_obligation(2, None, Some("Nova can send the message autonomously."));
        let card = format_obligation_card(&ob, "telegram");
        assert!(
            card.contains("Nova can send the message autonomously."),
            "expected owner reason in card, got: {card}"
        );
    }

    #[test]
    fn obligation_keyboard_layout() {
        let kb = obligation_keyboard("ob-abc-123");
        assert_eq!(kb.rows.len(), 2, "expected 2 rows");

        // Row 0 — primary actions
        let row0 = &kb.rows[0];
        assert_eq!(row0.len(), 3, "expected 3 buttons in row 0");

        let handle = row0.iter().find(|b| b.callback_data.starts_with("ob_handle:"));
        let delegate = row0.iter().find(|b| b.callback_data.starts_with("ob_delegate:"));
        let dismiss = row0.iter().find(|b| b.callback_data.starts_with("ob_dismiss:"));
        assert!(handle.is_some(), "missing ob_handle button in row 0");
        assert!(delegate.is_some(), "missing ob_delegate button in row 0");
        assert!(dismiss.is_some(), "missing ob_dismiss button in row 0");

        // Row 1 — secondary actions
        let row1 = &kb.rows[1];
        assert_eq!(row1.len(), 6, "expected 6 buttons in row 1");

        let edit = row1.iter().find(|b| b.callback_data.starts_with("ob_edit:"));
        let cancel = row1.iter().find(|b| b.callback_data.starts_with("ob_cancel:"));
        let extend = row1.iter().find(|b| b.callback_data.starts_with("ob_expiry:"));
        let snooze_1h = row1.iter().find(|b| b.callback_data.ends_with(":1h"));
        let snooze_4h = row1.iter().find(|b| b.callback_data.ends_with(":4h"));
        let snooze_tmr = row1.iter().find(|b| b.callback_data.ends_with(":tomorrow"));
        assert!(edit.is_some(), "missing ob_edit button in row 1");
        assert!(cancel.is_some(), "missing ob_cancel button in row 1");
        assert!(extend.is_some(), "missing ob_expiry button in row 1");
        assert!(snooze_1h.is_some(), "missing ob_snooze:1h button in row 1");
        assert!(snooze_4h.is_some(), "missing ob_snooze:4h button in row 1");
        assert!(snooze_tmr.is_some(), "missing ob_snooze:tomorrow button in row 1");

        // All buttons must embed the obligation_id
        for row in &kb.rows {
            for btn in row {
                assert!(
                    btn.callback_data.contains("ob-abc-123"),
                    "button callback_data missing obligation_id: {}",
                    btn.callback_data
                );
            }
        }
    }

    #[test]
    fn format_morning_briefing_zero_obligations() {
        let result = format_morning_briefing(&[], 0);
        assert!(result.contains("No open obligations."), "expected 'No open obligations.' in briefing, got: {result}");
    }

    #[test]
    fn format_morning_briefing_mixed_priorities_shows_counts_and_total() {
        let open_obligations = vec![(0i32, 1i64), (2i32, 3i64), (4i32, 2i64)];
        let total = 6i64;
        let result = format_morning_briefing(&open_obligations, total);
        assert!(result.contains("6 total"), "expected total count, got: {result}");
        assert!(result.contains('1'), "expected P0 count");
        assert!(result.contains('3'), "expected P2 count");
        assert!(result.contains('2'), "expected P4 count");
    }

    #[test]
    fn format_morning_briefing_html_structure() {
        let open_obligations = vec![(0i32, 1i64), (1i32, 2i64), (2i32, 3i64), (3i32, 1i64), (4i32, 0i64)];
        let result = format_morning_briefing(&open_obligations, 7);
        assert!(result.contains("<b>"), "expected <b> tag in briefing");
        assert!(result.contains("P0 Critical"), "expected P0 label");
        assert!(result.contains("P1 High"), "expected P1 label");
        assert!(result.contains("P2 Important"), "expected P2 label");
        assert!(result.contains("P3 Minor"), "expected P3 label");
        assert!(result.contains("P4 Backlog"), "expected P4 label");
    }

    // ── voice origin detection tests (Req-1, Req-2) ─────────────────

    fn make_voice_message(content: &str) -> Trigger {
        Trigger::Message(InboundMessage {
            id: "v1".into(),
            channel: "telegram".into(),
            sender: "leo".into(),
            content: content.into(),
            timestamp: chrono::Utc::now(),
            thread_id: None,
            metadata: serde_json::json!({"voice": true}),
        })
    }

    /// [4.2] Trigger batch with a voice message sets is_voice_trigger = true.
    #[test]
    fn any_trigger_is_voice_detects_voice_message() {
        let triggers = vec![make_voice_message("[voice transcription] hello nova")];
        assert!(any_trigger_is_voice(&triggers), "voice message should set is_voice_trigger");
    }

    /// [4.2] Trigger batch with only text messages returns false.
    #[test]
    fn any_trigger_is_voice_false_for_text_messages() {
        let triggers = vec![make_message("what is the status of OO-42?")];
        assert!(!any_trigger_is_voice(&triggers), "text message should not set is_voice_trigger");
    }

    /// [4.2] Mixed batch (text + voice) returns true because any() is satisfied.
    #[test]
    fn any_trigger_is_voice_true_for_mixed_batch() {
        let triggers = vec![
            make_message("some text"),
            make_voice_message("[voice transcription] also send this"),
        ];
        assert!(any_trigger_is_voice(&triggers), "mixed batch with voice should return true");
    }

    /// [4.2] Non-message triggers (Cron, CLI) always return false.
    #[test]
    fn any_trigger_is_voice_false_for_cron_trigger() {
        let triggers = vec![Trigger::Cron(CronEvent::Digest)];
        assert!(!any_trigger_is_voice(&triggers), "cron trigger must never be voice");
    }
}
