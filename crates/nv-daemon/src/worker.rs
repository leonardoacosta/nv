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

use anyhow::Result;
use nv_core::types::{InlineKeyboard, OutboundMessage, Trigger};
use uuid::Uuid;

use crate::agent::{
    build_system_context, check_bootstrap_state, ChannelRegistry,
};
use crate::claude::{ClaudeClient, ContentBlock, Message, StopReason, ToolDefinition, ToolResultBlock};
use crate::conversation::{ConversationStore, MAX_HISTORY_CHARS, MAX_HISTORY_TURNS};
use crate::diary::{DiaryEntry, DiaryWriter};
use crate::jira;
use crate::memory::Memory;
use crate::messages::MessageStore;
use crate::nexus;
use crate::query;
use crate::state::State;
use crate::telegram::client::TelegramClient;
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
    StageStarted { worker_id: Uuid, stage: String },
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
    pub conversation_store: Arc<std::sync::Mutex<ConversationStore>>,
    pub diary: Arc<std::sync::Mutex<DiaryWriter>>,
    pub jira_client: Option<jira::JiraClient>,
    pub nexus_client: Option<nexus::client::NexusClient>,
    pub channels: ChannelRegistry,
    pub nv_base_path: PathBuf,
    pub voice_enabled: Arc<std::sync::atomic::AtomicBool>,
    pub tts_client: Option<Arc<tts::TtsClient>>,
    pub voice_max_chars: u32,
    pub project_registry: std::collections::HashMap<String, PathBuf>,
    /// Channel for workers to emit progress events to the orchestrator.
    pub event_tx: mpsc::UnboundedSender<WorkerEvent>,
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
        let max_concurrent = self.max_concurrent;
        let client_template = self.client_template.clone();

        tokio::spawn(async move {
            let task_id = task.id;
            tracing::info!(
                worker_task = %task_id,
                priority = ?task.priority,
                triggers = task.triggers.len(),
                "worker started"
            );

            let result = Worker::run(
                task,
                Arc::clone(&deps),
                client,
                tg_client.clone(),
                tg_chat_id,
            ).await;

            if let Err(e) = &result {
                tracing::error!(worker_task = %task_id, error = %e, "worker failed");
            }

            // Release slot
            active.fetch_sub(1, Ordering::Relaxed);

            // Check queue for next task
            let next = {
                let mut q = queue.lock().unwrap();
                q.pop().map(|p| p.0)
            };
            if let Some(next_task) = next {
                let current_active = active.load(Ordering::Relaxed);
                if current_active < max_concurrent {
                    active.fetch_add(1, Ordering::Relaxed);
                    let next_client = client_template;
                    let next_active = Arc::clone(&active);
                    tokio::spawn(async move {
                        let result = Worker::run(
                            next_task,
                            deps,
                            next_client,
                            tg_client,
                            tg_chat_id,
                        ).await;
                        if let Err(e) = result {
                            tracing::error!(error = %e, "queued worker failed");
                        }
                        next_active.fetch_sub(1, Ordering::Relaxed);
                    });
                } else {
                    // Put it back
                    queue.lock().unwrap().push(PrioritizedTask(next_task));
                }
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
        task: WorkerTask,
        deps: Arc<SharedDeps>,
        client: ClaudeClient,
        tg_client: Option<TelegramClient>,
        default_chat_id: Option<i64>,
    ) -> Result<()> {
        let task_start = Instant::now();
        let task_id = task.id;
        let tg_chat_id = task.telegram_chat_id.or(default_chat_id);
        let tg_msg_id = task.telegram_message_id;
        let event_tx = &deps.event_tx;

        // Extract reply_to message ID from the first trigger (for threading)
        let reply_to_id: Option<String> = task.triggers.first().and_then(|t| match t {
            Trigger::Message(msg) => Some(msg.id.clone()),
            _ => None,
        });

        // Send typing indicator immediately (fire-and-forget)
        if let (Some(tg), Some(chat_id)) = (&tg_client, tg_chat_id) {
            tg.send_chat_action(chat_id, "typing").await;
        }

        // Emit StageStarted for context build
        let context_build_start = Instant::now();
        let _ = event_tx.send(WorkerEvent::StageStarted {
            worker_id: task_id,
            stage: "context_build".into(),
        });

        // Build system context and tool definitions
        let system_prompt = build_system_context();
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

        // Format triggers
        let trigger_text = crate::agent::format_trigger_batch(&task.triggers);

        // Log inbound messages to the message store
        for trigger in &task.triggers {
            if let Trigger::Message(msg) = trigger {
                let store = deps.message_store.lock().unwrap();
                if let Err(e) = store.log_inbound(
                    &msg.channel,
                    &msg.sender,
                    &msg.content,
                    "message",
                ) {
                    tracing::warn!(error = %e, "failed to log inbound message");
                }
            }
        }

        // Load recent messages context
        let recent_messages_context = {
            let store = deps.message_store.lock().unwrap();
            match store.format_recent_for_context(20) {
                Ok(ctx) if !ctx.is_empty() => Some(ctx),
                Ok(_) => None,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to load recent messages context");
                    None
                }
            }
        };

        // Load memory context
        let memory_context = match deps.memory.get_context_summary_for(&trigger_text) {
            Ok(ctx) if !ctx.is_empty() => Some(ctx),
            Ok(_) => None,
            Err(e) => {
                tracing::warn!(error = %e, "failed to load memory context");
                None
            }
        };

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

        // Load prior conversation turns from the shared store
        let prior_turns = {
            let mut store = deps.conversation_store.lock().unwrap();
            store.load()
        };

        let mut conversation_history = prior_turns;
        conversation_history.push(Message::user(&user_message));

        // Emit StageComplete for context build
        let _ = event_tx.send(WorkerEvent::StageComplete {
            worker_id: task_id,
            stage: "context_build".into(),
            duration_ms: context_build_start.elapsed().as_millis() as u64,
        });

        // Call Claude
        let call_start = Instant::now();
        let response = match client
            .send_messages(&system_prompt, &conversation_history, &tool_definitions)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                // Emit Error event
                let _ = event_tx.send(WorkerEvent::Error {
                    worker_id: task_id,
                    error: format!("Claude API failure: {e}"),
                });
                // React with error
                if let (Some(tg), Some(chat_id), Some(msg_id)) = (&tg_client, tg_chat_id, tg_msg_id) {
                    let _ = tg.set_message_reaction(chat_id, msg_id, "\u{274C}").await; // red X
                }
                // Send error to Telegram (threaded to original message)
                if let Some(channel) = deps.channels.get("telegram") {
                    let msg = OutboundMessage {
                        channel: "telegram".into(),
                        content: format!("\u{26A0} Worker error: {e}"),
                        reply_to: reply_to_id.clone(),
                        keyboard: None,
                    };
                    let _ = channel.send_message(msg).await;
                }
                // Send error to CLI channels
                let err_msg = format!("API error: {e}");
                for tx in task.cli_response_txs {
                    let _ = tx.send(err_msg.clone());
                }
                return Err(e);
            }
        };

        let response_time_ms = call_start.elapsed().as_millis() as i64;
        let tokens_in = response.usage.input_tokens as i64;
        let tokens_out = response.usage.output_tokens as i64;

        // Run tool loop
        let tool_loop_start = Instant::now();
        let _ = event_tx.send(WorkerEvent::StageStarted {
            worker_id: task_id,
            stage: "tool_loop".into(),
        });

        let (final_content, tool_names) = Self::run_tool_loop(
            response,
            &mut conversation_history,
            &system_prompt,
            &tool_definitions,
            &client,
            &deps,
            task_id,
        )
        .await?;

        let _ = event_tx.send(WorkerEvent::StageComplete {
            worker_id: task_id,
            stage: "tool_loop".into(),
            duration_ms: tool_loop_start.elapsed().as_millis() as u64,
        });

        let response_text = extract_text(&final_content);

        // Push the completed turn (user + assistant) to the conversation store
        {
            let user_msg_for_store = Message::user(&user_message);
            let assistant_msg_for_store = Message::assistant_blocks(final_content.clone());
            let mut conv_store = deps.conversation_store.lock().unwrap();
            conv_store.push(user_msg_for_store, assistant_msg_for_store);
        }

        // Emit Complete event
        let _ = event_tx.send(WorkerEvent::Complete {
            worker_id: task_id,
            response_len: response_text.len(),
        });

        // Send response to CLI channels
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

        if !response_text.is_empty() {
            if let Some(channel) = deps.channels.get(reply_channel) {
                if let Err(e) = channel
                    .send_message(OutboundMessage {
                        channel: reply_channel.to_string(),
                        content: response_text.clone(),
                        reply_to: reply_to_id.clone(),
                        keyboard: None,
                    })
                    .await
                {
                    tracing::error!(error = %e, "failed to route worker response");
                }
            }
        }

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
        let result_summary = if response_text.is_empty() {
            "empty response".to_string()
        } else {
            let truncated: String = response_text.chars().take(80).collect();
            if response_text.len() > 80 {
                format!("{truncated}...")
            } else {
                truncated
            }
        };
        let sources_checked = summarize_sources(&tool_names);

        let diary_entry = DiaryEntry {
            timestamp: chrono::Local::now(),
            trigger_type,
            trigger_source,
            trigger_count: task.triggers.len(),
            tools_called: tool_names.clone(),
            sources_checked,
            result_summary,
            tokens_in: tokens_in as u32,
            tokens_out: tokens_out as u32,
        };
        {
            let diary = deps.diary.lock().unwrap();
            if let Err(e) = diary.write_entry(&diary_entry) {
                tracing::warn!(error = %e, "failed to write diary entry");
            }
        }

        // Voice delivery
        if deps.voice_enabled.load(std::sync::atomic::Ordering::Relaxed)
            && (response_text.len() as u32) <= deps.voice_max_chars
            && !response_text.is_empty()
        {
            if let Some(tts_client) = &deps.tts_client {
                if let Some(tg) = deps.channels.get("telegram") {
                    if let Some(tg_channel) =
                        tg.as_any().downcast_ref::<crate::telegram::TelegramChannel>()
                    {
                        let tts_c = Arc::clone(tts_client);
                        let tg_client_voice = tg_channel.client.clone();
                        let chat_id = tg_channel.chat_id;
                        let text_for_tts = response_text.clone();
                        let reply_to_id = tg_msg_id;

                        tokio::spawn(async move {
                            match tts::synthesize(&tts_c, &text_for_tts).await {
                                Ok(ogg_bytes) => {
                                    if let Err(e) = tg_client_voice
                                        .send_voice(chat_id, ogg_bytes, reply_to_id)
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

    /// Execute the tool use loop for a worker.
    async fn run_tool_loop(
        initial_response: crate::claude::ApiResponse,
        conversation_history: &mut Vec<Message>,
        system_prompt: &str,
        tool_definitions: &[ToolDefinition],
        client: &ClaudeClient,
        deps: &SharedDeps,
        worker_id: Uuid,
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
                } else {
                    // Determine timeout based on tool category
                    let timeout_secs = if WRITE_TOOLS.contains(&name.as_str()) {
                        TOOL_TIMEOUT_WRITE
                    } else {
                        TOOL_TIMEOUT_READ
                    };
                    let timeout_dur = Duration::from_secs(timeout_secs);

                    // Wrap tool execution in a timeout
                    match tokio::time::timeout(
                        timeout_dur,
                        tools::execute_tool_send(
                            name,
                            input,
                            &deps.memory,
                            deps.jira_client.as_ref(),
                            deps.nexus_client.as_ref(),
                            &deps.project_registry,
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
                            Err(anyhow::anyhow!(
                                "Tool timed out after {timeout_secs}s"
                            ))
                        }
                    }
                };
                let tool_duration_ms = tool_start.elapsed().as_millis() as i64;

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
                                tg.as_any().downcast_ref::<crate::telegram::TelegramChannel>()
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

            truncate_history(conversation_history);

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

/// Remove ```tool_call...``` / ```tool_use...``` / ```json...``` blocks and
/// the preamble text that often precedes them ("Let me check that.", etc.)
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

    result.trim().to_string()
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
        ];
        for pattern in &preamble_patterns {
            if lower.starts_with(pattern) {
                return String::new();
            }
        }
    }

    trimmed.to_string()
}

/// Truncate conversation history to stay within context budget.
fn truncate_history(history: &mut Vec<Message>) {
    if history.len() > MAX_HISTORY_TURNS {
        let drain_count = history.len() - MAX_HISTORY_TURNS;
        history.drain(..drain_count);
    }

    let mut total_chars: usize = history.iter().map(|m| m.content_len()).sum();

    while total_chars > MAX_HISTORY_CHARS && history.len() > 2 {
        if let Some(removed) = history.first() {
            total_chars -= removed.content_len();
        }
        history.remove(0);
    }
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
        });
        let normal = PrioritizedTask(WorkerTask {
            id: Uuid::new_v4(),
            triggers: vec![],
            priority: Priority::Normal,
            created_at: Instant::now(),
            telegram_chat_id: None,
            telegram_message_id: None,
            cli_response_txs: vec![],
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
        });
        let task2 = PrioritizedTask(WorkerTask {
            id: Uuid::new_v4(),
            triggers: vec![],
            priority: Priority::Normal,
            created_at: later,
            telegram_chat_id: None,
            telegram_message_id: None,
            cli_response_txs: vec![],
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

    // ── WorkerEvent Tests ───────────────────────────────────────────

    #[test]
    fn worker_event_all_variants_construct() {
        let id = Uuid::new_v4();

        // StageStarted
        let e = WorkerEvent::StageStarted { worker_id: id, stage: "context_build".into() };
        assert!(matches!(&e, WorkerEvent::StageStarted { worker_id, stage } if *worker_id == id && stage == "context_build"));

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
        let event = WorkerEvent::StageStarted { worker_id: id, stage: "test".into() };
        let cloned = event.clone();
        assert!(matches!(&cloned, WorkerEvent::StageStarted { worker_id, stage } if *worker_id == id && stage == "test"));
    }

    #[test]
    fn worker_event_channel_normal_flow_sequence() {
        let (tx, mut rx) = mpsc::unbounded_channel::<WorkerEvent>();
        let id = Uuid::new_v4();

        tx.send(WorkerEvent::StageStarted { worker_id: id, stage: "context_build".into() }).unwrap();
        tx.send(WorkerEvent::StageComplete { worker_id: id, stage: "context_build".into(), duration_ms: 10 }).unwrap();
        tx.send(WorkerEvent::StageStarted { worker_id: id, stage: "tool_loop".into() }).unwrap();
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

        tx.send(WorkerEvent::StageStarted { worker_id: id, stage: "context_build".into() }).unwrap();
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
}
