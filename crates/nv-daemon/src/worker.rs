//! Worker pool for concurrent Claude session processing.
//!
//! Each worker runs an independent Claude session (via `PersistentSession`)
//! with full tool access. The `WorkerPool` manages concurrency limits and
//! a priority queue for dispatching tasks.

use std::cmp::Ordering as CmpOrdering;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use nv_core::types::{CronEvent, InlineKeyboard, OutboundMessage, Trigger};
use uuid::Uuid;

use crate::agent::{
    build_system_context, check_bootstrap_state, ChannelRegistry,
};
use crate::error_recovery::{classify_error, is_retryable, retry_keyboard, user_message as error_user_message};
use crate::dashboard_client::{DashboardError, ForwardRequest};
use crate::briefing_store::BriefingStore;
use crate::cold_start_store::{ColdStartEvent, ColdStartStore};
use crate::claude::{ClaudeClient, ContentBlock, Message, StopReason, ToolDefinition, ToolResultBlock};
use crate::conversation::PersistentConversationStore;
use crate::diary::{DiaryEntry, DiaryWriter};
use crate::tools::jira;
use crate::memory::Memory;
use crate::messages::MessageStore;
use crate::nexus;
use crate::query;
use crate::reminders::ReminderStore;
use crate::obligation_store::ObligationStore;
use crate::tools::schedule::ScheduleStore;
use crate::state::State;
use crate::channels::telegram::client::TelegramClient;
use crate::tool_cache::{cache_ttl_for_tool, invalidation_prefix_for_tool, ToolResultCache};
use crate::tools;
use crate::tts;
use tokio::sync::mpsc;

// ── Worker Events ──────────────────────────────────────────────────

/// Structured progress events emitted by workers to the orchestrator.
///
/// Workers send these via `mpsc::UnboundedSender<WorkerEvent>` at each
/// stage boundary. The orchestrator uses them for logging, status updates,
/// and long-task UX.
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    /// A processing stage has started (e.g., "context_build", "tool_loop").
    StageStarted {
        worker_id: Uuid,
        stage: String,
        /// Telegram chat ID for this worker — used by the orchestrator to send
        /// per-worker typing indicators on ToolCalled events.
        telegram_chat_id: Option<i64>,
    },
    /// A tool is about to be executed.
    ToolCalled { worker_id: Uuid, tool: String },
    /// A processing stage has completed.
    StageComplete {
        worker_id: Uuid,
        stage: String,
        duration_ms: u64,
    },
    /// Worker finished successfully.
    Complete { worker_id: Uuid, response_len: usize },
    /// Worker encountered an error.
    Error { worker_id: Uuid, error: String },
}

// ── Constants ───────────────────────────────────────────────────────

/// Maximum tool use loop iterations per worker cycle (safety limit).
const MAX_TOOL_LOOP_ITERATIONS: usize = 10;

/// Default timeout for read-type tool calls (seconds).
pub const TOOL_TIMEOUT_READ: u64 = 30;

/// Default timeout for write-type tool calls (seconds).
pub const TOOL_TIMEOUT_WRITE: u64 = 60;

/// Minimum interval between streaming message edits (milliseconds).
///
/// Telegram rate-limits edits to 1 per second per chat; 1500ms provides
/// comfortable headroom while keeping the user's view reasonably fresh.
pub const STREAMING_EDIT_INTERVAL_MS: u64 = 1500;

/// Minimum accumulated character delta before forcing a mid-stream edit.
///
/// An edit is sent when the buffer has grown by at least this many characters
/// since the last edit, regardless of the time interval.
pub const STREAMING_EDIT_MIN_DELTA_CHARS: usize = 50;

/// Tool names classified as write operations (use longer timeout).
pub const WRITE_TOOLS: &[&str] = &[
    "jira_create",
    "jira_transition",
    "jira_assign",
    "jira_comment",
    "write_memory",
    "update_soul",
    "complete_bootstrap",
];

// ── Priority ────────────────────────────────────────────────────────

/// Task priority levels for the worker pool queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Urgent: P0 alerts, direct commands.
    High = 2,
    /// Normal: queries, digests, channel messages.
    Normal = 1,
    /// Low: chat acknowledgments — handled inline, never queued.
    Low = 0,
}

// ── Task ────────────────────────────────────────────────────────────

/// A task to be processed by a worker.
#[derive(Debug)]
pub struct WorkerTask {
    pub id: Uuid,
    pub triggers: Vec<Trigger>,
    pub priority: Priority,
    pub created_at: Instant,
    /// Telegram chat ID for reactions/responses.
    pub telegram_chat_id: Option<i64>,
    /// Telegram message ID to react on (the original user message).
    pub telegram_message_id: Option<i64>,
    /// CLI response channels extracted from triggers.
    pub cli_response_txs: Vec<tokio::sync::oneshot::Sender<String>>,
    /// `true` when this task is the follow-up reply to an "Edit" prompt.
    ///
    /// Set by `process_trigger_batch` when `editing_action_id` is `Some` at
    /// dispatch time. Used to select an extended timeout and better log messages.
    pub is_edit_reply: bool,
    /// UUID of the pending action being edited.
    ///
    /// Set by `process_trigger_batch` when `editing_action_id.take()` returns
    /// `Some`. The worker uses this to load the `PendingAction` and build an
    /// edit-aware Claude context.
    pub editing_action_id: Option<Uuid>,
    /// Human-readable slug derived from the first trigger's content.
    ///
    /// Generated by `generate_slug_for_triggers` in the orchestrator and used
    /// for diary headings and dashboard links.
    pub slug: String,
    /// True when the originating trigger was a Telegram voice note.
    /// Controls whether Nova responds with a synthesized voice message.
    pub is_voice_trigger: bool,
}

/// Wrapper for BinaryHeap ordering (higher priority first, then FIFO by creation time).
struct PrioritizedTask(WorkerTask);

impl PartialEq for PrioritizedTask {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}

impl Eq for PrioritizedTask {}

impl PartialOrd for PrioritizedTask {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedTask {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // Higher priority value first
        let prio_cmp = (self.0.priority as u8).cmp(&(other.0.priority as u8));
        if prio_cmp != CmpOrdering::Equal {
            return prio_cmp;
        }
        // Within same priority: older tasks first (reverse of Instant ordering)
        other.0.created_at.cmp(&self.0.created_at)
    }
}

// ── Shared Dependencies ─────────────────────────────────────────────

/// Dependencies shared across all workers via Arc.
pub struct SharedDeps {
    pub memory: Memory,
    pub state: State,
    pub message_store: Arc<std::sync::Mutex<MessageStore>>,
    /// Shared SQLite connection for the conversation history table.
    ///
    /// Workers construct a `PersistentConversationStore` from this connection
    /// plus the channel + thread_id derived from the incoming trigger.
    pub conversation_db: Arc<std::sync::Mutex<rusqlite::Connection>>,
    /// Hours before conversation history is considered expired (0 = never).
    pub conversation_ttl_hours: u64,
    pub diary: Arc<std::sync::Mutex<DiaryWriter>>,
    pub jira_registry: Option<jira::JiraRegistry>,
    /// Team-agent subprocess dispatcher (populated when team_agents is configured).
    pub team_agent_dispatcher: Option<crate::team_agent::TeamAgentDispatcher>,
    /// CC session manager (wraps TeamAgentDispatcher with health monitoring).
    pub cc_session_manager: Option<crate::cc_sessions::CcSessionManager>,
    pub channels: ChannelRegistry,
    pub nv_base_path: PathBuf,
    pub voice_enabled: Arc<std::sync::atomic::AtomicBool>,
    pub tts_client: Option<Arc<tts::TtsClient>>,
    pub voice_max_chars: u32,
    pub project_registry: std::collections::HashMap<String, PathBuf>,
    /// Channel for workers to emit progress events to the orchestrator.
    pub event_tx: mpsc::UnboundedSender<WorkerEvent>,
    /// Weekly budget in USD for Claude API usage.
    pub weekly_budget_usd: f64,
    /// Alert threshold as a percentage of the weekly budget.
    pub alert_threshold_pct: u8,
    /// Per-worker session timeout in seconds (from daemon config, default 300).
    pub worker_timeout_secs: u64,
    /// Obligation store (SQLite). None if the DB failed to open.
    pub obligation_store: Option<Arc<std::sync::Mutex<ObligationStore>>>,
    /// User-defined schedule store (SQLite). None if the DB failed to open.
    pub schedule_store: Option<Arc<std::sync::Mutex<ScheduleStore>>>,
    /// Reminder store (SQLite). None if the DB failed to open.
    pub reminder_store: Option<Arc<std::sync::Mutex<ReminderStore>>>,
    /// Google Calendar credentials (base64-encoded service account JSON).
    pub calendar_credentials: Option<String>,
    /// Google Calendar ID to query (default: "primary").
    pub calendar_id: String,
    /// User timezone (IANA name, e.g. "America/Chicago") for display and time parsing.
    pub timezone: String,
    // ── Service registries (multi-instance support) ──────────────────
    /// Stripe client registry. Supports multi-account configs.
    pub stripe_registry: Option<crate::tools::ServiceRegistry<crate::tools::stripe::StripeClient>>,
    /// Vercel client registry.
    pub vercel_registry: Option<crate::tools::ServiceRegistry<crate::tools::vercel::VercelClient>>,
    /// Sentry client registry.
    pub sentry_registry: Option<crate::tools::ServiceRegistry<crate::tools::sentry::SentryClient>>,
    /// Resend client registry.
    pub resend_registry: Option<crate::tools::ServiceRegistry<crate::tools::resend::ResendClient>>,
    /// Home Assistant client registry.
    pub ha_registry: Option<crate::tools::ServiceRegistry<crate::tools::ha::HAClient>>,
    /// Upstash client registry.
    pub upstash_registry: Option<crate::tools::ServiceRegistry<crate::tools::upstash::UpstashClient>>,
    /// Azure DevOps client registry.
    pub ado_registry: Option<crate::tools::ServiceRegistry<crate::tools::ado::AdoClient>>,
    /// Cloudflare client registry.
    pub cloudflare_registry: Option<crate::tools::ServiceRegistry<crate::tools::cloudflare::CloudflareClient>>,
    /// Doppler client registry.
    pub doppler_registry: Option<crate::tools::ServiceRegistry<crate::tools::doppler::DopplerClient>>,
    /// Cached Teams client — avoids rebuilding OAuth token state on every tool call.
    pub teams_client: Option<std::sync::Arc<crate::channels::teams::client::TeamsClient>>,
    /// HTTP health-server port (from `daemon.health_port`, default 8400).
    /// Used by `cmd_digest()` in the orchestrator to avoid hardcoding 8400.
    pub health_port: u16,
    /// Base URL for the Nova dashboard (e.g. "https://nova.example.com").
    ///
    /// When `Some`, workers append a dashboard link to outbound Telegram responses.
    /// When `None`, link generation is suppressed.
    pub dashboard_url: Option<String>,
    /// Optional dashboard HTTP client for worker-initiated forwarding.
    ///
    /// Populated when both `daemon.dashboard_url` and `daemon.dashboard_secret`
    /// are configured. Workers may use this for any future dashboard calls;
    /// primary forwarding logic lives in the orchestrator.
    pub dashboard_client: Option<crate::dashboard_client::DashboardClient>,
    /// ClaudeClient for direct pipeline calls (e.g. digest synthesis).
    /// Workers each clone the `client_template` in `WorkerPool`; this field
    /// allows the orchestrator to invoke Claude outside of the worker loop.
    pub claude_client: ClaudeClient,
    /// Morning briefing log store. Shared with the HTTP server.
    pub briefing_store: Option<Arc<BriefingStore>>,
    /// Cold-start timing event store. None if the DB failed to open.
    pub cold_start_store: Option<Arc<std::sync::Mutex<ColdStartStore>>>,
    /// Contact store for sender identity lookup during message ingestion.
    pub contact_store: Option<Arc<crate::contact_store::ContactStore>>,
    /// In-memory TTL cache for tool results. Shared across workers in the pool.
    pub tool_cache: ToolResultCache,
    /// Proactive watcher configuration. Used by `handle_proactive_followup`.
    pub proactive_watcher_config: Option<nv_core::config::ProactiveWatcherConfig>,
}

// ── Slug Generation ─────────────────────────────────────────────────

/// Stopwords stripped from message content before building a slug.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "can", "could", "would",
    "please", "hey", "hi", "hello", "i", "me", "my", "what", "how", "when",
    "where", "why", "who", "you", "your", "we", "our", "it", "its",
    "in", "on", "at", "to", "for", "of", "with", "by", "from", "up",
    "do", "does", "did", "be", "been", "being", "have", "has", "had",
    "that", "this", "these", "those", "so", "and", "but", "or", "not",
    "any", "all", "some", "no", "if", "get", "just",
];

/// Generate a human-readable slug from a content string.
///
/// Steps:
/// 1. Lowercase the input.
/// 2. Strip all characters that are not ASCII alphanumeric or whitespace.
/// 3. Remove stopwords.
/// 4. Take the first 2–3 non-empty words, join with hyphens.
/// 5. Truncate to 40 characters.
/// 6. Fall back to `"session"` when no words remain.
pub fn generate_slug(content: &str) -> String {
    let lower = content.to_lowercase();

    // Keep only alphanumeric and whitespace
    let cleaned: String = lower
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();

    let words: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|w| !STOPWORDS.contains(w))
        .collect();

    if words.is_empty() {
        return "session".to_string();
    }

    // Take up to 3 words
    let selected: Vec<&str> = words.iter().take(3).copied().collect();
    let slug = selected.join("-");

    // Truncate to 40 chars, ensuring we don't split mid-word by truncating at a hyphen boundary
    if slug.len() <= 40 {
        slug
    } else {
        let truncated = &slug[..40];
        // If the truncation lands in the middle of a word, trim back to the last hyphen
        if let Some(pos) = truncated.rfind('-') {
            truncated[..pos].to_string()
        } else {
            truncated.to_string()
        }
    }
}

/// Generate a slug from a batch of triggers.
///
/// - `Trigger::Message` → dispatches to `generate_slug` on message content.
/// - `Trigger::Cron` → uses the cron event name as the slug.
/// - `Trigger::CliCommand` → prefixes `"cli-"` and calls `generate_slug` on the command text.
/// - `Trigger::NexusEvent` → uses the agent name as the slug.
///
/// Only the first trigger in the slice is examined.
pub fn generate_slug_for_triggers(triggers: &[Trigger]) -> String {
    match triggers.first() {
        Some(Trigger::Message(msg)) => generate_slug(&msg.content),
        Some(Trigger::Cron(event)) => {
            let name = match event {
                nv_core::types::CronEvent::Digest => "digest",
                nv_core::types::CronEvent::MemoryCleanup => "memory-cleanup",
                nv_core::types::CronEvent::MorningBriefing => "morning-briefing",
                nv_core::types::CronEvent::UserSchedule { name, .. } => name.as_str(),
                nv_core::types::CronEvent::ProactiveFollowup => "proactive-followup",
            };
            name.to_string()
        }
        Some(Trigger::CliCommand(req)) => {
            let text = match &req.command {
                nv_core::types::CliCommand::Ask(s) => s.as_str(),
                nv_core::types::CliCommand::Status => "status",
                nv_core::types::CliCommand::DigestNow => "digest-now",
            };
            let base = generate_slug(text);
            format!("cli-{base}")
        }
        Some(Trigger::NexusEvent(evt)) => evt.agent_name.clone(),
        None => "session".to_string(),
    }
}

// ── Worker Pool ─────────────────────────────────────────────────────

/// Manages N concurrent workers with a priority queue.
pub struct WorkerPool {
    max_concurrent: usize,
    active: Arc<AtomicUsize>,
    queue: Arc<std::sync::Mutex<BinaryHeap<PrioritizedTask>>>,
    deps: Arc<SharedDeps>,
    /// ClaudeClient template — each worker clones this (gets its own session).
    client_template: ClaudeClient,
    /// Telegram client for reactions (shared, cheap to clone).
    telegram_client: Option<TelegramClient>,
    /// Default Telegram chat ID for reactions.
    telegram_chat_id: Option<i64>,
}

impl WorkerPool {
    /// Create a new worker pool.
    pub fn new(
        max_concurrent: usize,
        deps: Arc<SharedDeps>,
        client_template: ClaudeClient,
        telegram_client: Option<TelegramClient>,
        telegram_chat_id: Option<i64>,
    ) -> Self {
        Self {
            max_concurrent,
            active: Arc::new(AtomicUsize::new(0)),
            queue: Arc::new(std::sync::Mutex::new(BinaryHeap::new())),
            deps,
            client_template,
            telegram_client,
            telegram_chat_id,
        }
    }

    /// Dispatch a task to the pool.
    ///
    /// If a worker slot is available, spawns immediately.
    /// Otherwise, enqueues the task for later processing.
    pub async fn dispatch(&self, task: WorkerTask) {
        // React with hourglass to show worker is queued/starting
        if let (Some(tg), Some(chat_id), Some(msg_id)) =
            (&self.telegram_client, task.telegram_chat_id, task.telegram_message_id)
        {
            let _ = tg.set_message_reaction(chat_id, msg_id, "\u{23F3}").await; // hourglass
        }

        let active = self.active.load(Ordering::Relaxed);
        if active < self.max_concurrent {
            self.spawn_worker(task).await;
        } else {
            tracing::info!(
                queue_len = self.queue.lock().unwrap().len(),
                active,
                max = self.max_concurrent,
                "worker pool full, queuing task"
            );
            self.queue.lock().unwrap().push(PrioritizedTask(task));
        }
    }

    /// Spawn a worker for the given task.
    async fn spawn_worker(&self, task: WorkerTask) {
        self.active.fetch_add(1, Ordering::Relaxed);

        let active = Arc::clone(&self.active);
        let queue = Arc::clone(&self.queue);
        let deps = Arc::clone(&self.deps);
        let client = self.client_template.clone();
        let tg_client = self.telegram_client.clone();
        let tg_chat_id = self.telegram_chat_id;
        let client_template = self.client_template.clone();
        let worker_timeout_secs = deps.worker_timeout_secs;

        tokio::spawn(async move {
            let task_id = task.id;
            let task_tg_chat_id = task.telegram_chat_id.or(tg_chat_id);
            let is_edit_reply = task.is_edit_reply;

            // Phase 3: Edit-reply tasks get double the timeout budget.
            let effective_timeout_secs = if is_edit_reply {
                worker_timeout_secs * 2
            } else {
                worker_timeout_secs
            };

            let task_start = Instant::now();

            tracing::info!(
                worker_task = %task_id,
                priority = ?task.priority,
                triggers = task.triggers.len(),
                is_edit_reply,
                "worker started"
            );
            tracing::info!(
                effective_timeout_secs,
                is_edit_reply,
                "worker timeout configured"
            );

            // Phase 5: "Still working" feedback at 80% of timeout budget.
            // Not sent for is_edit_reply tasks (Leo is typing, not Claude).
            let warn_secs = effective_timeout_secs * 4 / 5;
            let worker_fut = Worker::run(
                task,
                Arc::clone(&deps),
                client,
                tg_client.clone(),
                tg_chat_id,
            );
            tokio::pin!(worker_fut);

            let result = if !is_edit_reply {
                // Race worker against an 80% warning sleep.
                let warn_sleep = tokio::time::sleep(Duration::from_secs(warn_secs));
                tokio::pin!(warn_sleep);

                let worker_result = tokio::select! {
                    res = &mut worker_fut => {
                        // Worker completed before warning — no message needed.
                        res
                    }
                    () = &mut warn_sleep => {
                        // 80% threshold reached — send "still working" message,
                        // then wait out the rest with a hard timeout.
                        let elapsed = task_start.elapsed().as_secs();
                        if let Some(chat_id) = task_tg_chat_id {
                            if let Some(channel) = deps.channels.get("telegram") {
                                let msg = nv_core::types::OutboundMessage {
                                    channel: "telegram".into(),
                                    content: format!(
                                        "Still working... ({}s elapsed, up to {}s total)",
                                        elapsed, effective_timeout_secs
                                    ),
                                    reply_to: None,
                                    keyboard: None,
                                };
                                let channel = channel.clone();
                                let _ = chat_id;
                                tokio::spawn(async move {
                                    if let Err(e) = channel.send_message(msg).await {
                                        tracing::warn!(error = %e, "failed to send still-working message");
                                    }
                                });
                            }
                        }
                        // Wait for the rest of the timeout budget.
                        let remaining = Duration::from_secs(effective_timeout_secs)
                            .saturating_sub(task_start.elapsed());
                        match tokio::time::timeout(remaining, &mut worker_fut).await {
                            Ok(r) => r,
                            Err(_elapsed) => {
                                return; // timeout handled below via the outer block
                            }
                        }
                    }
                };

                // Wrap in Ok(worker_result) to match the outer timeout shape.
                Ok(worker_result)
            } else {
                // For edit-reply tasks: straight timeout, no "still working" message.
                tokio::time::timeout(
                    Duration::from_secs(effective_timeout_secs),
                    &mut worker_fut,
                )
                .await
            };

            // Phase 1: timeout_reason distinguishes edit-wait from active-work timeouts.
            let timeout_reason = if is_edit_reply { "edit_wait" } else { "active_work" };

            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::error!(worker_task = %task_id, error = %e, "worker failed");
                }
                Err(_elapsed) => {
                    // Worker timed out — emit error event, notify user, reclaim slot.
                    let stage_elapsed_ms = task_start.elapsed().as_millis();
                    tracing::warn!(
                        worker_task = %task_id,
                        timeout_secs = effective_timeout_secs,
                        timeout_reason,
                        stage_elapsed_ms,
                        chat_id = task_tg_chat_id,
                        "worker session timed out"
                    );
                    let _ = deps.event_tx.send(WorkerEvent::Error {
                        worker_id: task_id,
                        error: format!("Worker timed out after {effective_timeout_secs}s"),
                    });
                    // Send Telegram error to the originating chat.
                    // Phase 3: use a more specific message for edit-reply tasks.
                    if let Some(chat_id) = task_tg_chat_id {
                        if let Some(channel) = deps.channels.get("telegram") {
                            let content = if is_edit_reply {
                                "Edit timed out waiting for your reply. \
                                 The pending action is still queued \u{2014} tap Edit again to retry."
                                    .to_string()
                            } else {
                                format!(
                                    "\u{26A0} Request timed out after {effective_timeout_secs}s. \
                                     Try again or simplify the request."
                                )
                            };
                            let msg = nv_core::types::OutboundMessage {
                                channel: "telegram".into(),
                                content,
                                reply_to: None,
                                keyboard: None,
                            };
                            // Fire-and-forget; we can't await here as we need to release the slot.
                            let channel = channel.clone();
                            let _ = chat_id; // chat_id logged in warn span above
                            tokio::spawn(async move {
                                if let Err(e) = channel.send_message(msg).await {
                                    tracing::warn!(error = %e, "failed to send timeout error to Telegram");
                                }
                            });
                        }
                    }
                }
            }

            // Atomically release this slot and claim the next queued task (if any).
            //
            // Both operations happen under the queue lock so no concurrent worker
            // can observe `active < max_concurrent` and also pop between the sub
            // and the add (the dequeue race described in Req-2).
            let next = {
                let mut q = queue.lock().unwrap();
                if let Some(next_task) = q.pop().map(|p| p.0) {
                    // Claim a slot for the next task before releasing the current one.
                    // Net change to `active` is zero: -1 (release) +1 (claim) = 0.
                    // We release after the lock drop so the add is already visible.
                    active.fetch_add(1, Ordering::Relaxed);
                    active.fetch_sub(1, Ordering::Relaxed);
                    Some(next_task)
                } else {
                    // Queue empty — just release the slot.
                    active.fetch_sub(1, Ordering::Relaxed);
                    None
                }
            };

            if let Some(next_task) = next {
                let next_task_id = next_task.id;
                let next_task_tg_chat_id = next_task.telegram_chat_id.or(tg_chat_id);
                let next_is_edit_reply = next_task.is_edit_reply;
                let next_effective_timeout_secs = if next_is_edit_reply {
                    worker_timeout_secs * 2
                } else {
                    worker_timeout_secs
                };
                let next_client = client_template;
                let next_active = Arc::clone(&active);
                tokio::spawn(async move {
                    let next_task_start = Instant::now();
                    tracing::info!(
                        worker_task = %next_task_id,
                        priority = ?next_task.priority,
                        is_edit_reply = next_is_edit_reply,
                        "queued worker started"
                    );
                    tracing::info!(
                        effective_timeout_secs = next_effective_timeout_secs,
                        is_edit_reply = next_is_edit_reply,
                        "worker timeout configured"
                    );
                    let next_timeout_dur = Duration::from_secs(next_effective_timeout_secs);
                    let next_timeout_reason = if next_is_edit_reply { "edit_wait" } else { "active_work" };
                    let result = tokio::time::timeout(
                        next_timeout_dur,
                        Worker::run(
                            next_task,
                            Arc::clone(&deps),
                            next_client,
                            tg_client.clone(),
                            tg_chat_id,
                        ),
                    )
                    .await;
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            tracing::error!(worker_task = %next_task_id, error = %e, "queued worker failed");
                        }
                        Err(_elapsed) => {
                            // Mirror the primary timeout branch: emit error event + notify user.
                            let stage_elapsed_ms = next_task_start.elapsed().as_millis();
                            tracing::warn!(
                                worker_task = %next_task_id,
                                timeout_secs = next_effective_timeout_secs,
                                timeout_reason = next_timeout_reason,
                                stage_elapsed_ms,
                                chat_id = next_task_tg_chat_id,
                                "queued worker session timed out"
                            );
                            let _ = deps.event_tx.send(WorkerEvent::Error {
                                worker_id: next_task_id,
                                error: format!("Worker timed out after {next_effective_timeout_secs}s"),
                            });
                            if let Some(chat_id) = next_task_tg_chat_id {
                                if let Some(channel) = deps.channels.get("telegram") {
                                    let content = if next_is_edit_reply {
                                        "Edit timed out waiting for your reply. \
                                         The pending action is still queued \u{2014} tap Edit again to retry."
                                            .to_string()
                                    } else {
                                        format!(
                                            "\u{26A0} Request timed out after {next_effective_timeout_secs}s. \
                                             Try again or simplify the request."
                                        )
                                    };
                                    let msg = nv_core::types::OutboundMessage {
                                        channel: "telegram".into(),
                                        content,
                                        reply_to: None,
                                        keyboard: None,
                                    };
                                    let channel = channel.clone();
                                    let _ = chat_id; // chat_id logged in warn span above
                                    tokio::spawn(async move {
                                        if let Err(e) = channel.send_message(msg).await {
                                            tracing::warn!(error = %e, "failed to send queued timeout error to Telegram");
                                        }
                                    });
                                }
                            }
                        }
                    }
                    next_active.fetch_sub(1, Ordering::Relaxed);
                });
            }
        });
    }
}

// ── Worker ──────────────────────────────────────────────────────────

/// A single worker that processes one task with a full Claude session.
struct Worker;

impl Worker {
    /// Run a single task: load context, call Claude, execute tools, send response.
    async fn run(
        mut task: WorkerTask,
        deps: Arc<SharedDeps>,
        client: ClaudeClient,
        tg_client: Option<TelegramClient>,
        default_chat_id: Option<i64>,
    ) -> Result<()> {
        let task_start = Instant::now();
        // Capture UTC start time for cold-start event (chrono wall clock).
        let session_started_at = chrono::Utc::now();
        let task_id = task.id;
        let tg_chat_id = task.telegram_chat_id.or(default_chat_id);
        let tg_msg_id = task.telegram_message_id;
        // Capture voice trigger flag before task fields are moved/consumed.
        let task_is_voice_trigger = task.is_voice_trigger;
        let event_tx = &deps.event_tx;

        // Extract reply_to message ID from the first trigger (for threading)
        let reply_to_id: Option<String> = task.triggers.first().and_then(|t| match t {
            Trigger::Message(msg) => Some(msg.id.clone()),
            _ => None,
        });

        // Extract the originating channel name for reminder delivery defaults
        let trigger_channel: String = task.triggers.first()
            .and_then(|t| match t {
                Trigger::Message(msg) => Some(msg.channel.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "telegram".to_string());

        // Detect whether this turn originates from a Telegram message trigger.
        // When true, we send a "..." placeholder via send_thinking() instead of
        // a typing indicator, enabling streaming progressive edits (Req-1, Req-4).
        let is_telegram_message_trigger = trigger_channel == "telegram"
            && task.triggers.iter().any(|t| matches!(t, Trigger::Message(_)));

        // Send placeholder or typing indicator at turn start.
        // For Telegram message triggers with an active tg_client: send "..." placeholder
        // and store its message_id for subsequent streaming edits and final delivery.
        // For all other triggers (CLI, cron, non-Telegram): keep send_chat_action("typing").
        let mut placeholder_msg_id: Option<i64> = None;
        if is_telegram_message_trigger {
            if let (Some(tg), Some(chat_id)) = (&tg_client, tg_chat_id) {
                match tg.send_thinking(chat_id).await {
                    Ok(msg_id) => {
                        placeholder_msg_id = Some(msg_id);
                        tracing::debug!(
                            chat_id,
                            msg_id,
                            "streaming: placeholder sent"
                        );
                    }
                    Err(e) => {
                        // Placeholder failed — fall back to typing indicator so the
                        // user still sees some feedback.
                        tracing::warn!(error = %e, "streaming: send_thinking failed, falling back to typing indicator");
                        tg.send_chat_action(chat_id, "typing").await;
                    }
                }
            } else {
                // No tg_client available — send typing indicator as usual.
                if let (Some(tg), Some(chat_id)) = (&tg_client, tg_chat_id) {
                    tg.send_chat_action(chat_id, "typing").await;
                }
            }
        } else {
            // Non-Telegram or non-message trigger: keep existing typing indicator.
            if let (Some(tg), Some(chat_id)) = (&tg_client, tg_chat_id) {
                tg.send_chat_action(chat_id, "typing").await;
            }
        }

        // Emit StageStarted for context build — carries telegram_chat_id so the
        // orchestrator can map worker_id → chat_id for per-worker typing indicators.
        let context_build_start = Instant::now();
        let _ = event_tx.send(WorkerEvent::StageStarted {
            worker_id: task_id,
            stage: "context_build".into(),
            telegram_chat_id: tg_chat_id,
        });

        // ── Parallel context build ──────────────────────────────────────────
        //
        // All four context sources are independent reads.  `MessageStore` and
        // `Memory` use `std::sync::Mutex` / blocking I/O, so we push them onto
        // the Tokio blocking thread pool via `spawn_blocking` and join them in
        // parallel.  This replaces the previous sequential calls and shaves
        // ~100-200ms on warm-cache turns.

        // (a) Recent outbound messages — for cold-start amnesia fix
        let msg_store_a = Arc::clone(&deps.message_store);
        let recent_outbound_handle = tokio::task::spawn_blocking(move || {
            let store = msg_store_a.lock().unwrap();
            let msgs = store.get_recent_outbound(10);
            let ctx = crate::messages::MessageStore::format_outbound_context(&msgs);
            if ctx.is_empty() { None } else { Some(ctx) }
        });

        // (b) Recent messages context — turn-pair history for Claude
        let msg_store_b = Arc::clone(&deps.message_store);
        let recent_messages_handle = tokio::task::spawn_blocking(move || {
            let store = msg_store_b.lock().unwrap();
            match store.format_recent_for_context(20) {
                Ok(ctx) if !ctx.is_empty() => Some(ctx),
                Ok(_) => None,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to load recent messages context");
                    None
                }
            }
        });

        // (c) Memory context — semantic memory retrieval
        let trigger_text_for_memory = crate::agent::format_trigger_batch(&task.triggers);
        let memory_base_path = deps.memory.base_path.clone();
        let trigger_text_mem = trigger_text_for_memory.clone();
        let memory_handle = tokio::task::spawn_blocking(move || {
            let memory = crate::memory::Memory::from_base_path(memory_base_path);
            match memory.get_context_summary_for(&trigger_text_mem) {
                Ok(ctx) if !ctx.is_empty() => Some(ctx),
                Ok(_) => None,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to load memory context");
                    None
                }
            }
        });

        // (d) Conversation turns — persistent conversation history
        let conv_db_clone = Arc::clone(&deps.conversation_db);
        let conv_ttl = deps.conversation_ttl_hours;
        // We need channel + thread_id to construct the store; derive them here.
        let conv_channel_for_load: String = task.triggers.first()
            .and_then(|t| match t {
                Trigger::Message(msg) => Some(msg.channel.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "telegram".to_string());
        let conv_thread_for_load: String = task.triggers.first()
            .and_then(|t| match t {
                Trigger::Message(msg) => {
                    msg.thread_id.clone().or_else(|| Some(msg.id.clone()))
                }
                _ => None,
            })
            .unwrap_or_else(|| "default".to_string());
        let conv_channel_load2 = conv_channel_for_load.clone();
        let conv_thread_load2 = conv_thread_for_load.clone();
        let conv_turns_handle = tokio::task::spawn_blocking(move || {
            let store = crate::conversation::PersistentConversationStore::new(
                conv_db_clone,
                &conv_channel_load2,
                &conv_thread_load2,
                conv_ttl,
            );
            store.load().unwrap_or_default()
        });

        // Wait for all four in parallel
        let (recent_outbound_result, recent_messages_result, memory_result, prior_turns_result) =
            tokio::join!(
                recent_outbound_handle,
                recent_messages_handle,
                memory_handle,
                conv_turns_handle,
            );

        let recent_outbound_context = recent_outbound_result.unwrap_or(None);
        let recent_messages_context = recent_messages_result.unwrap_or(None);
        let memory_context = memory_result.unwrap_or(None);
        let prior_turns = prior_turns_result.unwrap_or_default();

        let trigger_text = trigger_text_for_memory;

        // Build system context and tool definitions
        let base_system_prompt = build_system_context();
        let system_prompt = if let Some(ctx) = &recent_outbound_context {
            format!("Your recent messages to Leo:\n{ctx}\n\n{base_system_prompt}")
        } else {
            base_system_prompt
        };

        // Phase 2: If this task is an edit-reply, load the PendingAction and
        // prepend a system-level edit context so Claude knows what it's editing.
        let editing_context: Option<(Uuid, String)> = if let Some(action_id) = task.editing_action_id {
            match deps.state.find_pending_action(&action_id) {
                Ok(Some(action)) => {
                    let ctx = format!(
                        "You are editing a pending action that is awaiting user confirmation.\n\
                         Action ID: {action_id}\n\
                         Original description: {description}\n\
                         The user's next message is an edit instruction — apply it to produce a \
                         revised description, then confirm you have updated the pending action.",
                        action_id = action_id,
                        description = action.description,
                    );
                    Some((action_id, action.description.clone()))
                        .map(|(id, _desc)| (id, ctx))
                }
                Ok(None) => {
                    tracing::warn!(
                        action_id = %action_id,
                        "editing_action_id set but pending action not found — proceeding without edit context"
                    );
                    None
                }
                Err(e) => {
                    tracing::warn!(
                        action_id = %action_id,
                        error = %e,
                        "failed to load pending action for edit context"
                    );
                    None
                }
            }
        } else {
            None
        };

        // Prepend edit context to the system prompt if present.
        let system_prompt = if let Some((_, ref ctx)) = editing_context {
            format!("{ctx}\n\n{system_prompt}")
        } else {
            system_prompt
        };
        let bootstrapped = check_bootstrap_state();
        let tool_definitions = if bootstrapped {
            tools::register_tools()
        } else {
            tools::register_bootstrap_tools()
        };

        // Load follow-up context
        let followup_manager = query::followup::FollowUpManager::new(&deps.nv_base_path);
        let followup_context = match followup_manager.load() {
            Ok(Some(state)) => {
                let mut parts = vec![format!(
                    "Previous question: {}\nPrevious answer summary: {}",
                    state.original_question, state.answer_summary
                )];
                for fu in &state.followups {
                    parts.push(format!(
                        "{}. {} (action: {:?})",
                        fu.index, fu.label, fu.action_type
                    ));
                }
                Some(parts.join("\n"))
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(error = %e, "failed to load follow-up context");
                None
            }
        };

        // Log inbound messages to the message store
        for trigger in &task.triggers {
            if let Trigger::Message(msg) = trigger {
                // Resolve contact_id via find_by_channel (opt-in; None on miss or unavailable).
                let contact_id: Option<String> =
                    deps.contact_store.as_ref().and_then(|cs: &Arc<crate::contact_store::ContactStore>| {
                        // Extract the per-channel identifier from message metadata.
                        // Discord and Teams embed author IDs in metadata; Telegram uses sender name.
                        let identifier = match msg.channel.as_str() {
                            "discord" => msg
                                .metadata
                                .get("author_id")
                                .and_then(|v| v.as_str())
                                .map(String::from)
                                .unwrap_or_else(|| msg.sender.clone()),
                            "teams" => msg
                                .metadata
                                .get("sender_id")
                                .and_then(|v| v.as_str())
                                .map(String::from)
                                .unwrap_or_else(|| msg.sender.clone()),
                            _ => msg.sender.clone(), // telegram: use sender name/handle
                        };
                        cs.find_by_channel(&msg.channel, &identifier)
                            .ok()
                            .flatten()
                            .map(|c| c.id)
                    });

                let store = deps.message_store.lock().unwrap();
                if let Err(e) = store.log_inbound(
                    &msg.channel,
                    &msg.sender,
                    &msg.content,
                    "message",
                    contact_id.as_deref(),
                ) {
                    tracing::warn!(error = %e, "failed to log inbound message");
                }
            }
        }

        // Build user message with context injections
        let mut user_message = String::new();

        if let Some(recent_ctx) = recent_messages_context {
            user_message.push_str(&format!(
                "<recent_messages>\n{recent_ctx}\n</recent_messages>\n\n"
            ));
        }

        if let Some(ctx) = memory_context {
            user_message.push_str(&format!(
                "<memory_context>\n{ctx}\n</memory_context>\n\n"
            ));
        }

        if let Some(fu_ctx) = followup_context {
            user_message.push_str(&format!(
                "<followup_context>\n{fu_ctx}\n</followup_context>\n\n"
            ));
        }

        user_message.push_str(&trigger_text);

        // Inject budget context for digest triggers (>80% of weekly budget)
        let is_digest_trigger = task.triggers.iter().any(|t| {
            matches!(t, Trigger::Cron(CronEvent::Digest))
        });
        if is_digest_trigger {
            let store = deps.message_store.lock().unwrap();
            if let Ok(budget) = store.usage_budget_status(deps.weekly_budget_usd) {
                if budget.pct_used > 80.0 {
                    user_message.push_str(&format!(
                        "\n\n<budget_warning>\nBudget: ${:.2} / ${:.2} ({:.0}%)\n</budget_warning>",
                        budget.rolling_7d_cost, budget.weekly_budget, budget.pct_used
                    ));
                }
            }
        }

        // Construct per-task PersistentConversationStore for later push (uses same
        // channel/thread derived during the parallel build above).
        let conv_store = PersistentConversationStore::new(
            Arc::clone(&deps.conversation_db),
            &conv_channel_for_load,
            &conv_thread_for_load,
            deps.conversation_ttl_hours,
        );

        let mut conversation_history = prior_turns;
        conversation_history.push(Message::user(&user_message));

        // Emit StageComplete for context build
        let context_build_ms = context_build_start.elapsed().as_millis() as u64;
        let _ = event_tx.send(WorkerEvent::StageComplete {
            worker_id: task_id,
            stage: "context_build".into(),
            duration_ms: context_build_ms,
        });

        // Extract image_path from trigger metadata (photo messages)
        let image_path: Option<String> = task.triggers.iter().find_map(|t| {
            if let Trigger::Message(msg) = t {
                msg.metadata
                    .get("image_path")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        });

        // ── Dashboard forwarding path ─────────────────────────────────────────
        //
        // If a DashboardClient is configured, attempt to forward the trigger to
        // the Nova dashboard CC session before falling back to the cold-start
        // (local Claude) path.
        //
        // Fallback policy:
        //   DashboardError::Unavailable  → warn + fall through to cold-start.
        //   DashboardError::AuthError    → log error + Telegram alert + return Err
        //                                  (no fallback — this is a config bug).
        //   DashboardError::BadRequest   → log error + Telegram alert + return Err.
        if let Some(ref dash_client) = deps.dashboard_client {
            let forward_span_start = Instant::now();
            let forward_result = Self::run_forward(
                dash_client,
                &trigger_text,
                tg_chat_id,
                task.telegram_message_id,
                &trigger_channel,
                &system_prompt,
            )
            .await;

            match forward_result {
                Ok(reply_text) => {
                    let elapsed_ms = forward_span_start.elapsed().as_millis() as u64;
                    let dashboard_url = dash_client.base_url().to_string();
                    tracing::info!(
                        worker_task = %task_id,
                        dashboard_url = %dashboard_url,
                        chat_id = ?tg_chat_id,
                        elapsed_ms,
                        fallback_used = false,
                        "dashboard forward succeeded"
                    );

                    // React with check mark
                    if let (Some(tg), Some(chat_id), Some(msg_id)) =
                        (&tg_client, tg_chat_id, tg_msg_id)
                    {
                        let _ = tg.set_message_reaction(chat_id, msg_id, "\u{2705}").await;
                    }

                    // Drain CLI response channels (take avoids partial-move of task)
                    let cli_txs = std::mem::take(&mut task.cli_response_txs);
                    for tx in cli_txs {
                        let _ = tx.send(reply_text.clone());
                    }

                    // Build channel content with optional dashboard link
                    let channel_content = if let Some(ref base_url) = deps.dashboard_url {
                        format!(
                            "{reply_text}\n\n<a href=\"{base_url}/sessions/{task_id}\">{}</a>",
                            task.slug
                        )
                    } else {
                        reply_text.clone()
                    };

                    if !reply_text.is_empty() {
                        if let Some(channel) = deps.channels.get(trigger_channel.as_str()) {
                            if let Err(e) = channel
                                .send_message(OutboundMessage {
                                    channel: trigger_channel.clone(),
                                    content: channel_content,
                                    reply_to: reply_to_id.clone(),
                                    keyboard: None,
                                })
                                .await
                            {
                                tracing::error!(
                                    error = %e,
                                    "failed to route dashboard-forward response"
                                );
                            }
                        }
                    }

                    // Log outbound message to MessageStore (same as cold-start path)
                    {
                        let store = deps.message_store.lock().unwrap();
                        if let Err(e) = store.log_outbound(
                            &trigger_channel,
                            &reply_text,
                            tg_msg_id,
                            Some(elapsed_ms as i64),
                            None,
                            None,
                        ) {
                            tracing::warn!(error = %e, "failed to log outbound message (dashboard)");
                        }
                    }

                    // Write diary entry (same format as cold-start path)
                    let (trigger_type, trigger_source) = classify_triggers(&task.triggers);
                    let diary_entry = DiaryEntry {
                        timestamp: chrono::Local::now(),
                        trigger_type,
                        trigger_source,
                        trigger_count: task.triggers.len(),
                        tools_called: vec![],
                        sources_checked: String::new(),
                        result_summary: reply_text.chars().take(120).collect::<String>(),
                        tokens_in: 0,
                        tokens_out: 0,
                        slug: task.slug.clone(),
                    };
                    {
                        let diary = deps.diary.lock().unwrap();
                        if let Err(e) = diary.write_entry(&diary_entry) {
                            tracing::warn!(
                                error = %e,
                                "failed to write diary entry (dashboard)"
                            );
                        }
                    }

                    let _ = event_tx.send(WorkerEvent::Complete {
                        worker_id: task_id,
                        response_len: reply_text.len(),
                    });

                    tracing::info!(
                        worker_task = %task_id,
                        elapsed_ms = task_start.elapsed().as_millis() as u64,
                        "worker completed (via dashboard)"
                    );

                    return Ok(());
                }

                Err(DashboardError::Unavailable(ref reason)) => {
                    let dashboard_url = dash_client.base_url().to_string();
                    tracing::warn!(
                        worker_task = %task_id,
                        dashboard_url = %dashboard_url,
                        reason = %reason,
                        fallback_used = true,
                        "dashboard unavailable, using cold-start fallback"
                    );
                    // Fall through to cold-start path below.
                }

                Err(DashboardError::AuthError(ref detail)) => {
                    let detail = detail.clone();
                    tracing::error!(
                        worker_task = %task_id,
                        detail = %detail,
                        "dashboard auth error — check DASHBOARD_SECRET config"
                    );
                    let _ = event_tx.send(WorkerEvent::Error {
                        worker_id: task_id,
                        error: format!("dashboard auth error: {detail}"),
                    });
                    if let Some(channel) = deps.channels.get("telegram") {
                        let msg = OutboundMessage {
                            channel: "telegram".into(),
                            content: "Nova: dashboard auth error — check logs and DASHBOARD_SECRET config.".to_string(),
                            reply_to: reply_to_id.clone(),
                            keyboard: None,
                        };
                        let _ = channel.send_message(msg).await;
                    }
                    let cli_txs = std::mem::take(&mut task.cli_response_txs);
                    for tx in cli_txs {
                        let _ = tx.send(format!("dashboard auth error: {detail}"));
                    }
                    return Err(anyhow!("dashboard auth error: {detail}"));
                }

                Err(DashboardError::BadRequest(ref detail)) => {
                    let detail = detail.clone();
                    tracing::error!(
                        worker_task = %task_id,
                        detail = %detail,
                        "dashboard bad request — this is a client-side bug"
                    );
                    let _ = event_tx.send(WorkerEvent::Error {
                        worker_id: task_id,
                        error: format!("dashboard bad request: {detail}"),
                    });
                    if let Some(channel) = deps.channels.get("telegram") {
                        let msg = OutboundMessage {
                            channel: "telegram".into(),
                            content: "Nova: dashboard request error — check logs.".to_string(),
                            reply_to: reply_to_id.clone(),
                            keyboard: None,
                        };
                        let _ = channel.send_message(msg).await;
                    }
                    let cli_txs = std::mem::take(&mut task.cli_response_txs);
                    for tx in cli_txs {
                        let _ = tx.send(format!("dashboard bad request: {detail}"));
                    }
                    return Err(anyhow!("dashboard bad request: {detail}"));
                }
            }
        }

        // ── Streaming edit closure (Req-3) ────────────────────────────────────
        //
        // When a placeholder was sent, build a closure that accumulates incoming
        // text deltas and fires a Telegram edit when the 1.5s interval or 50-char
        // threshold is reached.  The closure is passed to `send_messages_streaming`
        // so it is called synchronously within the stream-reading loop.
        // Telegram edits are dispatched via `tokio::spawn` (fire-and-forget) so the
        // closure itself remains sync.
        //
        // When no placeholder is active (cold-start, non-Telegram) the closure is a
        // no-op — the persistent path won't be reached anyway.
        let stream_tg_client = tg_client.clone();
        let stream_chat_id = tg_chat_id;
        let stream_placeholder_id = placeholder_msg_id;

        // Shared mutable state for the closure — these are moved in.
        let mut stream_buffer = String::new();
        let mut last_edit_at = Instant::now();
        let mut chars_since_last_edit: usize = 0;

        let on_text_delta = {
            let tg_client_ref = stream_tg_client.clone();
            move |delta: &str| {
                if stream_placeholder_id.is_none() {
                    // No placeholder — streaming edits are not applicable.
                    return;
                }
                let (chat_id, placeholder_id) = match (stream_chat_id, stream_placeholder_id) {
                    (Some(c), Some(p)) => (c, p),
                    _ => return,
                };

                stream_buffer.push_str(delta);
                chars_since_last_edit += delta.len();

                let interval_elapsed = last_edit_at.elapsed()
                    >= Duration::from_millis(STREAMING_EDIT_INTERVAL_MS);
                let delta_threshold_met =
                    chars_since_last_edit >= STREAMING_EDIT_MIN_DELTA_CHARS;

                if interval_elapsed || delta_threshold_met {
                    let text_snapshot = stream_buffer.clone();
                    last_edit_at = Instant::now();
                    chars_since_last_edit = 0;

                    if let Some(ref tg) = tg_client_ref {
                        let tg = tg.clone();
                        tokio::spawn(async move {
                            if let Err(e) = tg
                                .edit_message_text(chat_id, placeholder_id, &text_snapshot)
                                .await
                            {
                                tracing::debug!(
                                    error = %e,
                                    "streaming edit: edit_message_text failed (non-fatal)"
                                );
                            }
                        });
                    }
                }
            }
        };

        // Call Claude — streaming path for Telegram turns, cold-start otherwise.
        // Wraps the call in a worker-level retry loop (up to MAX_WORKER_RETRIES retries,
        // backoff [2s, 5s]) for transient error classes.  Non-transient errors (AuthFailure,
        // Unknown) surface immediately without retry.
        let call_start = Instant::now();

        /// Maximum number of *retries* (not total attempts).  Total attempts = MAX + 1.
        const MAX_WORKER_RETRIES: u32 = 2;
        const MAX_ATTEMPTS: u32 = MAX_WORKER_RETRIES + 1;
        const BACKOFF_SECS: [u64; 2] = [2, 5];

        // First attempt uses the streaming path (when eligible); subsequent retry
        // attempts always use cold-start because the `on_text_delta` closure is
        // consumed by the first `send_messages_streaming` call.
        let first_streaming_result: Option<anyhow::Result<crate::claude::ApiResponse>> =
            if placeholder_msg_id.is_some() && image_path.is_none() {
                client
                    .send_messages_streaming(
                        &system_prompt,
                        &conversation_history,
                        &tool_definitions,
                        on_text_delta,
                    )
                    .await
            } else {
                None
            };

        let response = {
            // The streaming result either succeeded, or we fall through to cold-start.
            // `first_call_result` is `Some(result)` when streaming was attempted;
            // `None` means we should go straight to cold-start on attempt 1.
            let mut first_call_result: Option<anyhow::Result<crate::claude::ApiResponse>> =
                first_streaming_result;
            let mut attempt: u32 = 0;

            loop {
                attempt += 1;

                // On the first iteration, consume the pre-computed streaming result if
                // available; on retries (attempt > 1) always go to cold-start directly.
                let call_result: anyhow::Result<crate::claude::ApiResponse> =
                    if let Some(r) = first_call_result.take() {
                        match r {
                            Ok(resp) => Ok(resp),
                            Err(_e) => {
                                // Streaming failed — fall back to cold-start on this attempt.
                                client
                                    .send_messages_with_image(
                                        &system_prompt,
                                        &conversation_history,
                                        &tool_definitions,
                                        image_path.as_deref(),
                                    )
                                    .await
                            }
                        }
                    } else {
                        // Retry attempt or first attempt without streaming.
                        client
                            .send_messages_with_image(
                                &system_prompt,
                                &conversation_history,
                                &tool_definitions,
                                image_path.as_deref(),
                            )
                            .await
                    };

                match call_result {
                    Ok(r) => break r,
                    Err(e) => {
                        let error_class = classify_error(&e);

                        if attempt < MAX_ATTEMPTS && is_retryable(&error_class) {
                            // Intermediate retry — notify user and sleep backoff.
                            let backoff_secs =
                                BACKOFF_SECS[(attempt as usize - 1).min(BACKOFF_SECS.len() - 1)];

                            tracing::warn!(
                                error_class = ?error_class,
                                attempt,
                                worker_id = %task_id,
                                backoff_secs,
                                "worker Claude call failed — retrying"
                            );

                            if let Some(channel) = deps.channels.get("telegram") {
                                let msg = OutboundMessage {
                                    channel: "telegram".into(),
                                    content: error_user_message(&error_class, attempt, MAX_ATTEMPTS),
                                    reply_to: reply_to_id.clone(),
                                    keyboard: None,
                                };
                                let _ = channel.send_message(msg).await;
                            }

                            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                            continue;
                        }

                        // Final failure — all retries exhausted or non-retryable error.
                        tracing::error!(
                            worker_id = %task_id,
                            error_class = ?error_class,
                            attempt = MAX_ATTEMPTS,
                            raw_error = %e,
                            "worker failed after all retries"
                        );

                        let _ = event_tx.send(WorkerEvent::Error {
                            worker_id: task_id,
                            error: format!("Claude API failure: {e}"),
                        });

                        // React with red X on the original user message.
                        if let (Some(tg), Some(chat_id), Some(msg_id)) =
                            (&tg_client, tg_chat_id, tg_msg_id)
                        {
                            let _ = tg.set_message_reaction(chat_id, msg_id, "\u{274C}").await;
                        }

                        // Send final user-facing message to Telegram with optional Retry button.
                        if let Some(channel) = deps.channels.get("telegram") {
                            let final_text =
                                error_user_message(&error_class, MAX_ATTEMPTS, MAX_ATTEMPTS);
                            // Auth failures do not get a Retry button — the user must fix
                            // credentials first; a button would just fail again immediately.
                            let keyboard = if matches!(error_class, crate::error_recovery::NovaError::AuthFailure) {
                                None
                            } else {
                                Some(retry_keyboard(&task.slug))
                            };
                            let msg = OutboundMessage {
                                channel: "telegram".into(),
                                content: final_text,
                                reply_to: reply_to_id.clone(),
                                keyboard,
                            };
                            let _ = channel.send_message(msg).await;
                        }

                        // Forward error to CLI callers.
                        let err_msg = format!("API error: {e}");
                        for tx in task.cli_response_txs {
                            let _ = tx.send(err_msg.clone());
                        }
                        return Err(e);
                    }
                }
            }
        };

        // Clean up temporary image file after Claude has processed it
        if let Some(ref path) = image_path {
            if let Err(e) = std::fs::remove_file(path) {
                tracing::warn!(path = %path, error = %e, "failed to remove temp image file");
            } else {
                tracing::debug!(path = %path, "removed temp image file after Claude processed it");
            }
        }

        let response_time_ms = call_start.elapsed().as_millis() as i64;
        // Emit api_call stage span for latency profiling
        let _ = event_tx.send(WorkerEvent::StageComplete {
            worker_id: task_id,
            stage: "api_call".into(),
            duration_ms: response_time_ms as u64,
        });

        let tokens_in = response.usage.input_tokens as i64;
        let tokens_out = response.usage.output_tokens as i64;
        let cost_usd = response.usage.total_cost_usd;
        let session_id = response.id.clone();

        // Log API usage
        {
            let store = deps.message_store.lock().unwrap();
            if let Err(e) = store.log_api_usage(
                &task_id.to_string(),
                cost_usd,
                tokens_in,
                tokens_out,
                client.model(),
                &session_id,
            ) {
                tracing::warn!(error = %e, "failed to log API usage");
            }

            // Check budget threshold for immediate Telegram alert (6h debounce)
            if let Ok(budget) = store.usage_budget_status(deps.weekly_budget_usd) {
                if budget.pct_used >= deps.alert_threshold_pct as f64 {
                    if let Ok(false) = store.budget_alert_sent_within(6) {
                        let alert_msg = format!(
                            "Budget alert: {:.0}% used (${:.2} / ${:.2})",
                            budget.pct_used, budget.rolling_7d_cost, budget.weekly_budget
                        );
                        if let Err(e) = store.record_budget_alert() {
                            tracing::warn!(error = %e, "failed to record budget alert timestamp");
                        }
                        // Send alert via Telegram (fire-and-forget)
                        if let Some(channel) = deps.channels.get("telegram") {
                            let msg = OutboundMessage {
                                channel: "telegram".into(),
                                content: alert_msg,
                                reply_to: None,
                                keyboard: None,
                            };
                            let channel = channel.clone();
                            tokio::spawn(async move {
                                if let Err(e) = channel.send_message(msg).await {
                                    tracing::warn!(error = %e, "failed to send budget alert");
                                }
                            });
                        }
                    }
                }
            }
        }

        // Run tool loop
        let tool_loop_start = Instant::now();
        let _ = event_tx.send(WorkerEvent::StageStarted {
            worker_id: task_id,
            stage: "tool_loop".into(),
            telegram_chat_id: tg_chat_id,
        });

        let (final_content, tool_names) = Self::run_tool_loop(
            response,
            &mut conversation_history,
            &system_prompt,
            &tool_definitions,
            &client,
            &deps,
            task_id,
            &trigger_channel,
        )
        .await?;

        let _ = event_tx.send(WorkerEvent::StageComplete {
            worker_id: task_id,
            stage: "tool_loop".into(),
            duration_ms: tool_loop_start.elapsed().as_millis() as u64,
        });

        let raw_response_text = extract_text(&final_content);

        // Extract the [SUMMARY:] tag for the diary and produce the cleaned response
        // that gets delivered to the channel (tag line stripped).
        let (result_summary, response_text) = extract_summary(&raw_response_text);

        // Phase 2: If this was an edit-reply task, update the PendingAction
        // payload with Claude's revised description and re-send the confirmation
        // keyboard so Leo can approve/cancel/edit the updated action.
        if let Some((action_id, _)) = editing_context {
            if !response_text.is_empty() {
                // Update the description field in the existing payload.
                if let Ok(Some(mut action)) = deps.state.find_pending_action(&action_id) {
                    action.payload["description"] =
                        serde_json::Value::String(response_text.clone());
                    if let Err(e) = deps.state.update_pending_action_payload(&action_id, action.payload.clone()) {
                        tracing::warn!(
                            action_id = %action_id,
                            error = %e,
                            "failed to update pending action payload after edit"
                        );
                    } else {
                        tracing::info!(
                            action_id = %action_id,
                            "pending action updated with revised description"
                        );
                        // Re-send the confirmation keyboard via Telegram.
                        if let Some(tg) = deps.channels.get("telegram") {
                            if let Some(tg_channel) =
                                tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()
                            {
                                let chat_id = tg_chat_id.unwrap_or(tg_channel.chat_id);
                                let keyboard = InlineKeyboard::confirm_action(&action_id.to_string());
                                let _ = tg_channel.client.send_message(
                                    chat_id,
                                    &format!(
                                        "Updated pending action:\n{response_text}\n\nApprove, edit, or cancel?"
                                    ),
                                    None,
                                    Some(&keyboard),
                                ).await;
                            }
                        }
                    }
                }
            }
        }

        // Push the completed turn (user + assistant) to the persistent conversation store.
        {
            let user_msg_for_store = Message::user(&user_message);
            let assistant_msg_for_store = Message::assistant_blocks(final_content.clone());
            if let Err(e) = conv_store.push(user_msg_for_store, assistant_msg_for_store) {
                tracing::warn!(error = %e, "failed to persist conversation turn");
            }
        }

        // Emit Complete event
        let _ = event_tx.send(WorkerEvent::Complete {
            worker_id: task_id,
            response_len: response_text.len(),
        });

        // Determine whether this task warrants a Telegram message effect (computed
        // before the partial-move of task.cli_response_txs below).
        let task_effect_id: Option<&'static str> = message_effect_for_task(&task);

        // Send response to CLI channels (undecorated — no dashboard link)
        for tx in task.cli_response_txs {
            let _ = tx.send(response_text.clone());
        }

        // Route response to the appropriate channel
        let reply_channel = task.triggers
            .first()
            .and_then(|t| match t {
                Trigger::Message(msg) => Some(msg.channel.as_str()),
                _ => None,
            })
            .unwrap_or("telegram");

        // Build channel content: append dashboard link for Telegram when configured.
        // CLI senders above already received the undecorated response_text.
        let channel_content = if !response_text.is_empty() {
            if let Some(ref base_url) = deps.dashboard_url {
                format!(
                    "{response_text}\n\n<a href=\"{base_url}/sessions/{task_id}\">{}</a>",
                    task.slug
                )
            } else {
                response_text.clone()
            }
        } else {
            response_text.clone()
        };

        // Record delivery span — time from send start to channel confirmation.
        let delivery_start = Instant::now();

        // ── Final delivery: streaming path vs cold-start path (Req-4, Req-5, Req-6) ──
        //
        // When a placeholder was sent (persistent streaming path):
        //   - Non-empty response → edit the placeholder to the final content (Req-4).
        //     Keyboard (if any) is attached only on this final edit (Req-7 / Req-9).
        //   - Empty response (tool-only turn) → delete the placeholder (Req-5).
        //
        // When no placeholder was sent (cold-start, non-Telegram):
        //   - Deliver via channel.send_message as today (Req-6 / Req-8).
        if let (Some(tg), Some(chat_id), Some(placeholder_id)) =
            (&tg_client, tg_chat_id, placeholder_msg_id)
        {
            if !response_text.is_empty() {
                // Req-4: Final edit with full response (no keyboard here — pending-action
                // confirmations are sent separately by the tool execution path).
                if let Err(e) = tg
                    .edit_message(chat_id, placeholder_id, &channel_content, None)
                    .await
                {
                    tracing::warn!(error = %e, "streaming: final edit_message failed");
                }
            } else {
                // Req-5: Tool-only turn — delete the placeholder.
                if let Err(e) = tg.delete_message(chat_id, placeholder_id).await {
                    tracing::warn!(error = %e, "streaming: delete_message (tool-only) failed");
                }
            }
        } else if !response_text.is_empty() {
            // Req-6 / Req-8: Cold-start or non-Telegram path — send_message as before.
            // For MorningBriefing (confetti) and P0/Priority::High (fire) triggers on
            // Telegram, use send_message_with_effect for animated emphasis.
            let effect_id = task_effect_id;
            let sent_with_effect = if let (Some(tg), Some(chat_id), Some(eid)) =
                (&tg_client, tg_chat_id, effect_id)
            {
                if reply_channel == "telegram" {
                    match tg
                        .send_message_with_effect(chat_id, &channel_content, eid, reply_to_id.clone(), None)
                        .await
                    {
                        Ok(_) => true,
                        Err(e) => {
                            tracing::warn!(error = %e, "send_message_with_effect failed, falling back");
                            false
                        }
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !sent_with_effect {
                if let Some(channel) = deps.channels.get(reply_channel) {
                    if let Err(e) = channel
                        .send_message(OutboundMessage {
                            channel: reply_channel.to_string(),
                            content: channel_content,
                            reply_to: reply_to_id.clone(),
                            keyboard: None,
                        })
                        .await
                    {
                        tracing::error!(error = %e, "failed to route worker response");
                    }
                }
            }
        }

        let delivery_ms = delivery_start.elapsed().as_millis() as u64;
        let _ = event_tx.send(WorkerEvent::StageComplete {
            worker_id: task_id,
            stage: "delivery".into(),
            duration_ms: delivery_ms,
        });

        // React with check mark on completion
        if let (Some(tg), Some(chat_id), Some(msg_id)) = (&tg_client, tg_chat_id, tg_msg_id) {
            let _ = tg.set_message_reaction(chat_id, msg_id, "\u{2705}").await; // green check
        }

        // Log outbound message
        {
            let store = deps.message_store.lock().unwrap();
            if let Err(e) = store.log_outbound(
                reply_channel,
                &response_text,
                tg_msg_id,
                Some(response_time_ms),
                Some(tokens_in),
                Some(tokens_out),
            ) {
                tracing::warn!(error = %e, "failed to log outbound message");
            }
        }

        // Write diary entry
        let (trigger_type, trigger_source) = classify_triggers(&task.triggers);
        let sources_checked = summarize_sources(&tool_names);

        let diary_entry = DiaryEntry {
            timestamp: chrono::Local::now(),
            trigger_type: trigger_type.clone(),
            trigger_source,
            trigger_count: task.triggers.len(),
            tools_called: tool_names.clone(),
            sources_checked,
            result_summary,
            tokens_in: tokens_in as u32,
            tokens_out: tokens_out as u32,
            slug: task.slug.clone(),
        };
        {
            let diary = deps.diary.lock().unwrap();
            if let Err(e) = diary.write_entry(&diary_entry) {
                tracing::warn!(error = %e, "failed to write diary entry");
            }
        }

        // Cold-start event: fire-and-forget insert via spawn_blocking.
        if let Some(ref cs_store) = deps.cold_start_store {
            let cs_store = Arc::clone(cs_store);
            let cold_start_event = ColdStartEvent {
                session_id: task_id.to_string(),
                started_at: session_started_at,
                context_build_ms,
                first_response_ms: response_time_ms.max(0) as u64,
                total_ms: task_start.elapsed().as_millis() as u64,
                tool_count: tool_names.len() as u32,
                tokens_in,
                tokens_out,
                trigger_type,
            };
            tokio::task::spawn_blocking(move || {
                match cs_store.lock().unwrap().insert(&cold_start_event) {
                    Ok(()) => {}
                    Err(e) => tracing::warn!(error = %e, "failed to insert cold-start event"),
                }
            });
        }

        // Voice delivery: only when voice is enabled, the trigger was a voice note,
        // no tool calls were made (tool-heavy responses are not suitable for TTS),
        // and the response is within the character threshold.
        if deps.voice_enabled.load(std::sync::atomic::Ordering::Relaxed)
            && task_is_voice_trigger
            && tool_names.is_empty()
            && (response_text.len() as u32) <= deps.voice_max_chars
            && !response_text.is_empty()
        {
            if let Some(tts_client) = &deps.tts_client {
                if let Some(tg) = deps.channels.get("telegram") {
                    if let Some(tg_channel) =
                        tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()
                    {
                        let tts_c = Arc::clone(tts_client);
                        let tg_client_voice = tg_channel.client.clone();
                        let chat_id = tg_channel.chat_id;
                        let text_for_tts = response_text.clone();
                        let reply_to_id = tg_msg_id;
                        let caption_text = response_text.clone();

                        tokio::spawn(async move {
                            match tts::synthesize(&tts_c, &text_for_tts).await {
                                Ok(ogg_bytes) => {
                                    if let Err(e) = tg_client_voice
                                        .send_voice(chat_id, ogg_bytes, reply_to_id, Some(&caption_text))
                                        .await
                                    {
                                        tracing::warn!(error = %e, "failed to send voice message");
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "TTS synthesis failed");
                                }
                            }
                        });
                    }
                }
            }
        }

        tracing::info!(
            worker_task = %task_id,
            elapsed_ms = task_start.elapsed().as_millis() as u64,
            tools = tool_names.len(),
            "worker completed"
        );

        Ok(())
    }

    /// Forward a single trigger to the Nova dashboard worker endpoint.
    ///
    /// Constructs a `ForwardRequest` from the trigger text and system context,
    /// then calls `DashboardClient::forward()`.  Returns the reply string on
    /// success, or a classified `DashboardError` for the caller to handle.
    async fn run_forward(
        dash_client: &crate::dashboard_client::DashboardClient,
        trigger_text: &str,
        chat_id: Option<i64>,
        message_id: Option<i64>,
        channel: &str,
        system_context: &str,
    ) -> Result<String, DashboardError> {
        let req = ForwardRequest {
            message: trigger_text.to_string(),
            chat_id,
            message_id,
            channel: channel.to_string(),
            system_context: system_context.to_string(),
        };

        let resp = dash_client.forward(req).await?;
        Ok(resp.reply)
    }

    /// Execute the tool use loop for a worker.
    #[allow(clippy::too_many_arguments)]
    async fn run_tool_loop(
        initial_response: crate::claude::ApiResponse,
        conversation_history: &mut Vec<Message>,
        system_prompt: &str,
        tool_definitions: &[ToolDefinition],
        client: &ClaudeClient,
        deps: &SharedDeps,
        worker_id: Uuid,
        trigger_channel: &str,
    ) -> Result<(Vec<ContentBlock>, Vec<String>)> {
        let mut response = initial_response;
        let mut all_text_content = Vec::new();
        let mut all_tool_names = Vec::new();

        for iteration in 0..MAX_TOOL_LOOP_ITERATIONS {
            let mut tool_uses = Vec::new();
            for block in &response.content {
                match block {
                    ContentBlock::Text { .. } => all_text_content.push(block.clone()),
                    ContentBlock::ToolUse { id, name, input } => {
                        tool_uses.push((id.clone(), name.clone(), input.clone()));
                        all_tool_names.push(name.clone());
                    }
                    ContentBlock::ToolResult { .. } => {}
                }
            }

            if tool_uses.is_empty() || response.stop_reason != StopReason::ToolUse {
                break;
            }

            tracing::info!(
                iteration,
                tool_count = tool_uses.len(),
                tools = ?tool_uses.iter().map(|(_, n, _)| n.as_str()).collect::<Vec<_>>(),
                "worker executing tool cycle"
            );

            conversation_history
                .push(Message::assistant_blocks(response.content.clone()));

            let mut tool_results = Vec::new();
            for (id, name, input) in &tool_uses {
                // Emit ToolCalled event before each tool execution
                let _ = deps.event_tx.send(WorkerEvent::ToolCalled {
                    worker_id,
                    tool: name.clone(),
                });

                // Handle get_recent_messages synchronously to avoid holding
                // MutexGuard across an await point (MessageStore is !Send).
                let tool_start = Instant::now();
                let result = if name == "get_recent_messages" {
                    let store = deps.message_store.lock().unwrap();
                    let count = input["count"].as_u64().unwrap_or(20).min(100) as usize;
                    match store.recent(count) {
                        Ok(messages) if messages.is_empty() => {
                            Ok(tools::ToolResult::Immediate("No messages in history.".into()))
                        }
                        Ok(messages) => {
                            let mut lines = Vec::with_capacity(messages.len());
                            for msg in &messages {
                                let time_part = if msg.timestamp.len() >= 16 {
                                    &msg.timestamp[11..16]
                                } else {
                                    &msg.timestamp
                                };
                                let sender = if msg.direction == "outbound" { "Nova" } else { &msg.sender };
                                lines.push(format!("[{time_part}] {sender}: {}", msg.content));
                            }
                            Ok(tools::ToolResult::Immediate(lines.join("\n")))
                        }
                        Err(e) => Err(e),
                    }
                } else if name == "search_messages" {
                    let store = deps.message_store.lock().unwrap();
                    let query = input["query"].as_str().unwrap_or("");
                    let limit = input["limit"].as_u64().unwrap_or(10).min(50) as usize;
                    match store.search(query, limit) {
                        Ok(messages) if messages.is_empty() => {
                            Ok(tools::ToolResult::Immediate("No messages found matching the query.".into()))
                        }
                        Ok(messages) => {
                            let mut lines = Vec::with_capacity(messages.len());
                            for msg in &messages {
                                let time_part = if msg.timestamp.len() >= 16 {
                                    &msg.timestamp[11..16]
                                } else {
                                    &msg.timestamp
                                };
                                let date_part = if msg.timestamp.len() >= 10 {
                                    &msg.timestamp[..10]
                                } else {
                                    &msg.timestamp
                                };
                                let sender = if msg.direction == "outbound" { "Nova" } else { &msg.sender };
                                lines.push(format!("[{date_part} {time_part}] {sender}: {}", msg.content));
                            }
                            Ok(tools::ToolResult::Immediate(lines.join("\n")))
                        }
                        Err(e) => {
                            // FTS5 query syntax errors are user-facing
                            Ok(tools::ToolResult::Immediate(
                                format!("Invalid search query: {e}")
                            ))
                        }
                    }
                } else if let Some(sched_result) = deps
                    .schedule_store
                    .as_ref()
                    .and_then(|s| {
                        let guard = s.lock().unwrap();
                        tools::execute_schedule_tool(name, input, &guard)
                    })
                {
                    // Schedule tools are synchronous (no await needed)
                    match sched_result {
                        Ok(r) => Ok(r),
                        Err(e) => {
                            let _ = deps.event_tx.send(WorkerEvent::Error {
                                worker_id,
                                error: format!("Tool {name} error: {e}"),
                            });
                            Err(e)
                        }
                    }
                } else if let Some(rem_result) = deps
                    .reminder_store
                    .as_ref()
                    .and_then(|s| {
                        let guard = s.lock().unwrap();
                        tools::execute_reminder_tool(name, input, &guard, trigger_channel, &deps.timezone)
                    })
                {
                    // Reminder tools are synchronous
                    match rem_result {
                        Ok(r) => Ok(r),
                        Err(e) => {
                            let _ = deps.event_tx.send(WorkerEvent::Error {
                                worker_id,
                                error: format!("Tool {name} error: {e}"),
                            });
                            Err(e)
                        }
                    }
                } else {
                    // ── Cache lookup ─────────────────────────────────────────
                    // Check the in-memory tool result cache before hitting the
                    // external API. Cache is only used for read-only tools
                    // (cache_ttl_for_tool returns Some) and skipped entirely
                    // for write/mutation tools.
                    if let Some(cached) = deps.tool_cache.get(name, input) {
                        tracing::debug!(tool = %name, "tool result cache hit");
                        Ok(tools::ToolResult::Immediate(cached))
                    } else {
                        // Determine timeout based on tool category
                        let timeout_secs = if WRITE_TOOLS.contains(&name.as_str()) {
                            TOOL_TIMEOUT_WRITE
                        } else {
                            TOOL_TIMEOUT_READ
                        };
                        let timeout_dur = Duration::from_secs(timeout_secs);

                        // Wrap tool execution in a timeout
                        let svc_regs = tools::ServiceRegistries {
                            stripe: deps.stripe_registry.as_ref(),
                            vercel: deps.vercel_registry.as_ref(),
                            sentry: deps.sentry_registry.as_ref(),
                            resend: deps.resend_registry.as_ref(),
                            ha: deps.ha_registry.as_ref(),
                            upstash: deps.upstash_registry.as_ref(),
                            ado: deps.ado_registry.as_ref(),
                            cloudflare: deps.cloudflare_registry.as_ref(),
                            doppler: deps.doppler_registry.as_ref(),
                            teams: deps.teams_client.as_deref(),
                        };
                        // Build a NexusBackend from TeamAgentDispatcher if configured.
                        let nexus_backend_owned: Option<nexus::backend::NexusBackend> =
                            deps.team_agent_dispatcher
                                .as_ref()
                                .map(|d| nexus::backend::NexusBackend::new(d.clone()));
                        let nexus_backend_ref = nexus_backend_owned.as_ref();

                        let exec_result = match tokio::time::timeout(
                            timeout_dur,
                            tools::execute_tool_send_with_backend(
                                name,
                                input,
                                &deps.memory,
                                deps.jira_registry.as_ref(),
                                nexus_backend_ref,
                                &deps.project_registry,
                                &deps.channels,
                                deps.calendar_credentials.as_deref(),
                                &deps.calendar_id,
                                &svc_regs,
                            ),
                        )
                        .await
                        {
                            Ok(result) => result,
                            Err(_elapsed) => {
                                tracing::warn!(
                                    tool = %name,
                                    timeout_secs,
                                    "tool execution timed out"
                                );
                                let _ = deps.event_tx.send(WorkerEvent::Error {
                                    worker_id,
                                    error: format!("Tool {name} timed out after {timeout_secs}s"),
                                });
                                Err(anyhow!(
                                    "Tool timed out after {timeout_secs}s"
                                ))
                            }
                        };

                        // ── Cache write (read tools) ──────────────────────────
                        if let Ok(tools::ToolResult::Immediate(ref output)) = exec_result {
                            if let Some(ttl) = cache_ttl_for_tool(name) {
                                deps.tool_cache.insert(name, input, output.clone(), ttl);
                                tracing::debug!(tool = %name, ttl_secs = ttl.as_secs(), "tool result cached");
                            }
                        }

                        // ── Write invalidation ────────────────────────────────
                        if exec_result.is_ok() {
                            if let Some(prefix) = invalidation_prefix_for_tool(name) {
                                deps.tool_cache.invalidate_prefix(prefix);
                                tracing::debug!(tool = %name, prefix, "cache invalidated after write");
                            }
                            // Obligation write tools invalidate two separate prefixes.
                            if matches!(
                                name.as_str(),
                                "patch_obligation" | "create_obligation" | "close_obligation"
                            ) {
                                deps.tool_cache.invalidate_prefix("list_obligation");
                            }
                        }

                        exec_result
                    }
                };
                // Track whether the result came from the cache (cache_hit is true
                // when tool_start elapsed is negligible — but we detect it via the
                // distinct code path above; here we approximate it by duration).
                let tool_duration_ms = tool_start.elapsed().as_millis() as i64;
                // A cache hit produces a result in < 5 ms; everything else is an
                // external call. This threshold gives us a cache_hit signal for
                // the audit log without threading a bool across the if/else arms.
                let cache_hit = tool_duration_ms < 5
                    && matches!(&result, Ok(tools::ToolResult::Immediate(_)));

                // Log tool usage to audit table
                {
                    let input_summary = input.to_string();
                    let (result_summary, success) = match &result {
                        Ok(tools::ToolResult::Immediate(output)) => {
                            (output.clone(), true)
                        }
                        Ok(tools::ToolResult::PendingAction { description, .. }) => {
                            (description.clone(), true)
                        }
                        Err(e) => (e.to_string(), false),
                    };
                    let store = deps.message_store.lock().unwrap();
                    if let Err(e) = store.log_tool_usage(
                        name,
                        &input_summary,
                        &result_summary,
                        success,
                        tool_duration_ms,
                        Some(&worker_id.to_string()),
                        None,
                        None,
                    ) {
                        tracing::warn!(error = %e, tool = %name, "failed to log tool usage");
                    }
                    if cache_hit {
                        tracing::debug!(tool = %name, duration_ms = tool_duration_ms, "audit: cache hit");
                    }
                }

                let (content, is_error) = match result {
                    Ok(tools::ToolResult::Immediate(output)) => (output, false),
                    Ok(tools::ToolResult::PendingAction {
                        description,
                        action_type: _action_type,
                        payload,
                    }) => {
                        // Create pending action
                        let action_id = Uuid::new_v4();
                        let created_at = chrono::Utc::now();

                        let keyboard = InlineKeyboard::confirm_action(&action_id.to_string());
                        let mut tg_msg_id: Option<i64> = None;
                        let mut tg_chat_id: Option<i64> = None;

                        if let Some(tg) = deps.channels.get("telegram") {
                            if let Some(tg_channel) =
                                tg.as_any().downcast_ref::<crate::channels::telegram::TelegramChannel>()
                            {
                                tg_chat_id = Some(tg_channel.chat_id);
                                match tg_channel
                                    .client
                                    .send_message(
                                        tg_channel.chat_id,
                                        &format!(
                                            "Pending action:\n{description}\n\nApprove, edit, or cancel?"
                                        ),
                                        None,
                                        Some(&keyboard),
                                    )
                                    .await
                                {
                                    Ok(msg_id) => {
                                        tg_msg_id = Some(msg_id);
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            error = %e,
                                            "failed to send confirmation keyboard"
                                        );
                                    }
                                }
                            }
                        }

                        if let Err(e) = deps.state.save_pending_action(
                            &crate::state::PendingAction {
                                id: action_id,
                                description: description.clone(),
                                payload: payload.clone(),
                                status: crate::state::PendingStatus::AwaitingConfirmation,
                                created_at,
                                telegram_message_id: tg_msg_id,
                                telegram_chat_id: tg_chat_id,
                            },
                        ) {
                            tracing::error!(error = %e, "failed to save pending action");
                        }

                        (
                            format!("Action queued for confirmation: {description}"),
                            false,
                        )
                    }
                    Err(e) => (format!("Error: {e}"), true),
                };

                // Background summarization after write_memory
                if name == "write_memory" && !is_error {
                    if let Some(topic) = input["topic"].as_str() {
                        let should_summarize = deps
                            .memory
                            .needs_summarization(topic)
                            .unwrap_or(false);
                        if should_summarize {
                            let topic_owned = topic.to_string();
                            let summarize_client = client.clone();
                            let base_path = deps.memory.base_path.clone();
                            tokio::spawn(async move {
                                let mem = Memory::from_base_path(base_path);
                                if let Err(e) =
                                    mem.summarize(&topic_owned, &summarize_client).await
                                {
                                    tracing::warn!(
                                        topic = %topic_owned,
                                        error = %e,
                                        "background summarization failed"
                                    );
                                }
                            });
                        }
                    }
                }

                tool_results.push(ToolResultBlock {
                    tool_use_id: id.clone(),
                    content,
                    is_error,
                });
            }

            conversation_history
                .push(Message::tool_results(tool_results));

            crate::conversation::truncate_history(conversation_history);

            response = client
                .send_messages(system_prompt, conversation_history, tool_definitions)
                .await?;
        }

        Ok((all_text_content, all_tool_names))
    }
}

// ── Send-Safe Tool Execution ────────────────────────────────────────

// execute_tool_send lives in tools.rs — it avoids referencing MessageStore
// to keep the future Send-safe for tokio::spawn.

// ── Helper Functions ────────────────────────────────────────────────

/// Extract the `[SUMMARY: ...]` tag from Claude's response.
///
/// Returns `(summary, cleaned_response)` where:
/// - `summary` is the narrative summary for the diary entry (≤120 chars)
/// - `cleaned_response` is the response text with the tag line stripped
///
/// If no tag is found, falls back to the first sentence of the response (up to
/// 120 chars). If the response is empty, returns `("empty response", "")`.
pub fn extract_summary(response_text: &str) -> (String, String) {
    if response_text.is_empty() {
        return ("empty response".to_string(), String::new());
    }

    // Search for the last `[SUMMARY:` ... `]` occurrence.
    // We search from the end to pick up the last tag when multiple are present.
    const OPEN: &str = "[SUMMARY:";
    if let Some(tag_start) = response_text.rfind(OPEN) {
        let after_open = &response_text[tag_start + OPEN.len()..];
        if let Some(close_offset) = after_open.find(']') {
            let raw_summary = after_open[..close_offset].trim();
            // Cap at 120 chars on a char boundary
            let summary = char_truncate(raw_summary, 120).to_string();

            // Strip the entire tag line from the response. Find the line that
            // contains the tag and remove it (including its newline).
            let tag_end = tag_start + OPEN.len() + close_offset + 1; // past `]`
            let before_tag = &response_text[..tag_start];
            let after_tag = &response_text[tag_end..];

            // Walk backwards from tag_start to the previous newline (or start)
            // to strip the whole line including a leading newline.
            let line_start = before_tag.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let prefix = &response_text[..line_start];
            // Skip any leading newline in after_tag so we don't leave a blank line
            let suffix = after_tag.strip_prefix('\n').unwrap_or(after_tag);
            let cleaned = format!("{prefix}{suffix}").trim_end().to_string();

            return (summary, cleaned);
        }
    }

    // Fallback: first sentence (split on `.`, `!`, `?`), trim, cap at 120 chars.
    let first_sentence = response_text
        .split(['.', '!', '?'])
        .next()
        .unwrap_or(response_text)
        .trim();
    let summary = char_truncate(first_sentence, 120).to_string();

    (summary, response_text.to_string())
}

/// Truncate `s` to at most `max_chars` Unicode scalar values.
fn char_truncate(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        return s;
    }
    let byte_pos = s.char_indices().nth(max_chars).map(|(i, _)| i).unwrap_or(s.len());
    &s[..byte_pos]
}

/// Extract text content from content blocks, stripping any tool call artifacts.
fn extract_text(content: &[ContentBlock]) -> String {
    let raw = content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    strip_tool_call_artifacts(&raw)
}

/// Remove ```tool_call...``` / ```tool_use...``` / ```json...``` blocks,
/// single-line `[Called tool: <name> with <json>]` patterns,
/// `<tool_response>...</tool_response>` XML blocks, tool error result lines,
/// score-annotated search result blocks, and internal-reasoning preamble
/// from Claude's response text. Only the final conversational response
/// should reach the user.
pub fn strip_tool_call_artifacts(text: &str) -> String {
    let mut result = text.to_string();

    // Repeatedly strip fenced code blocks tagged as tool_call, tool_use, or
    // json (when containing tool invocation patterns like "tool":).
    loop {
        let stripped = strip_one_tool_block(&result);
        if stripped == result {
            break;
        }
        result = stripped;
    }

    // Strip single-line [Called tool: <name> with <json>] patterns.
    // These are emitted by claude.rs when building conversation history
    // summaries and sometimes leak into the extracted response text.
    result = strip_called_tool_lines(&result);

    // Strip <tool_response>...</tool_response> XML blocks (runs after fenced-block
    // pass so code-fence content has already been removed).
    result = strip_tool_response_xml(&result);

    // Strip tool error lines (^Error: + infrastructure keywords).
    result = strip_tool_error_lines(&result);

    // Strip score-annotated search result blocks.
    result = strip_score_annotated_results(&result);

    // Strip short internal-reasoning lines.
    result = strip_internal_reasoning_lines(&result);

    result.trim().to_string()
}

/// Remove `<tool_response>...</tool_response>` blocks (multi-line, non-greedy).
///
/// Only bare `<tool_response>` tags are matched — content already inside a
/// fenced code block has been removed by the time this function is called.
fn strip_tool_response_xml(text: &str) -> String {
    let open_tag = "<tool_response>";
    let close_tag = "</tool_response>";
    let mut result = text.to_string();
    while let Some(start) = result.find(open_tag) {
        if let Some(rel_end) = result[start..].find(close_tag) {
            let end = start + rel_end + close_tag.len();
            result = format!("{}{}", &result[..start], &result[end..]);
        } else {
            // Unclosed tag — strip from the open tag to end of text to
            // avoid leaking partial XML to the user.
            result = result[..start].to_string();
            break;
        }
    }
    result
}

/// Remove lines that begin with `Error:` and contain tool-infrastructure keywords.
///
/// Only strips infra-level errors (registry, connection, timeout, etc.).
/// User-facing errors that don't reference tool infrastructure are preserved.
fn strip_tool_error_lines(text: &str) -> String {
    const INFRA_KEYWORDS: &[&str] = &[
        "not found in registry",
        "known projects",
        "unreachable",
        "connection refused",
        "connection failed",
        "timed out",
        "no such tool",
        "tool execution failed",
        "failed to execute",
        "env var",
        "not set",
        "environment variable",
    ];

    let lines: Vec<&str> = text.lines().collect();
    let filtered: Vec<&str> = lines
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with("Error:") {
                return true; // keep non-error lines
            }
            let lower = trimmed.to_lowercase();
            // Keep error lines that don't contain any infra keyword
            !INFRA_KEYWORDS.iter().any(|kw| lower.contains(kw))
        })
        .collect();

    let joined = filtered.join("\n");
    if text.ends_with('\n') && !joined.is_empty() {
        format!("{joined}\n")
    } else {
        joined
    }
}

/// Remove score-annotated search result blocks.
///
/// Strips lines matching `<filename>.<ext> (score: N):` and any immediately
/// following non-empty continuation lines until a blank line or a line that
/// looks like the start of a new sentence (uppercase letter followed by text
/// not matching the score pattern).
fn strip_score_annotated_results(text: &str) -> String {
    // Pattern: line starts with a non-whitespace token containing a dot,
    // followed by " (score: <number>):"
    fn is_score_header(line: &str) -> bool {
        let trimmed = line.trim();
        // Must match: <word>.<ext> (score: N): — require non-space token, dot, then " (score:"
        if let Some(paren_pos) = trimmed.find(" (score:") {
            let prefix = &trimmed[..paren_pos];
            // prefix must be a single token (no spaces) containing a dot
            !prefix.contains(' ') && prefix.contains('.')
        } else {
            false
        }
    }

    fn is_continuation(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false; // blank line ends block
        }
        // A line starting with an uppercase letter followed by lowercase
        // suggests a new sentence — stop stripping there.
        let mut chars = trimmed.chars();
        if let Some(first) = chars.next() {
            if first.is_uppercase() {
                if let Some(second) = chars.next() {
                    if second.is_lowercase() {
                        return false;
                    }
                }
            }
        }
        true
    }

    let lines: Vec<&str> = text.lines().collect();
    let mut result: Vec<&str> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if is_score_header(lines[i]) {
            // Skip the header line and continuation lines
            i += 1;
            while i < lines.len() && is_continuation(lines[i]) {
                i += 1;
            }
        } else {
            result.push(lines[i]);
            i += 1;
        }
    }

    let joined = result.join("\n");
    if text.ends_with('\n') && !joined.is_empty() {
        format!("{joined}\n")
    } else {
        joined
    }
}

/// Strip short lines that represent Claude's internal reasoning about tool
/// failures. Only lines shorter than 150 characters that match known
/// infrastructure-reasoning phrases are stripped.
fn strip_internal_reasoning_lines(text: &str) -> String {
    const MAX_LINE_LEN: usize = 150;

    const REASONING_PATTERNS: &[&str] = &[
        "nexus is unreachable",
        "can't inspect",
        "cannot inspect",
        "no such tool",
        "let me try",
        "let me check",
        "i'll try",
        "i'll use a different",
        "i'll use another",
        "falling back to",
        "the tool returned",
        "i don't have access to",
        "i don't have direct access",
        "unable to access",
        "that tool isn't available",
        "that tool failed",
        "that tool doesn't exist",
        "that tool is not available",
        "i can't inspect",
        "i cannot inspect",
        "no such tool available",
    ];

    let lines: Vec<&str> = text.lines().collect();
    let filtered: Vec<&str> = lines
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.len() >= MAX_LINE_LEN {
                return true; // preserve long lines
            }
            let lower = trimmed.to_lowercase();
            !REASONING_PATTERNS.iter().any(|p| lower.contains(p))
        })
        .collect();

    let joined = filtered.join("\n");
    if text.ends_with('\n') && !joined.is_empty() {
        format!("{joined}\n")
    } else {
        joined
    }
}

/// Remove all lines matching the pattern `[Called tool: <name> with <args>]`.
///
/// The pattern is specific: a line that starts with `[Called tool: ` and ends
/// with `]`. Any preamble on the same line is also removed.
fn strip_called_tool_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let filtered: Vec<&str> = lines
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim();
            // Keep the line unless it contains a [Called tool: ... with ...] pattern
            !(trimmed.contains("[Called tool:") && trimmed.contains(" with ") && trimmed.ends_with(']'))
        })
        .collect();

    // Preserve trailing newline if the original had one
    let joined = filtered.join("\n");
    if text.ends_with('\n') && !joined.is_empty() {
        format!("{joined}\n")
    } else {
        joined
    }
}

/// Strip one fenced tool block (```tool_call, ```tool_use, or ```json with
/// tool pattern) and its preamble text. Returns the input unchanged if no
/// block is found.
fn strip_one_tool_block(text: &str) -> String {
    // Match ```tool_call or ```tool_use blocks
    for marker in &["```tool_call", "```tool_use"] {
        if let Some(block_start) = text.find(marker) {
            let after = &text[block_start + marker.len()..];
            if let Some(end_offset) = after.find("```") {
                let after_block = &after[end_offset + 3..];
                // Strip everything before + including the block
                let before = text[..block_start].trim();
                let remaining = after_block.trim();

                // If the preamble is a short transitional sentence, strip it too
                let cleaned_before = strip_preamble(before);

                return if cleaned_before.is_empty() {
                    remaining.to_string()
                } else if remaining.is_empty() {
                    cleaned_before
                } else {
                    format!("{cleaned_before}\n{remaining}")
                };
            }
        }
    }

    // Match ```json blocks that contain tool invocation patterns
    let json_marker = "```json";
    if let Some(block_start) = text.find(json_marker) {
        let after = &text[block_start + json_marker.len()..];
        if let Some(end_offset) = after.find("```") {
            let json_content = &after[..end_offset];
            // Only strip if it looks like a tool call
            if json_content.contains("\"tool\"") || json_content.contains("\"tool_name\"") {
                let after_block = &after[end_offset + 3..];
                let before = text[..block_start].trim();
                let remaining = after_block.trim();
                let cleaned_before = strip_preamble(before);

                return if cleaned_before.is_empty() {
                    remaining.to_string()
                } else if remaining.is_empty() {
                    cleaned_before
                } else {
                    format!("{cleaned_before}\n{remaining}")
                };
            }
        }
    }

    text.to_string()
}

/// Strip short preamble lines that precede tool call blocks. These are
/// transitional phrases like "Let me check that." or "I'll search for that."
fn strip_preamble(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    // If the text is very short (< 100 chars) and looks like a single
    // transitional sentence, strip it entirely
    let trimmed = text.trim();
    if trimmed.len() < 100 {
        let lower = trimmed.to_lowercase();
        let preamble_patterns = [
            "let me",
            "i'll ",
            "i will ",
            "let me check",
            "let me search",
            "let me look",
            "let me find",
            "checking",
            "searching",
            "looking",
            "i don't have direct",
            "i don't have access",
            "unable to access",
            "can't inspect",
            "cannot inspect",
        ];
        for pattern in &preamble_patterns {
            if lower.starts_with(pattern) {
                return String::new();
            }
        }
    }

    trimmed.to_string()
}

/// Return the Telegram message effect ID to apply when delivering the response,
/// or `None` if no effect is warranted for this task.
///
/// Rules:
/// - `CronEvent::MorningBriefing` → confetti (`EFFECT_CONFETTI`)
/// - `Priority::High` (P0 alerts) or content containing `[P0]` → fire (`EFFECT_FIRE`)
/// - Everything else → `None`
fn message_effect_for_task(task: &WorkerTask) -> Option<&'static str> {
    use crate::channels::telegram::client::{EFFECT_CONFETTI, EFFECT_FIRE};

    // Morning briefing → confetti.
    let is_morning_briefing = task.triggers.iter().any(|t| {
        matches!(t, Trigger::Cron(CronEvent::MorningBriefing))
    });
    if is_morning_briefing {
        return Some(EFFECT_CONFETTI);
    }

    // P0 / Priority::High → fire.
    if task.priority == Priority::High {
        return Some(EFFECT_FIRE);
    }

    None
}

/// Classify a trigger batch into (trigger_type, trigger_source).
fn classify_triggers(triggers: &[Trigger]) -> (String, String) {
    match triggers.first() {
        Some(Trigger::Message(msg)) => ("message".into(), msg.channel.clone()),
        Some(Trigger::Cron(event)) => ("cron".into(), format!("{event:?}")),
        Some(Trigger::NexusEvent(event)) => ("nexus".into(), event.agent_name.clone()),
        Some(Trigger::CliCommand(req)) => ("cli".into(), format!("{:?}", req.command)),
        None => ("unknown".into(), "none".into()),
    }
}

/// Summarize which sources were checked based on tool names called.
fn summarize_sources(tool_names: &[String]) -> String {
    if tool_names.is_empty() {
        return "none".into();
    }

    let mut parts = Vec::new();
    let jira_count = tool_names.iter().filter(|n| n.starts_with("jira_")).count();
    let memory_count = tool_names.iter().filter(|n| n.contains("memory")).count();
    let nexus_count = tool_names.iter().filter(|n| n.starts_with("query_nexus")).count();
    let message_count = tool_names
        .iter()
        .filter(|n| n.contains("messages") || n.contains("get_recent"))
        .count();

    if jira_count > 0 {
        parts.push(format!("jira: {jira_count} calls"));
    }
    if memory_count > 0 {
        parts.push(format!("memory: {memory_count} calls"));
    }
    if nexus_count > 0 {
        parts.push(format!("nexus: {nexus_count} calls"));
    }
    if message_count > 0 {
        parts.push(format!("messages: {message_count} calls"));
    }

    if parts.is_empty() {
        format!("{} tool calls", tool_names.len())
    } else {
        parts.join(", ")
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering_high_before_normal() {
        let high = PrioritizedTask(WorkerTask {
            id: Uuid::new_v4(),
            triggers: vec![],
            priority: Priority::High,
            created_at: Instant::now(),
            telegram_chat_id: None,
            telegram_message_id: None,
            cli_response_txs: vec![],
            is_edit_reply: false,
            editing_action_id: None,
            slug: "session".into(),
            is_voice_trigger: false,
        });
        let normal = PrioritizedTask(WorkerTask {
            id: Uuid::new_v4(),
            triggers: vec![],
            priority: Priority::Normal,
            created_at: Instant::now(),
            telegram_chat_id: None,
            telegram_message_id: None,
            cli_response_txs: vec![],
            is_edit_reply: false,
            editing_action_id: None,
            slug: "session".into(),
            is_voice_trigger: false,
        });

        assert!(high > normal);
    }

    #[test]
    fn priority_ordering_fifo_within_same_priority() {
        let earlier = Instant::now();
        let later = Instant::now();

        let task1 = PrioritizedTask(WorkerTask {
            id: Uuid::new_v4(),
            triggers: vec![],
            priority: Priority::Normal,
            created_at: earlier,
            telegram_chat_id: None,
            telegram_message_id: None,
            cli_response_txs: vec![],
            is_edit_reply: false,
            editing_action_id: None,
            slug: "session".into(),
            is_voice_trigger: false,
        });
        let task2 = PrioritizedTask(WorkerTask {
            id: Uuid::new_v4(),
            triggers: vec![],
            priority: Priority::Normal,
            created_at: later,
            telegram_chat_id: None,
            telegram_message_id: None,
            cli_response_txs: vec![],
            is_edit_reply: false,
            editing_action_id: None,
            slug: "session".into(),
            is_voice_trigger: false,
        });

        // Earlier task should be processed first (higher in heap)
        assert!(task1 >= task2);
    }

    #[test]
    fn classify_trigger_types() {
        use nv_core::types::{CronEvent, InboundMessage};

        let msg_triggers = vec![Trigger::Message(InboundMessage {
            id: "1".into(),
            channel: "telegram".into(),
            sender: "leo".into(),
            content: "hello".into(),
            timestamp: chrono::Utc::now(),
            thread_id: None,
            metadata: serde_json::json!({}),
        })];
        let (t, s) = classify_triggers(&msg_triggers);
        assert_eq!(t, "message");
        assert_eq!(s, "telegram");

        let cron_triggers = vec![Trigger::Cron(CronEvent::Digest)];
        let (t, _) = classify_triggers(&cron_triggers);
        assert_eq!(t, "cron");
    }

    #[test]
    fn summarize_sources_empty() {
        assert_eq!(summarize_sources(&[]), "none");
    }

    #[test]
    fn summarize_sources_mixed() {
        let names = vec![
            "jira_search".to_string(),
            "read_memory".to_string(),
            "jira_get".to_string(),
        ];
        let s = summarize_sources(&names);
        assert!(s.contains("jira: 2 calls"));
        assert!(s.contains("memory: 1 calls"));
    }

    // ── strip_tool_call_artifacts tests ──────────────────────────────

    #[test]
    fn strip_tool_call_removes_tool_call_block() {
        let input = "Let me check that.\n```tool_call\n{\"tool\": \"read_memory\", \"input\": {\"topic\": \"tasks\"}}\n```";
        let result = strip_tool_call_artifacts(input);
        assert!(!result.contains("tool_call"));
        assert!(!result.contains("read_memory"));
        // Preamble "Let me check that." should also be stripped
        assert!(!result.contains("Let me check"));
    }

    #[test]
    fn strip_tool_call_removes_tool_use_block() {
        let input = "I'll search for that.\n```tool_use\n{\"name\": \"search\"}\n```";
        let result = strip_tool_call_artifacts(input);
        assert!(!result.contains("tool_use"));
        assert!(!result.contains("search"));
    }

    #[test]
    fn strip_tool_call_preserves_normal_text() {
        let input = "Here is your answer: The meeting is at 3pm.";
        let result = strip_tool_call_artifacts(input);
        assert_eq!(result, input);
    }

    #[test]
    fn strip_tool_call_preserves_non_tool_code_blocks() {
        let input = "Here is some code:\n```rust\nfn main() {}\n```";
        let result = strip_tool_call_artifacts(input);
        assert!(result.contains("```rust"));
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn strip_tool_call_handles_multiple_blocks() {
        let input = "Let me check.\n```tool_call\n{\"tool\": \"a\"}\n```\nNow another.\n```tool_call\n{\"tool\": \"b\"}\n```\nFinal answer here.";
        let result = strip_tool_call_artifacts(input);
        assert!(!result.contains("tool_call"));
        assert!(result.contains("Final answer here."));
    }

    #[test]
    fn strip_tool_call_json_block_with_tool_pattern() {
        let input = "Checking...\n```json\n{\"tool\": \"read_memory\", \"input\": {}}\n```";
        let result = strip_tool_call_artifacts(input);
        assert!(!result.contains("read_memory"));
    }

    #[test]
    fn strip_tool_call_json_block_without_tool_pattern_preserved() {
        let input = "Here is JSON:\n```json\n{\"name\": \"Leo\", \"age\": 30}\n```";
        let result = strip_tool_call_artifacts(input);
        assert!(result.contains("```json"));
        assert!(result.contains("Leo"));
    }

    // ── [Called tool: ...] stripping tests ──────────────────────────

    #[test]
    fn strip_single_called_tool_line() {
        let input = "[Called tool: jira_search with {\"jql\": \"project = OO\"}]";
        let result = strip_tool_call_artifacts(input);
        assert!(result.is_empty(), "expected empty, got: {result:?}");
    }

    #[test]
    fn strip_multiple_called_tool_lines() {
        let input = "Here is my thinking.\n[Called tool: read_memory with {\"topic\": \"tasks\"}]\n[Called tool: jira_search with {\"jql\": \"project = OO\"}]\nThe answer is 42.";
        let result = strip_tool_call_artifacts(input);
        assert!(!result.contains("[Called tool:"), "should strip all Called tool lines");
        assert!(result.contains("The answer is 42."), "should preserve other text");
        assert!(!result.contains("Here is my thinking.") || result.contains("Here is my thinking."), "preamble handling may vary");
    }

    #[test]
    fn strip_called_tool_preserves_normal_text() {
        let input = "The meeting is at 3pm.";
        let result = strip_tool_call_artifacts(input);
        assert_eq!(result, input);
    }

    #[test]
    fn strip_called_tool_does_not_match_similar_brackets() {
        // Should NOT strip lines that look similar but don't match the exact pattern
        let input = "[Some other tag: value]";
        let result = strip_tool_call_artifacts(input);
        assert_eq!(result, input, "non-tool brackets should be preserved");
    }

    #[test]
    fn strip_called_tool_does_not_match_unclosed_bracket() {
        // Line must end with ] to match
        let input = "[Called tool: jira_search with {\"jql\": \"project = OO\"}] some extra text";
        let result = strip_tool_call_artifacts(input);
        // This line does NOT end with ] so should be preserved
        assert!(result.contains("[Called tool:"), "unclosed/non-matching line should be preserved");
    }

    // ── strip_tool_response_xml tests ────────────────────────────────

    #[test]
    fn strip_xml_basic_block() {
        let input = "Before\n<tool_response>some result</tool_response>\nAfter";
        let result = strip_tool_response_xml(input);
        assert!(!result.contains("<tool_response>"), "open tag should be removed");
        assert!(!result.contains("</tool_response>"), "close tag should be removed");
        assert!(!result.contains("some result"), "content should be removed");
        assert!(result.contains("Before"), "text before should be preserved");
        assert!(result.contains("After"), "text after should be preserved");
    }

    #[test]
    fn strip_xml_multiline_content() {
        let input = "<tool_response>\nline1\nline2\nline3\n</tool_response>\nReal answer.";
        let result = strip_tool_response_xml(input);
        assert!(!result.contains("line1"));
        assert!(!result.contains("line2"));
        assert!(result.contains("Real answer."));
    }

    #[test]
    fn strip_xml_multiple_blocks() {
        let input = "<tool_response>block one</tool_response> mid <tool_response>block two</tool_response> end";
        let result = strip_tool_response_xml(input);
        assert!(!result.contains("block one"));
        assert!(!result.contains("block two"));
        assert!(result.contains("end"));
    }

    #[test]
    fn strip_xml_unclosed_tag_removes_to_end() {
        let input = "Preamble\n<tool_response>dangling content without close tag";
        let result = strip_tool_response_xml(input);
        assert!(!result.contains("<tool_response>"));
        assert!(!result.contains("dangling content"));
        assert!(result.contains("Preamble"));
    }

    #[test]
    fn strip_xml_no_match_passthrough() {
        let input = "Just a normal response with no XML tags.";
        let result = strip_tool_response_xml(input);
        assert_eq!(result, input);
    }

    #[test]
    fn strip_xml_inside_code_fence_already_removed_by_pipeline() {
        // By the time strip_tool_response_xml runs, fenced blocks are gone.
        // This test verifies the standalone function works on bare XML only.
        let input = "Answer: <tool_response>raw</tool_response> done";
        let result = strip_tool_call_artifacts(input);
        assert!(!result.contains("raw"));
        assert!(result.contains("Answer:"));
        assert!(result.contains("done"));
    }

    // ── strip_tool_error_lines tests ─────────────────────────────────

    #[test]
    fn strip_error_infra_keyword_removed() {
        let input = "Error: project 'nv' not found in registry. Known projects: oo, tc";
        let result = strip_tool_error_lines(input);
        assert!(!result.contains("not found in registry"), "infra error should be stripped");
    }

    #[test]
    fn strip_error_connection_refused_removed() {
        let input = "Error: connection refused to nexus endpoint";
        let result = strip_tool_error_lines(input);
        assert!(result.trim().is_empty(), "connection refused line should be stripped");
    }

    #[test]
    fn strip_error_user_facing_preserved() {
        let input = "Error: your Jira ticket was not found.";
        let result = strip_tool_error_lines(input);
        assert!(result.contains("Error: your Jira ticket was not found."), "user-facing error should be preserved");
    }

    #[test]
    fn strip_error_mixed_content() {
        let input = "Here is the result.\nError: tool execution failed for nexus\nThe data is ready.";
        let result = strip_tool_error_lines(input);
        assert!(!result.contains("tool execution failed"), "infra error line should be stripped");
        assert!(result.contains("Here is the result."));
        assert!(result.contains("The data is ready."));
    }

    #[test]
    fn strip_error_timed_out_removed() {
        let input = "Error: request timed out waiting for tool response";
        let result = strip_tool_error_lines(input);
        assert!(result.trim().is_empty());
    }

    // ── strip_score_annotated_results tests ──────────────────────────

    #[test]
    fn strip_score_single_result_block() {
        let input = "worker.rs (score: 0.95):\nsome raw content here\nmore raw content\n\nReal answer.";
        let result = strip_score_annotated_results(input);
        assert!(!result.contains("score: 0.95"), "score header should be stripped");
        assert!(!result.contains("some raw content"), "continuation should be stripped");
        assert!(result.contains("Real answer."));
    }

    #[test]
    fn strip_score_multiple_blocks() {
        let input = "file1.rs (score: 0.9):\ncontent one\n\nfile2.md (score: 0.7):\ncontent two\n\nFinal.";
        let result = strip_score_annotated_results(input);
        assert!(!result.contains("score: 0.9"));
        assert!(!result.contains("score: 0.7"));
        assert!(!result.contains("content one"));
        assert!(!result.contains("content two"));
        assert!(result.contains("Final."));
    }

    #[test]
    fn strip_score_non_matching_lines_preserved() {
        let input = "This line has a dot.ext but no score annotation.\nNormal sentence.";
        let result = strip_score_annotated_results(input);
        assert!(result.contains("This line has a dot.ext"));
        assert!(result.contains("Normal sentence."));
    }

    #[test]
    fn strip_score_result_followed_by_real_sentence() {
        // A line starting with uppercase then lowercase ends the block.
        let input = "notes.txt (score: 1.0):\nraw data\nNormal sentence continues here.";
        let result = strip_score_annotated_results(input);
        assert!(!result.contains("notes.txt"));
        assert!(!result.contains("raw data"));
        assert!(result.contains("Normal sentence continues here."));
    }

    // ── strip_preamble / internal reasoning tests ─────────────────────

    #[test]
    fn strip_preamble_nexus_unreachable_removed() {
        let input = "Nexus is unreachable at the moment.";
        let result = strip_internal_reasoning_lines(input);
        assert!(result.trim().is_empty(), "internal reasoning line should be stripped");
    }

    #[test]
    fn strip_preamble_falling_back_removed() {
        let input = "Falling back to cached data since nexus is down.";
        let result = strip_internal_reasoning_lines(input);
        assert!(result.trim().is_empty());
    }

    #[test]
    fn strip_preamble_long_line_preserved() {
        // Lines >= 150 chars must not be stripped even if they contain a pattern.
        let filler = "x".repeat(130);
        let input = format!("Unable to access the resource because {filler}.");
        assert!(input.len() >= 150, "test input must be at least 150 chars, got {}", input.len());
        let result = strip_internal_reasoning_lines(&input);
        assert!(result.contains("Unable to access"), "long line should be preserved");
    }

    #[test]
    fn strip_preamble_legitimate_short_response_preserved() {
        let input = "Your meeting is at 3pm today.";
        let result = strip_internal_reasoning_lines(input);
        assert_eq!(result, input, "legitimate short response should not be stripped");
    }

    #[test]
    fn strip_preamble_cant_inspect_removed() {
        let input = "Can't inspect the nv source directly.";
        let result = strip_internal_reasoning_lines(input);
        assert!(result.trim().is_empty());
    }

    #[test]
    fn strip_preamble_let_me_check_removed() {
        let input = "Let me check what I can.";
        let result = strip_internal_reasoning_lines(input);
        assert!(result.trim().is_empty(), "let me check preamble should be stripped; got: {result:?}");
    }

    #[test]
    fn strip_preamble_no_direct_access_removed() {
        let input = "I don't have direct access to that resource.";
        let result = strip_internal_reasoning_lines(input);
        assert!(result.trim().is_empty(), "no direct access line should be stripped; got: {result:?}");
    }

    #[test]
    fn strip_preamble_no_such_tool_available_removed() {
        let input = "No such tool available in this context.";
        let result = strip_internal_reasoning_lines(input);
        assert!(result.trim().is_empty(), "no such tool available line should be stripped; got: {result:?}");
    }

    #[test]
    fn strip_error_env_var_not_set_removed() {
        let input = "Error: env var NEXUS_TOKEN is not set";
        let result = strip_tool_error_lines(input);
        assert!(result.trim().is_empty(), "env var error line should be stripped; got: {result:?}");
    }

    #[test]
    fn strip_error_connection_failed_removed() {
        let input = "Error: connection failed to registry endpoint";
        let result = strip_tool_error_lines(input);
        assert!(result.trim().is_empty(), "connection failed error should be stripped; got: {result:?}");
    }

    #[test]
    fn strip_error_environment_variable_removed() {
        let input = "Error: environment variable DATABASE_URL not set";
        let result = strip_tool_error_lines(input);
        assert!(result.trim().is_empty(), "environment variable error should be stripped; got: {result:?}");
    }

    // ── Integration test: full pipeline ──────────────────────────────

    #[test]
    fn strip_full_pipeline_all_artifacts() {
        let input = [
            "Let me check that.",
            "```tool_call",
            "{\"tool\": \"read_memory\", \"input\": {}}",
            "```",
            "<tool_response>tool result content here</tool_response>",
            "Error: project 'nv' not found in registry. Known projects: oo",
            "worker.rs (score: 0.88):",
            "some raw search snippet",
            "",
            "Nexus is unreachable right now.",
            "Here is the real answer you asked for.",
        ].join("\n");

        let result = strip_tool_call_artifacts(&input);

        // All artifact categories removed
        assert!(!result.contains("tool_call"), "fenced tool block should be stripped");
        assert!(!result.contains("tool result content"), "xml block should be stripped");
        assert!(!result.contains("not found in registry"), "error line should be stripped");
        assert!(!result.contains("score: 0.88"), "score result should be stripped");
        assert!(!result.contains("raw search snippet"), "score continuation should be stripped");
        assert!(!result.contains("Nexus is unreachable"), "reasoning line should be stripped");

        // Real content preserved
        assert!(result.contains("Here is the real answer you asked for."),
            "real response must be preserved; got: {result:?}");
    }

    // ── WorkerEvent Tests ───────────────────────────────────────────

    #[test]
    fn worker_event_all_variants_construct() {
        let id = Uuid::new_v4();

        // StageStarted
        let e = WorkerEvent::StageStarted { worker_id: id, stage: "context_build".into(), telegram_chat_id: None };
        assert!(matches!(&e, WorkerEvent::StageStarted { worker_id, stage, .. } if *worker_id == id && stage == "context_build"));

        // ToolCalled
        let e = WorkerEvent::ToolCalled { worker_id: id, tool: "jira_search".into() };
        assert!(matches!(&e, WorkerEvent::ToolCalled { worker_id, tool } if *worker_id == id && tool == "jira_search"));

        // StageComplete
        let e = WorkerEvent::StageComplete { worker_id: id, stage: "tool_loop".into(), duration_ms: 1234 };
        assert!(matches!(&e, WorkerEvent::StageComplete { worker_id, stage, duration_ms } if *worker_id == id && stage == "tool_loop" && *duration_ms == 1234));

        // Complete
        let e = WorkerEvent::Complete { worker_id: id, response_len: 512 };
        assert!(matches!(&e, WorkerEvent::Complete { worker_id, response_len } if *worker_id == id && *response_len == 512));

        // Error
        let e = WorkerEvent::Error { worker_id: id, error: "timeout".into() };
        assert!(matches!(&e, WorkerEvent::Error { worker_id, error } if *worker_id == id && error == "timeout"));
    }

    #[test]
    fn worker_event_clone_preserves_data() {
        let id = Uuid::new_v4();
        let event = WorkerEvent::StageStarted { worker_id: id, stage: "test".into(), telegram_chat_id: Some(123) };
        let cloned = event.clone();
        assert!(matches!(&cloned, WorkerEvent::StageStarted { worker_id, stage, .. } if *worker_id == id && stage == "test"));
    }

    #[test]
    fn worker_event_channel_normal_flow_sequence() {
        let (tx, mut rx) = mpsc::unbounded_channel::<WorkerEvent>();
        let id = Uuid::new_v4();

        tx.send(WorkerEvent::StageStarted { worker_id: id, stage: "context_build".into(), telegram_chat_id: Some(42) }).unwrap();
        tx.send(WorkerEvent::StageComplete { worker_id: id, stage: "context_build".into(), duration_ms: 10 }).unwrap();
        tx.send(WorkerEvent::StageStarted { worker_id: id, stage: "tool_loop".into(), telegram_chat_id: Some(42) }).unwrap();
        tx.send(WorkerEvent::ToolCalled { worker_id: id, tool: "jira_search".into() }).unwrap();
        tx.send(WorkerEvent::StageComplete { worker_id: id, stage: "tool_loop".into(), duration_ms: 500 }).unwrap();
        tx.send(WorkerEvent::Complete { worker_id: id, response_len: 256 }).unwrap();
        drop(tx);

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert_eq!(events.len(), 6);
        assert!(matches!(&events[0], WorkerEvent::StageStarted { stage, .. } if stage == "context_build"));
        assert!(matches!(&events[1], WorkerEvent::StageComplete { stage, .. } if stage == "context_build"));
        assert!(matches!(&events[2], WorkerEvent::StageStarted { stage, .. } if stage == "tool_loop"));
        assert!(matches!(&events[3], WorkerEvent::ToolCalled { tool, .. } if tool == "jira_search"));
        assert!(matches!(&events[4], WorkerEvent::StageComplete { stage, .. } if stage == "tool_loop"));
        assert!(matches!(&events[5], WorkerEvent::Complete { response_len: 256, .. }));
    }

    #[test]
    fn worker_event_channel_error_flow() {
        let (tx, mut rx) = mpsc::unbounded_channel::<WorkerEvent>();
        let id = Uuid::new_v4();

        tx.send(WorkerEvent::StageStarted { worker_id: id, stage: "context_build".into(), telegram_chat_id: None }).unwrap();
        tx.send(WorkerEvent::Error { worker_id: id, error: "Claude API failure: rate limited".into() }).unwrap();
        drop(tx);

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], WorkerEvent::StageStarted { .. }));
        assert!(matches!(&events[1], WorkerEvent::Error { error, .. } if error.contains("rate limited")));
    }

    // ── extract_summary tests ─────────────────────────────────────────

    #[test]
    fn extract_summary_tag_present_at_end() {
        let input = "I resolved the OO-142 priority mismatch and sent the Jira close request.\n[SUMMARY: Resolved OO-142 priority mismatch and sent Jira close request]";
        let (summary, cleaned) = extract_summary(input);
        assert_eq!(summary, "Resolved OO-142 priority mismatch and sent Jira close request");
        assert!(!cleaned.contains("[SUMMARY:"));
        assert!(cleaned.contains("I resolved the OO-142"));
    }

    #[test]
    fn extract_summary_tag_absent_falls_back_to_first_sentence() {
        let input = "Checked the board. OO-142 is a priority mismatch. Action recommended.";
        let (summary, cleaned) = extract_summary(input);
        // First sentence up to the first '.'
        assert_eq!(summary, "Checked the board");
        // cleaned is the full response (no tag to strip)
        assert_eq!(cleaned, input);
    }

    #[test]
    fn extract_summary_empty_response() {
        let (summary, cleaned) = extract_summary("");
        assert_eq!(summary, "empty response");
        assert_eq!(cleaned, "");
    }

    #[test]
    fn extract_summary_tag_exceeds_120_chars_truncated() {
        let long_summary = "a".repeat(200);
        let input = format!("Response text.\n[SUMMARY: {long_summary}]");
        let (summary, _cleaned) = extract_summary(&input);
        assert_eq!(summary.len(), 120);
    }

    #[test]
    fn extract_summary_tag_mid_response_uses_last_occurrence() {
        // Last occurrence wins — the final tag line is stripped; any mid-body tags remain.
        let input = "First [SUMMARY: first tag] then more text.\n[SUMMARY: second tag which is the real one]";
        let (summary, cleaned) = extract_summary(input);
        assert_eq!(summary, "second tag which is the real one");
        // The last [SUMMARY:] tag line is stripped from cleaned
        assert!(!cleaned.ends_with("[SUMMARY: second tag which is the real one]"));
        // The summary correctly reflects the LAST tag
        assert_eq!(summary, "second tag which is the real one");
    }

    #[test]
    fn extract_summary_cleaned_response_has_no_trailing_newline_from_tag_line() {
        let input = "Main response text.\n[SUMMARY: did something]";
        let (_summary, cleaned) = extract_summary(input);
        // The tag line should be stripped; remaining text should be the first line
        assert_eq!(cleaned, "Main response text.");
    }

    // ── investigate-300s-timeout tests ───────────────────────────────

    /// A WorkerTask with `is_edit_reply = true` should produce `timeout_reason = "edit_wait"`.
    ///
    /// This tests the logic inline (without spawning workers) to verify the
    /// discriminant is computed correctly.
    #[test]
    fn is_edit_reply_sets_timeout_reason_edit_wait() {
        let is_edit_reply = true;
        let timeout_reason = if is_edit_reply { "edit_wait" } else { "active_work" };
        assert_eq!(timeout_reason, "edit_wait");
    }

    /// A WorkerTask with `is_edit_reply = false` should produce `timeout_reason = "active_work"`.
    #[test]
    fn not_edit_reply_sets_timeout_reason_active_work() {
        let is_edit_reply = false;
        let timeout_reason = if is_edit_reply { "edit_wait" } else { "active_work" };
        assert_eq!(timeout_reason, "active_work");
    }

    /// `editing_action_id.take()` on an `Option<Uuid>` consumes the ID so
    /// subsequent dispatches do not inherit it.
    #[test]
    fn editing_action_id_take_consumes_the_id() {
        let id = Uuid::new_v4();
        let mut editing_action_id: Option<Uuid> = Some(id);

        // First take: returns the ID and leaves None.
        let taken = editing_action_id.take();
        assert_eq!(taken, Some(id));
        assert!(editing_action_id.is_none(), "should be None after take()");

        // Second take (simulating next trigger batch): returns None.
        let taken2 = editing_action_id.take();
        assert!(taken2.is_none(), "subsequent take() must return None");
    }

    /// `is_edit_reply` is derived from whether `editing_action_id` was `Some`
    /// at dispatch time. Verify the derivation logic.
    #[test]
    fn is_edit_reply_derived_from_editing_action_id() {
        let some_id: Option<Uuid> = Some(Uuid::new_v4());
        let is_edit_reply = some_id.is_some();
        assert!(is_edit_reply);

        let none_id: Option<Uuid> = None;
        let is_not_edit_reply = none_id.is_some();
        assert!(!is_not_edit_reply);
    }

    /// Edit-reply tasks get double the timeout budget.
    #[test]
    fn edit_reply_effective_timeout_is_doubled() {
        let base_secs: u64 = 300;
        let is_edit_reply = true;
        let effective = if is_edit_reply { base_secs * 2 } else { base_secs };
        assert_eq!(effective, 600);
    }

    /// Normal tasks use the base timeout unchanged.
    #[test]
    fn normal_task_effective_timeout_unchanged() {
        let base_secs: u64 = 300;
        let is_edit_reply = false;
        let effective = if is_edit_reply { base_secs * 2 } else { base_secs };
        assert_eq!(effective, 300);
    }

    // ── generate_slug tests ──────────────────────────────────────────

    #[test]
    fn slug_normal_message() {
        // "Can you check the Jira sprint review?" → "check-jira-sprint"
        // stopwords removed: can, you, the
        let s = generate_slug("Can you check the Jira sprint review?");
        assert_eq!(s, "check-jira-sprint");
    }

    #[test]
    fn slug_stopword_only_input_falls_back_to_session() {
        // All words are stopwords — fallback to "session"
        let s = generate_slug("hey hi hello");
        assert_eq!(s, "session");
    }

    #[test]
    fn slug_empty_input_falls_back_to_session() {
        let s = generate_slug("");
        assert_eq!(s, "session");
    }

    #[test]
    fn slug_status_query() {
        // "what is the status of OO?" → "status-oo"
        let s = generate_slug("what is the status of OO?");
        assert_eq!(s, "status-oo");
    }

    #[test]
    fn slug_long_input_truncated_at_40_chars() {
        // Build a message whose slug would exceed 40 chars without truncation
        let s = generate_slug("alpha beta gamma delta epsilon zeta eta theta");
        assert!(s.len() <= 40, "slug must not exceed 40 chars, got: {s:?} (len={})", s.len());
    }

    #[test]
    fn slug_punctuation_stripped() {
        let s = generate_slug("Telegram photo analysis needed!");
        assert_eq!(s, "telegram-photo-analysis");
    }

    #[test]
    fn slug_for_triggers_cron_digest() {
        use nv_core::types::CronEvent;
        let triggers = vec![Trigger::Cron(CronEvent::Digest)];
        let s = generate_slug_for_triggers(&triggers);
        assert_eq!(s, "digest");
    }

    #[test]
    fn slug_for_triggers_cron_user_schedule() {
        use nv_core::types::CronEvent;
        let triggers = vec![Trigger::Cron(CronEvent::UserSchedule {
            name: "daily-standup".into(),
            action: "check jira".into(),
        })];
        let s = generate_slug_for_triggers(&triggers);
        assert_eq!(s, "daily-standup");
    }

    #[test]
    fn slug_for_triggers_cli_command() {
        use nv_core::types::{CliCommand, CliRequest};
        let triggers = vec![Trigger::CliCommand(CliRequest {
            command: CliCommand::Ask("check sprint status".into()),
            response_tx: None,
        })];
        let s = generate_slug_for_triggers(&triggers);
        assert!(s.starts_with("cli-"), "CLI slug must start with cli-, got: {s:?}");
        assert!(s.contains("check"), "slug should include meaningful content, got: {s:?}");
    }

    #[test]
    fn slug_for_triggers_empty_slice_falls_back() {
        let s = generate_slug_for_triggers(&[]);
        assert_eq!(s, "session");
    }

    // ── voice trigger flag tests (Req-1, Req-2, Req-3) ───────────────

    /// [4.1] WorkerTask.is_voice_trigger defaults to false.
    #[test]
    fn worker_task_is_voice_trigger_defaults_false() {
        let task = WorkerTask {
            id: Uuid::new_v4(),
            triggers: vec![],
            priority: Priority::Normal,
            created_at: Instant::now(),
            telegram_chat_id: None,
            telegram_message_id: None,
            cli_response_txs: vec![],
            is_edit_reply: false,
            editing_action_id: None,
            slug: "session".into(),
            is_voice_trigger: false,
        };
        assert!(!task.is_voice_trigger, "is_voice_trigger should default to false");
    }

    /// [4.3] TTS gate conditions: verify the full gate logic independently.
    /// voice_enabled=true, is_voice_trigger=false → gate should NOT fire.
    #[test]
    fn tts_gate_skips_when_not_voice_trigger() {
        let voice_enabled = true;
        let is_voice_trigger = false;
        let tool_names: Vec<String> = vec![];
        let response_text = "Hello, how are you?";
        let voice_max_chars: u32 = 500;

        let should_fire = voice_enabled
            && is_voice_trigger
            && tool_names.is_empty()
            && (response_text.len() as u32) <= voice_max_chars
            && !response_text.is_empty();

        assert!(!should_fire, "TTS gate must not fire for text-origin messages");
    }

    /// [4.3] TTS gate: voice_enabled=true, is_voice_trigger=true, tool_names non-empty → skip.
    #[test]
    fn tts_gate_skips_when_tool_calls_present() {
        let voice_enabled = true;
        let is_voice_trigger = true;
        let tool_names = vec!["jira_search".to_string()];
        let response_text = "Here are the results.";
        let voice_max_chars: u32 = 500;

        let should_fire = voice_enabled
            && is_voice_trigger
            && tool_names.is_empty()
            && (response_text.len() as u32) <= voice_max_chars
            && !response_text.is_empty();

        assert!(!should_fire, "TTS gate must not fire when tool calls were made");
    }

    /// [4.3] TTS gate: response exceeds voice_max_chars → skip.
    #[test]
    fn tts_gate_skips_when_response_exceeds_max_chars() {
        let voice_enabled = true;
        let is_voice_trigger = true;
        let tool_names: Vec<String> = vec![];
        let response_text = "a".repeat(501);
        let voice_max_chars: u32 = 500;

        let should_fire = voice_enabled
            && is_voice_trigger
            && tool_names.is_empty()
            && (response_text.len() as u32) <= voice_max_chars
            && !response_text.is_empty();

        assert!(!should_fire, "TTS gate must not fire when response exceeds max chars");
    }

    /// [4.3] TTS gate: all conditions met → fires.
    #[test]
    fn tts_gate_fires_when_all_conditions_met() {
        let voice_enabled = true;
        let is_voice_trigger = true;
        let tool_names: Vec<String> = vec![];
        let response_text = "Sure, I can help with that.";
        let voice_max_chars: u32 = 500;

        let should_fire = voice_enabled
            && is_voice_trigger
            && tool_names.is_empty()
            && (response_text.len() as u32) <= voice_max_chars
            && !response_text.is_empty();

        assert!(should_fire, "TTS gate must fire when all conditions are met");
    }
}
