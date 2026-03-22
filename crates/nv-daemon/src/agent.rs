use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use nv_core::channel::Channel;
use nv_core::config::AgentConfig;
use nv_core::types::{
    ActionStatus, InlineKeyboard, OutboundMessage, PendingAction, Trigger,
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::claude::{
    ApiError, ApiResponse, ClaudeClient, ContentBlock, Message, StopReason, ToolDefinition,
    ToolResultBlock,
};
use crate::jira;
use crate::memory::Memory;
use crate::nexus;
use crate::query;
use crate::state::State;
use crate::tools;

// ── Constants ───────────────────────────────────────────────────────

/// Maximum number of conversation turns to keep in history.
const MAX_HISTORY_TURNS: usize = 20;

/// Maximum total characters across conversation history.
const MAX_HISTORY_CHARS: usize = 50_000;

/// Clear conversation history after this much inactivity.
const SESSION_TIMEOUT: Duration = Duration::from_secs(600); // 10 minutes

/// Maximum tool use loop iterations per agent cycle (safety limit).
const MAX_TOOL_LOOP_ITERATIONS: usize = 10;

// ── System Prompt ───────────────────────────────────────────────────

/// Default system prompt compiled into the binary.
/// Can be overridden by `~/.nv/system-prompt.md`.
const DEFAULT_SYSTEM_PROMPT: &str = r#"You are NV, a task-focused agent harness for Leo. You are NOT a chatbot — you are an operations assistant that monitors systems, manages Jira, and provides cross-system context.

## Identity
- Name: NV
- Operator: Leo (solo user, power user)
- Primary channel: Telegram

## Autonomy Rules
- READ operations: Execute immediately (memory, Jira search, Nexus query)
- WRITE operations: ALWAYS draft first, present for confirmation via pending action
- Never create/modify/transition Jira issues without explicit confirmation
- Memory writes (storing context) are autonomous — no confirmation needed

## Available Tools
You have access to these tools. Use them proactively when relevant:
- read_memory(topic): Read a specific memory file
- search_memory(query): Search across all memory files
- write_memory(topic, content): Store information for future reference
- jira_search(jql): Search Jira issues with JQL (immediate)
- jira_get(issue_key): Get full issue details including comments (immediate)
- jira_create(project, issue_type, title, ...): Create a Jira issue (requires confirmation)
- jira_transition(issue_key, transition_name): Transition issue status (requires confirmation)
- jira_assign(issue_key, assignee): Assign an issue (requires confirmation)
- jira_comment(issue_key, body): Add a comment to an issue (requires confirmation)
- query_nexus(): Get running Nexus session status

## Response Format
Respond conversationally but concisely. When you need to take an action:
1. Describe what you want to do
2. If it's a write operation, say "I'll draft this for your confirmation"
3. Use the appropriate tool

## Intent Classification
Every incoming message falls into one of three categories. Classify before acting:
- **Query**: Questions seeking information ("What's blocking OO?", "Status of TC?", "How many open issues?", "What did we decide about X?"). Use tools to gather data, then synthesize a direct answer with source attribution in [Source: ref] format.
- **Command**: Requests to change state ("Create a P1 bug for checkout crash", "Assign OO-142 to me", "Transition OO-42 to Done"). Draft the action and present for confirmation via pending action.
- **Chat**: Conversational messages ("Thanks NV", "Good morning", "Got it"). Reply conversationally and briefly.

## Query Handling
When you identify a query:
1. Use tools proactively to gather relevant data (jira_search, read_memory, search_memory, jira_get, query_nexus)
2. Synthesize a direct, factual answer — do not hedge or speculate
3. Include source attribution: cite with [Jira: OO-142], [Memory: decisions.md], [Nexus: session-abc]
4. If data is missing, say so explicitly rather than guessing
5. Suggest 1-3 follow-up actions the user might want to take

## Follow-Up Context
When provided with <followup_context> tags, the user may be referencing a previous query answer. Look for references like "do the first one", "assign that to me", "yes do it" and map them to the suggested follow-up actions.

## Context
You receive triggers from multiple sources (Telegram messages, cron events, Nexus events, CLI commands). Process them in priority order. Multiple triggers may arrive at once — batch your reasoning."#;

/// Load the system prompt — override from file, or fall back to default.
pub fn load_system_prompt() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let override_path = std::path::Path::new(&home).join(".nv").join("system-prompt.md");
    if let Ok(contents) = std::fs::read_to_string(&override_path) {
        tracing::info!(path = %override_path.display(), "loaded custom system prompt");
        contents
    } else {
        tracing::debug!("using default system prompt");
        DEFAULT_SYSTEM_PROMPT.to_string()
    }
}

// ── Channel Registry ────────────────────────────────────────────────

/// Maps channel names to their implementations for outbound routing.
pub type ChannelRegistry = HashMap<String, Arc<dyn Channel>>;

// ── Agent Loop ──────────────────────────────────────────────────────

pub struct AgentLoop {
    #[allow(dead_code)]
    config: AgentConfig,
    client: ClaudeClient,
    trigger_rx: mpsc::UnboundedReceiver<Trigger>,
    channels: ChannelRegistry,
    memory: Memory,
    state: State,
    jira_client: Option<jira::JiraClient>,
    nexus_client: Option<nexus::client::NexusClient>,
    conversation_history: Vec<Message>,
    system_prompt: String,
    tool_definitions: Vec<ToolDefinition>,
    last_activity: Instant,
    followup_manager: query::followup::FollowUpManager,
}

impl AgentLoop {
    pub fn new(
        config: AgentConfig,
        client: ClaudeClient,
        trigger_rx: mpsc::UnboundedReceiver<Trigger>,
        channels: ChannelRegistry,
        nv_base_path: PathBuf,
        jira_client: Option<jira::JiraClient>,
        nexus_client: Option<nexus::client::NexusClient>,
    ) -> Self {
        let system_prompt = load_system_prompt();
        let tool_definitions = tools::register_tools();
        let memory = Memory::new(&nv_base_path);
        let state = State::new(&nv_base_path);
        let followup_manager = query::followup::FollowUpManager::new(&nv_base_path);

        tracing::info!(
            tools = tool_definitions.len(),
            jira_enabled = jira_client.is_some(),
            nexus_enabled = nexus_client.is_some(),
            system_prompt_len = system_prompt.len(),
            "agent loop initialized"
        );

        Self {
            config,
            client,
            trigger_rx,
            channels,
            memory,
            state,
            jira_client,
            nexus_client,
            conversation_history: Vec::new(),
            system_prompt,
            tool_definitions,
            last_activity: Instant::now(),
            followup_manager,
        }
    }

    /// Main agent loop — drains triggers, calls Claude, routes responses.
    ///
    /// Runs until the trigger channel closes (all senders dropped).
    pub async fn run(mut self) -> Result<()> {
        tracing::info!("agent loop started, waiting for triggers");

        loop {
            let mut triggers = self.drain_triggers().await;
            if triggers.is_empty() {
                tracing::info!("trigger channel closed, shutting down agent loop");
                break;
            }

            // Extract CLI response channels before processing (triggers are not Clone)
            let cli_response_txs = extract_cli_response_channels(&mut triggers);

            // Send direct Telegram notifications for Nexus events
            // (bypasses Claude — these are informational)
            for trigger in &triggers {
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

            // Check session timeout — clear history if stale
            self.maybe_reset_session();

            // Load memory context to inject before the user message
            let memory_context = match self.memory.get_context_summary() {
                Ok(ctx) if !ctx.is_empty() => Some(ctx),
                Ok(_) => None,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to load memory context");
                    None
                }
            };

            // Load follow-up context if available
            let followup_context = match self.followup_manager.load() {
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

            // Format the trigger batch into a user message
            let trigger_text = format_trigger_batch(&triggers);
            tracing::debug!(trigger_text = %trigger_text, "formatted trigger batch");

            // Build the full user message with context injections
            let mut user_message = String::new();

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

            self.conversation_history.push(Message::user(&user_message));

            // Truncate history if needed
            truncate_history(&mut self.conversation_history);

            // Call Claude API
            match self
                .client
                .send_messages(
                    &self.system_prompt,
                    &self.conversation_history,
                    &self.tool_definitions,
                )
                .await
            {
                Ok(response) => {
                    if response.stop_reason == StopReason::MaxTokens {
                        tracing::warn!("Claude response hit max_tokens — response may be partial");
                    }

                    // Run tool loop if needed
                    match self.run_tool_loop(response).await {
                        Ok(final_content) => {
                            let response_text = extract_text(&final_content);

                            // Send response to CLI channels if any
                            for tx in cli_response_txs {
                                let _ = tx.send(response_text.clone());
                            }

                            // Route response to appropriate channel (Telegram, etc.)
                            if let Err(e) = self.route_response(&final_content, &triggers).await {
                                tracing::error!(error = %e, "failed to route response");
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "tool loop failed");
                            let err_msg = format!("Tool loop error: {e}");

                            // Send error to CLI channels if any
                            for tx in cli_response_txs {
                                let _ = tx.send(err_msg.clone());
                            }

                            self.send_error_to_telegram(&err_msg).await;
                        }
                    }

                    self.last_activity = Instant::now();
                }
                Err(e) => {
                    tracing::error!(error = %e, "Claude API call failed");

                    // Send error to CLI channels if any
                    let err_msg = format!("API error: {e}");
                    for tx in cli_response_txs {
                        let _ = tx.send(err_msg.clone());
                    }

                    self.handle_api_error(&e, &triggers).await;
                }
            }
        }

        Ok(())
    }

    // ── Trigger Draining ────────────────────────────────────────────

    /// Block until at least one trigger arrives, then drain all queued triggers.
    async fn drain_triggers(&mut self) -> Vec<Trigger> {
        // Block until the first trigger arrives
        let first = self.trigger_rx.recv().await;
        let Some(first) = first else {
            return vec![]; // Channel closed — signal shutdown
        };

        let mut batch = vec![first];

        // Non-blocking drain of any additional queued triggers
        while let Ok(trigger) = self.trigger_rx.try_recv() {
            batch.push(trigger);
        }

        tracing::info!(count = batch.len(), "drained trigger batch");
        batch
    }

    // ── Session Management ──────────────────────────────────────────

    /// Clear conversation history if the session has been inactive too long.
    fn maybe_reset_session(&mut self) {
        if self.last_activity.elapsed() > SESSION_TIMEOUT {
            let old_len = self.conversation_history.len();
            self.conversation_history.clear();
            tracing::info!(
                old_turns = old_len,
                "session timeout — cleared conversation history"
            );
        }
    }

    // ── Tool Use Loop ───────────────────────────────────────────────

    /// Execute the tool use loop: when Claude returns tool_use, execute
    /// the tools and send results back until Claude produces a final response.
    async fn run_tool_loop(
        &mut self,
        initial_response: ApiResponse,
    ) -> Result<Vec<ContentBlock>> {
        let mut response = initial_response;
        let mut all_text_content = Vec::new();

        for iteration in 0..MAX_TOOL_LOOP_ITERATIONS {
            // Separate text and tool_use blocks
            let mut tool_uses = Vec::new();
            for block in &response.content {
                match block {
                    ContentBlock::Text { .. } => all_text_content.push(block.clone()),
                    ContentBlock::ToolUse { id, name, input } => {
                        tool_uses.push((id.clone(), name.clone(), input.clone()));
                    }
                    ContentBlock::ToolResult { .. } => {} // shouldn't appear in response
                }
            }

            // If no tool uses or stop reason isn't tool_use, we're done
            if tool_uses.is_empty() || response.stop_reason != StopReason::ToolUse {
                // Capture any remaining text from the final response
                for block in &response.content {
                    if let ContentBlock::Text { .. } = block {
                        // Already captured above
                    }
                }
                break;
            }

            tracing::info!(
                iteration,
                tool_count = tool_uses.len(),
                tools = ?tool_uses.iter().map(|(_, n, _)| n.as_str()).collect::<Vec<_>>(),
                "executing tool use cycle"
            );

            // Add assistant response (with tool_use blocks) to history
            self.conversation_history
                .push(Message::assistant_blocks(response.content.clone()));

            // Execute each tool and collect results
            let mut tool_results = Vec::new();
            for (id, name, input) in &tool_uses {
                let result = tools::execute_tool(
                    name,
                    input,
                    &self.memory,
                    self.jira_client.as_ref(),
                    self.nexus_client.as_ref(),
                )
                .await;

                let (content, is_error) = match result {
                    Ok(tools::ToolResult::Immediate(output)) => (output, false),
                    Ok(tools::ToolResult::PendingAction {
                        description,
                        action_type,
                        payload,
                    }) => {
                        // Create and persist the pending action
                        let action = PendingAction {
                            id: Uuid::new_v4(),
                            description: description.clone(),
                            action_type,
                            payload,
                            status: ActionStatus::Pending,
                            created_at: chrono::Utc::now(),
                        };

                        if let Err(e) = self.state.save_pending_action(
                            &crate::state::PendingAction {
                                id: action.id,
                                description: action.description.clone(),
                                payload: action.payload.clone(),
                                status: crate::state::PendingStatus::AwaitingConfirmation,
                                created_at: action.created_at,
                            },
                        ) {
                            tracing::error!(error = %e, "failed to save pending action");
                        }

                        // Send Telegram confirmation keyboard
                        let keyboard = InlineKeyboard::confirm_action(&action.id.to_string());
                        if let Some(channel) = self.channels.get("telegram") {
                            let msg = OutboundMessage {
                                channel: "telegram".into(),
                                content: format!(
                                    "Pending action:\n{description}\n\nApprove, edit, or cancel?"
                                ),
                                reply_to: None,
                                keyboard: Some(keyboard),
                            };
                            if let Err(e) = channel.send_message(msg).await {
                                tracing::error!(error = %e, "failed to send confirmation keyboard");
                            }
                        }

                        (
                            format!("Action queued for confirmation: {description}"),
                            false,
                        )
                    }
                    Err(e) => (format!("Error: {e}"), true),
                };

                tracing::debug!(
                    tool = %name,
                    is_error,
                    result_len = content.len(),
                    "tool execution complete"
                );

                tool_results.push(ToolResultBlock {
                    tool_use_id: id.clone(),
                    content,
                    is_error,
                });
            }

            // Add tool results as user message to history
            self.conversation_history
                .push(Message::tool_results(tool_results));

            // Truncate before next API call
            truncate_history(&mut self.conversation_history);

            // Send continued conversation back to Claude
            response = self
                .client
                .send_messages(
                    &self.system_prompt,
                    &self.conversation_history,
                    &self.tool_definitions,
                )
                .await?;

            if response.stop_reason == StopReason::MaxTokens {
                tracing::warn!("Claude response hit max_tokens during tool loop");
            }
        }

        Ok(all_text_content)
    }

    // ── Response Routing ────────────────────────────────────────────

    /// Route the final response to the appropriate channel.
    async fn route_response(
        &self,
        content: &[ContentBlock],
        source_triggers: &[Trigger],
    ) -> Result<()> {
        let text: String = content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        if text.is_empty() {
            tracing::debug!("empty response from Claude, nothing to route");
            return Ok(());
        }

        // Determine the reply channel from the source trigger
        let reply_channel = source_triggers
            .first()
            .and_then(|t| match t {
                Trigger::Message(msg) => Some(msg.channel.as_str()),
                _ => None,
            })
            .unwrap_or("telegram");

        // Determine reply_to from the source trigger
        let reply_to = source_triggers.first().and_then(|t| match t {
            Trigger::Message(msg) => Some(msg.id.clone()),
            _ => None,
        });

        // Send the response via the appropriate channel
        if let Some(channel) = self.channels.get(reply_channel) {
            channel
                .send_message(OutboundMessage {
                    channel: reply_channel.to_string(),
                    content: text,
                    reply_to,
                    keyboard: None,
                })
                .await?;
        } else {
            tracing::warn!(channel = reply_channel, "no channel registered for routing");
        }

        Ok(())
    }

    // ── Error Handling ──────────────────────────────────────────────

    /// Handle API errors — notify via Telegram for persistent failures.
    async fn handle_api_error(&self, error: &anyhow::Error, _triggers: &[Trigger]) {
        // Check if it's an auth error (non-retryable)
        if let Some(api_err) = error.downcast_ref::<ApiError>() {
            match api_err {
                ApiError::AuthError(..) => {
                    tracing::error!("Authentication failed — check Claude CLI login");
                    self.send_error_to_telegram(
                        "NV authentication failed. Run `claude login` on the host.",
                    )
                    .await;
                }
                _ => {
                    self.send_error_to_telegram(&format!("NV API error: {error}"))
                        .await;
                }
            }
        } else {
            self.send_error_to_telegram(&format!("NV error: {error}"))
                .await;
        }
    }

    /// Send an error notification to Telegram.
    async fn send_error_to_telegram(&self, message: &str) {
        if let Some(channel) = self.channels.get("telegram") {
            let msg = OutboundMessage {
                channel: "telegram".into(),
                content: format!("⚠ {message}"),
                reply_to: None,
                keyboard: None,
            };
            if let Err(e) = channel.send_message(msg).await {
                tracing::error!(error = %e, "failed to send error notification to Telegram");
            }
        }
    }
}

// ── Trigger Formatting ──────────────────────────────────────────────

/// Format a batch of triggers into a structured text message for Claude.
fn format_trigger_batch(triggers: &[Trigger]) -> String {
    let mut parts = Vec::new();
    for trigger in triggers {
        match trigger {
            Trigger::Message(msg) => {
                parts.push(format!(
                    "[{}] {} from @{}: {}",
                    msg.channel,
                    msg.timestamp.format("%H:%M"),
                    msg.sender,
                    msg.content
                ));
            }
            Trigger::Cron(event) => {
                parts.push(format!("[cron] {event:?} triggered"));
            }
            Trigger::NexusEvent(event) => {
                parts.push(format!(
                    "[nexus] {} session {} — {:?}{}",
                    event.agent_name,
                    event.session_id,
                    event.event_type,
                    event
                        .details
                        .as_ref()
                        .map(|d| format!(": {d}"))
                        .unwrap_or_default()
                ));
            }
            Trigger::CliCommand(req) => {
                parts.push(format!("[cli] {:?}", req.command));
            }
        }
    }
    parts.join("\n")
}

// ── Context Window Management ───────────────────────────────────────

/// Truncate conversation history to stay within context budget.
///
/// Enforces both a turn count limit and a character budget.
/// Always keeps at least the 2 most recent turns.
fn truncate_history(history: &mut Vec<Message>) {
    // Keep at most MAX_HISTORY_TURNS turns
    if history.len() > MAX_HISTORY_TURNS {
        let drain_count = history.len() - MAX_HISTORY_TURNS;
        history.drain(..drain_count);
    }

    // If still too large by character count, drop oldest turns
    let mut total_chars: usize = history.iter().map(|m| m.content_len()).sum();

    while total_chars > MAX_HISTORY_CHARS && history.len() > 2 {
        if let Some(removed) = history.first() {
            total_chars -= removed.content_len();
        }
        history.remove(0);
    }
}

// ── CLI Response Channel Extraction ─────────────────────────────────

/// Extract oneshot response senders from CLI triggers.
///
/// Takes ownership of the `response_tx` from each `CliCommand` trigger,
/// leaving `None` in the trigger (since triggers are consumed by value
/// in the format step anyway). Returns the senders so the agent loop
/// can send the final response back to the HTTP handler.
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

/// Extract text content from a list of content blocks.
fn extract_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use nv_core::types::{
        CliCommand, CliRequest, CronEvent, InboundMessage, SessionEvent, SessionEventType,
    };

    #[test]
    fn format_trigger_batch_message() {
        let triggers = vec![Trigger::Message(InboundMessage {
            id: "1".into(),
            channel: "telegram".into(),
            sender: "leo".into(),
            content: "hello NV".into(),
            timestamp: Utc::now(),
            thread_id: None,
            metadata: serde_json::json!({}),
        })];

        let text = format_trigger_batch(&triggers);
        assert!(text.contains("[telegram]"));
        assert!(text.contains("@leo"));
        assert!(text.contains("hello NV"));
    }

    #[test]
    fn format_trigger_batch_cron() {
        let triggers = vec![Trigger::Cron(CronEvent::Digest)];
        let text = format_trigger_batch(&triggers);
        assert!(text.contains("[cron]"));
        assert!(text.contains("Digest"));
    }

    #[test]
    fn format_trigger_batch_nexus() {
        let triggers = vec![Trigger::NexusEvent(SessionEvent {
            agent_name: "builder".into(),
            session_id: "s-1".into(),
            event_type: SessionEventType::Completed,
            details: Some("all tests passed".into()),
        })];
        let text = format_trigger_batch(&triggers);
        assert!(text.contains("[nexus]"));
        assert!(text.contains("builder"));
        assert!(text.contains("Completed"));
        assert!(text.contains("all tests passed"));
    }

    #[test]
    fn format_trigger_batch_cli() {
        let triggers = vec![Trigger::CliCommand(CliRequest {
            command: CliCommand::Status,
            response_tx: None,
        })];
        let text = format_trigger_batch(&triggers);
        assert!(text.contains("[cli]"));
        assert!(text.contains("Status"));
    }

    #[test]
    fn format_trigger_batch_multiple() {
        let triggers = vec![
            Trigger::Message(InboundMessage {
                id: "1".into(),
                channel: "telegram".into(),
                sender: "leo".into(),
                content: "first".into(),
                timestamp: Utc::now(),
                thread_id: None,
                metadata: serde_json::json!({}),
            }),
            Trigger::Message(InboundMessage {
                id: "2".into(),
                channel: "telegram".into(),
                sender: "leo".into(),
                content: "second".into(),
                timestamp: Utc::now(),
                thread_id: None,
                metadata: serde_json::json!({}),
            }),
            Trigger::Cron(CronEvent::MemoryCleanup),
        ];

        let text = format_trigger_batch(&triggers);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("first"));
        assert!(lines[1].contains("second"));
        assert!(lines[2].contains("MemoryCleanup"));
    }

    #[test]
    fn truncate_history_under_limit() {
        let mut history = vec![
            Message::user("hello"),
            Message::user("world"),
        ];
        truncate_history(&mut history);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn truncate_history_over_turn_limit() {
        let mut history: Vec<Message> = (0..30)
            .map(|i| Message::user(format!("message {i}")))
            .collect();
        truncate_history(&mut history);
        assert_eq!(history.len(), MAX_HISTORY_TURNS);
        // Should keep the newest messages
        match &history.last().unwrap().content {
            crate::claude::MessageContent::Text(t) => assert_eq!(t, "message 29"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn truncate_history_over_char_limit() {
        // Create messages that exceed MAX_HISTORY_CHARS
        let big_msg = "x".repeat(20_000);
        let mut history = vec![
            Message::user(big_msg.clone()),
            Message::user(big_msg.clone()),
            Message::user(big_msg.clone()),
            Message::user("recent message"),
        ];
        truncate_history(&mut history);
        // Should have dropped old messages but kept at least 2
        assert!(history.len() >= 2);
        let total: usize = history.iter().map(|m| m.content_len()).sum();
        assert!(total <= MAX_HISTORY_CHARS || history.len() == 2);
    }

    #[test]
    fn truncate_history_keeps_minimum_two() {
        let big_msg = "x".repeat(40_000);
        let mut history = vec![
            Message::user(big_msg.clone()),
            Message::user(big_msg),
        ];
        truncate_history(&mut history);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn load_system_prompt_returns_default() {
        // In a test environment without ~/.nv/system-prompt.md, should return default
        let prompt = load_system_prompt();
        assert!(prompt.contains("NV"));
        assert!(prompt.contains("Leo"));
        assert!(prompt.contains("Autonomy Rules"));
    }

    #[tokio::test]
    async fn drain_triggers_single() {
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        let mut agent_rx = rx;

        tx.send(Trigger::Cron(CronEvent::Digest)).unwrap();
        drop(tx);

        // Manually drain
        let first = agent_rx.recv().await.unwrap();
        let mut batch = vec![first];
        loop {
            match agent_rx.try_recv() {
                Ok(trigger) => batch.push(trigger),
                Err(_) => break,
            }
        }
        assert_eq!(batch.len(), 1);
    }

    #[tokio::test]
    async fn drain_triggers_batch() {
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        let mut agent_rx = rx;

        // Send 5 triggers before draining
        for _ in 0..5 {
            tx.send(Trigger::Cron(CronEvent::Digest)).unwrap();
        }
        drop(tx);

        let first = agent_rx.recv().await.unwrap();
        let mut batch = vec![first];
        loop {
            match agent_rx.try_recv() {
                Ok(trigger) => batch.push(trigger),
                Err(_) => break,
            }
        }
        assert_eq!(batch.len(), 5);
    }

    #[tokio::test]
    async fn drain_triggers_channel_closed() {
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        let mut agent_rx = rx;

        drop(tx); // Close immediately

        let result = agent_rx.recv().await;
        assert!(result.is_none());
    }
}
