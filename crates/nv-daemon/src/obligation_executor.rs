//! Autonomous obligation executor.
//!
//! When Nova is idle (no interactive messages for `idle_debounce_secs` and
//! no active workers), the orchestrator calls `execute_obligation()` to pick
//! up the highest-priority Nova-owned open obligation and work on it using
//! all available tools.
//!
//! The executor reuses the same tool dispatch as `Worker::run` — full tool
//! access, no PendingAction gates, no tool count cap. The only safety bound
//! is a 5-minute wall-clock timeout configured via `[autonomy]` in `nv.toml`.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use nv_core::config::AutonomyConfig;
use nv_core::types::{InlineButton, InlineKeyboard, Obligation, OutboundMessage};

use crate::agent::build_system_context;
use crate::anthropic::AnthropicClient;
use crate::claude::{ContentBlock, Message, StopReason};
use crate::http::{DaemonEvent, ObligationActivityEvent};
use crate::nexus;
use crate::tools;
use crate::worker::{SharedDeps, TOOL_TIMEOUT_READ, TOOL_TIMEOUT_WRITE, WRITE_TOOLS};

// ── Activity Emit Helper ───────────────────────────────────────────────

/// Emit an `ObligationActivityEvent` to both the ring buffer and the WebSocket
/// broadcast channel. Fire-and-forget: errors are logged at debug level.
fn emit_activity(deps: &SharedDeps, event_type: &str, obligation_id: &str, description: &str, metadata: Option<serde_json::Value>) {
    let event = ObligationActivityEvent {
        id: uuid::Uuid::new_v4().to_string(),
        event_type: event_type.to_string(),
        obligation_id: obligation_id.to_string(),
        description: description.to_string(),
        timestamp: Utc::now(),
        metadata,
    };
    deps.activity_buffer.push(event.clone());
    let _ = deps.obligation_event_tx.send(DaemonEvent::ObligationActivity(event));
}

// ── Result Type ───────────────────────────────────────────────────────

/// The outcome of an autonomous obligation execution attempt.
#[derive(Debug)]
pub enum ObligationResult {
    /// Nova completed the work. Contains a summary of what was accomplished.
    Completed { summary: String },
    /// Execution failed (Claude error, tool error, or empty response).
    Failed { error: String },
    /// The 5-minute timeout was exceeded.
    Timeout,
    /// The budget was exhausted (reserved for future use; currently unused
    /// since there is no tool count cap).
    BudgetExhausted { partial: String },
}

// ── Context Builder ───────────────────────────────────────────────────

/// Build the system prompt for autonomous obligation execution.
///
/// Includes the obligation's `detected_action`, `source_message`, `priority`,
/// `project_code`, and any existing research notes. Instructs Nova to use
/// available tools and summarize what she accomplished.
pub fn build_obligation_context(obligation: &Obligation, research_summary: Option<&str>) -> String {
    let base = build_system_context(None);

    let priority_label = match obligation.priority {
        0 => "P0 (Critical)",
        1 => "P1 (High)",
        2 => "P2 (Important)",
        3 => "P3 (Minor)",
        _ => "P4 (Backlog)",
    };

    let project_info = obligation
        .project_code
        .as_deref()
        .map(|p| format!("Project: {p}\n"))
        .unwrap_or_default();

    let source_info = obligation
        .source_message
        .as_deref()
        .map(|s| format!("Source message context: {s}\n"))
        .unwrap_or_default();

    let research_info = research_summary
        .map(|r| format!("\nExisting research context:\n{r}\n"))
        .unwrap_or_default();

    format!(
        "{base}\n\n\
         --- AUTONOMOUS OBLIGATION EXECUTION ---\n\
         You are working autonomously on one of your own obligations.\n\
         Obligation ID: {id}\n\
         Priority: {priority_label}\n\
         {project_info}{source_info}Obligation: {action}\
         {research_info}\n\
         IMPORTANT: You MUST use your tools to fulfill this obligation. Do NOT just \
         describe what you would do — actually call the tools. For example:\
         \n- Need Teams data? Call teams_list_chats, teams_read_chat, teams_messages\
         \n- Need Jira data? Call jira_search, jira_get\
         \n- Need to save findings? Call write_memory\
         \n- Need Outlook? Call read_outlook_inbox, read_outlook_calendar\
         \n- Need ADO? Call query_ado_work_items\
         \nYou have full tool access. Use tool_use blocks, not text descriptions. \
         After completing the work, summarize what you accomplished in 2-3 sentences. \
         Do not ask for confirmation — act directly.",
        base = base,
        id = obligation.id,
        priority_label = priority_label,
        project_info = project_info,
        source_info = source_info,
        action = obligation.detected_action,
        research_info = research_info,
    )
}

// ── Executor ──────────────────────────────────────────────────────────

/// Execute an obligation autonomously using all available tools.
///
/// The entire execution (context build + Claude call + all tool calls) is
/// bounded by `config.timeout_secs`. There is no tool count cap — Nova uses
/// as many tools as needed within the time budget.
///
/// After execution (regardless of result), calls `update_last_attempt_at`
/// on the obligation store to prevent immediate retry.
pub async fn execute_obligation(
    obligation: &Obligation,
    deps: &Arc<SharedDeps>,
    config: &AutonomyConfig,
) -> ObligationResult {
    tracing::info!(
        obligation_id = %obligation.id,
        priority = obligation.priority,
        action = %obligation.detected_action,
        "autonomous executor: starting obligation"
    );

    // Emit execution_started activity event.
    emit_activity(
        deps,
        "obligation.execution_started",
        &obligation.id,
        &format!("Nova started working on: {}", obligation.detected_action),
        None,
    );

    // Load latest research notes for this obligation (if any).
    let research_summary: Option<String> = deps
        .obligation_store
        .as_ref()
        .and_then(|store_arc| store_arc.lock().ok())
        .and_then(|store| {
            store
                .get_latest_research(&obligation.id)
                .ok()
                .flatten()
                .map(|r| r.summary)
        });

    let system_prompt =
        build_obligation_context(obligation, research_summary.as_deref());
    let tool_definitions = tools::register_tools();

    let prompt = format!("Work on this obligation now: {}", obligation.detected_action);
    let timeout_dur = Duration::from_secs(config.timeout_secs);

    // ── Sidecar path (highest priority) ──────────────────────────────
    // If the Agent SDK sidecar is available, send a single request and get
    // the final result back. The sidecar handles the entire tool loop via MCP
    // so no manual tool dispatch is needed here.
    if let Some(ref sidecar) = deps.sidecar {
        let sidecar_tools: Vec<crate::sidecar::ToolDefinition> = tool_definitions
            .iter()
            .map(|t| crate::sidecar::ToolDefinition {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect();

        let req = crate::sidecar::SidecarRequest {
            id: obligation.id.clone(),
            system: system_prompt.clone(),
            prompt: prompt.clone(),
            tools: sidecar_tools,
            max_turns: 30,
            timeout_secs: config.timeout_secs,
        };

        let sidecar_result = tokio::time::timeout(
            timeout_dur,
            sidecar.send_request(req),
        )
        .await;

        // Update last_attempt_at regardless of sidecar result.
        if let Some(store_arc) = &deps.obligation_store {
            if let Ok(store) = store_arc.lock() {
                let now = chrono::Utc::now();
                if let Err(e) = store.update_last_attempt_at(&obligation.id, &now) {
                    tracing::warn!(
                        obligation_id = %obligation.id,
                        error = %e,
                        "failed to update last_attempt_at"
                    );
                }
            }
        }

        return match sidecar_result {
            Err(_elapsed) => {
                tracing::warn!(
                    obligation_id = %obligation.id,
                    timeout_secs = config.timeout_secs,
                    "autonomous executor: sidecar timed out"
                );
                ObligationResult::Timeout
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    obligation_id = %obligation.id,
                    error = %e,
                    "autonomous executor: sidecar request failed — falling back to AnthropicClient"
                );
                // Fall back: run the standard executor loop below.
                // We return early here; the code below handles the fallback.
                // To avoid code duplication, drop into the fallback path by
                // continuing execution after this block.
                //
                // Since we already updated last_attempt_at above, we need to
                // run the fallback and return its result directly.
                let anthropic_fallback = AnthropicClient::from_env("claude-sonnet-4-6");
                match anthropic_fallback {
                    Ok(ac) => {
                        let user_msg = Message::user(&prompt);
                        let mut conversation = vec![user_msg];
                        let fallback_result = tokio::time::timeout(
                            timeout_dur,
                            run_executor_loop(
                                &system_prompt,
                                &mut conversation,
                                &tool_definitions,
                                &ac,
                                deps,
                                &obligation.id,
                            ),
                        )
                        .await;
                        match fallback_result {
                            Err(_elapsed) => ObligationResult::Timeout,
                            Ok(inner) => inner,
                        }
                    }
                    Err(api_err) => ObligationResult::Failed {
                        error: format!("Sidecar failed and AnthropicClient unavailable: sidecar={e}, api={api_err}"),
                    },
                }
            }
            Ok(Ok(sidecar_resp)) => {
                if let Some(ref err) = sidecar_resp.error {
                    tracing::warn!(
                        obligation_id = %obligation.id,
                        error = %err,
                        "autonomous executor: sidecar returned error"
                    );
                    ObligationResult::Failed {
                        error: format!("Sidecar error: {err}"),
                    }
                } else {
                    let summary = sidecar_resp
                        .content
                        .iter()
                        .filter(|b| b.block_type == "text")
                        .map(|b| b.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");

                    emit_activity(
                        deps,
                        "obligation.tool_loop_completed",
                        &obligation.id,
                        "Nova completed tool loop via Agent SDK sidecar",
                        None,
                    );

                    if summary.is_empty() {
                        ObligationResult::Failed {
                            error: "Sidecar returned empty response".to_string(),
                        }
                    } else {
                        ObligationResult::Completed { summary }
                    }
                }
            }
        };
    }

    // ── AnthropicClient fallback (no sidecar) ─────────────────────────
    // Use AnthropicClient (direct HTTP API) instead of ClaudeClient (CC CLI wrapper).
    // The CC CLI subprocess had --tools-json removed, so it can't dispatch tools.
    // AnthropicClient sends tool definitions in the request body directly.
    let anthropic_client = match AnthropicClient::from_env("claude-sonnet-4-6") {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "autonomous executor: failed to create AnthropicClient");
            return ObligationResult::Failed {
                error: format!("Failed to create API client: {e}"),
            };
        }
    };

    let user_message = Message::user(&prompt);
    let mut conversation = vec![user_message];

    let result = tokio::time::timeout(
        timeout_dur,
        run_executor_loop(
            &system_prompt,
            &mut conversation,
            &tool_definitions,
            &anthropic_client,
            deps,
            &obligation.id,
        ),
    )
    .await;

    // Update last_attempt_at regardless of result (enables cooldown).
    if let Some(store_arc) = &deps.obligation_store {
        if let Ok(store) = store_arc.lock() {
            let now = Utc::now();
            if let Err(e) = store.update_last_attempt_at(&obligation.id, &now) {
                tracing::warn!(
                    obligation_id = %obligation.id,
                    error = %e,
                    "failed to update last_attempt_at"
                );
            }
        }
    }

    match result {
        Err(_elapsed) => {
            tracing::warn!(
                obligation_id = %obligation.id,
                timeout_secs = config.timeout_secs,
                "autonomous executor: timed out"
            );
            ObligationResult::Timeout
        }
        Ok(inner) => inner,
    }
}

/// Inner loop: send Claude messages, execute tools, iterate until done.
///
/// Returns the final summary text from Claude, or an error description.
async fn run_executor_loop(
    system_prompt: &str,
    conversation: &mut Vec<Message>,
    tool_definitions: &[crate::claude::ToolDefinition],
    client: &AnthropicClient,
    deps: &Arc<SharedDeps>,
    obligation_id: &str,
) -> ObligationResult {
    let mut final_text = String::new();

    loop {
        // AnthropicClient::send_message takes (messages, system, tools) — direct HTTP API.
        let response = match client
            .send_message(conversation, system_prompt, tool_definitions)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "autonomous executor: Claude call failed");
                return ObligationResult::Failed {
                    error: format!("Claude API error: {e}"),
                };
            }
        };

        // Collect text from this response.
        for block in &response.content {
            if let ContentBlock::Text { text } = block {
                final_text = text.clone();
            }
        }

        // Collect tool uses.
        let tool_uses: Vec<(String, String, serde_json::Value)> = response
            .content
            .iter()
            .filter_map(|b| {
                if let ContentBlock::ToolUse { id, name, input } = b {
                    Some((id.clone(), name.clone(), input.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Stop if no tool uses or stop reason is end_turn.
        if tool_uses.is_empty() || response.stop_reason != StopReason::ToolUse {
            break;
        }

        // Append assistant turn.
        conversation.push(Message::assistant_blocks(response.content.clone()));

        // Execute each tool call.
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

        let nexus_backend_owned: Option<nexus::backend::NexusBackend> = deps
            .team_agent_dispatcher
            .as_ref()
            .map(|d| nexus::backend::NexusBackend::new(d.clone()));
        let nexus_backend_ref = nexus_backend_owned.as_ref();

        let mut tool_results = Vec::new();
        for (tool_id, tool_name, tool_input) in &tool_uses {
            tracing::debug!(
                tool = %tool_name,
                "autonomous executor: executing tool"
            );

            // Emit obligation.tool_called activity event.
            emit_activity(
                deps,
                "obligation.tool_called",
                obligation_id,
                &format!("Nova called tool: {tool_name}"),
                Some(serde_json::json!({ "tool_name": tool_name })),
            );

            let timeout_secs = if WRITE_TOOLS.contains(&tool_name.as_str()) {
                TOOL_TIMEOUT_WRITE
            } else {
                TOOL_TIMEOUT_READ
            };

            let exec_result = match tokio::time::timeout(
                Duration::from_secs(timeout_secs),
                tools::execute_tool_send_with_backend(
                    tool_name,
                    tool_input,
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
                Ok(r) => r,
                Err(_elapsed) => {
                    tracing::warn!(tool = %tool_name, "autonomous executor: tool timed out");
                    Err(anyhow::anyhow!("tool timed out after {timeout_secs}s"))
                }
            };

            let (content, is_error) = match exec_result {
                Ok(tools::ToolResult::Immediate(output)) => (output, false),
                Ok(tools::ToolResult::PendingAction { description, .. }) => {
                    // In autonomous mode, PendingAction tools are executed directly.
                    // Nova has full autonomy — no confirmation gates.
                    (
                        format!("Action queued (autonomous mode): {description}"),
                        false,
                    )
                }
                Err(e) => (format!("Tool error: {e}"), true),
            };

            tool_results.push(crate::claude::ToolResultBlock {
                tool_use_id: tool_id.clone(),
                content,
                is_error,
            });
        }

        // Append tool results as user turn.
        conversation.push(Message::tool_results(tool_results));
    }

    if final_text.is_empty() {
        ObligationResult::Failed {
            error: "Claude returned an empty response".to_string(),
        }
    } else {
        ObligationResult::Completed {
            summary: final_text,
        }
    }
}

// ── Reporting ─────────────────────────────────────────────────────────

/// Build the `[Confirm Done] [Reopen]` inline keyboard for a proposed-done obligation.
pub fn proposed_done_keyboard(obligation_id: &str) -> InlineKeyboard {
    InlineKeyboard {
        rows: vec![vec![
            InlineButton {
                text: "Confirm Done".to_string(),
                callback_data: format!("confirm_done:{obligation_id}"),
            },
            InlineButton {
                text: "Reopen".to_string(),
                callback_data: format!("reopen:{obligation_id}"),
            },
        ]],
    }
}

/// Handle the result of an obligation execution:
/// - On `Completed`: store note, set status to `proposed_done`, send Telegram summary with keyboard.
/// - On `Failed`/`Timeout`/`BudgetExhausted`: store error note, keep status as `in_progress`,
///   send Telegram error message.
pub async fn handle_execution_result(
    obligation: &Obligation,
    result: ObligationResult,
    deps: &Arc<SharedDeps>,
    telegram_chat_id: Option<i64>,
) {
    let now_str = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

    match result {
        ObligationResult::Completed { ref summary } => {
            tracing::info!(
                obligation_id = %obligation.id,
                summary_len = summary.len(),
                "autonomous executor: completed successfully"
            );

            // Emit execution_completed activity event.
            let short_summary = if summary.len() > 200 {
                format!("{}…", &summary[..197])
            } else {
                summary.clone()
            };
            emit_activity(
                deps,
                "obligation.execution_completed",
                &obligation.id,
                &format!("Nova completed: {short_summary}"),
                Some(serde_json::json!({ "result": "proposed_done" })),
            );

            // Store execution note.
            if let Some(store_arc) = &deps.obligation_store {
                if let Ok(store) = store_arc.lock() {
                    let note = format!("[Auto-executed {now_str}] {summary}");
                    if let Err(e) = store.append_execution_note(&obligation.id, &note) {
                        tracing::warn!(error = %e, "failed to append execution note");
                    }
                    // Transition to proposed_done.
                    if let Err(e) = store.update_status(
                        &obligation.id,
                        &nv_core::types::ObligationStatus::ProposedDone,
                    ) {
                        tracing::warn!(error = %e, "failed to set proposed_done status");
                    }
                }
            }

            // Send Telegram summary with confirm/reopen keyboard.
            if let Some(chat_id) = telegram_chat_id {
                let truncated_summary = if summary.len() > 500 {
                    format!("{}…", &summary[..497])
                } else {
                    summary.clone()
                };

                let action_short = if obligation.detected_action.len() > 80 {
                    format!("{}…", &obligation.detected_action[..77])
                } else {
                    obligation.detected_action.clone()
                };

                let text = format!(
                    "Completed: {action_short}\n\n{truncated_summary}\n\nMark as done?",
                );
                let keyboard = proposed_done_keyboard(&obligation.id);

                if let Some(channel) = deps.channels.get("telegram") {
                    if let Some(tg_chan) = channel
                        .as_any()
                        .downcast_ref::<crate::channels::telegram::TelegramChannel>()
                    {
                        if let Err(e) = tg_chan
                            .client
                            .send_message(chat_id, &text, None, Some(&keyboard))
                            .await
                        {
                            tracing::warn!(error = %e, "failed to send proposed_done notification");
                        }
                    } else {
                        // Fallback: use channel send_message (no keyboard)
                        let msg = OutboundMessage {
                            channel: "telegram".to_string(),
                            content: text,
                            reply_to: None,
                            keyboard: Some(keyboard),
                        };
                        if let Err(e) = channel.send_message(msg).await {
                            tracing::warn!(error = %e, "failed to send proposed_done notification via channel");
                        }
                    }
                }
                let _ = chat_id;
            }
        }

        ObligationResult::Failed { .. }
        | ObligationResult::Timeout
        | ObligationResult::BudgetExhausted { .. } => {
            let (label, error_text) = match &result {
                ObligationResult::Failed { error } => ("failed", error.as_str()),
                ObligationResult::Timeout => ("timed out", "Execution exceeded 5-minute limit"),
                ObligationResult::BudgetExhausted { partial } => {
                    ("budget exhausted", partial.as_str())
                }
                _ => unreachable!(),
            };

            tracing::warn!(
                obligation_id = %obligation.id,
                label,
                error = error_text,
                "autonomous executor: execution did not complete"
            );

            // Emit execution_completed (failed) activity event.
            emit_activity(
                deps,
                "obligation.execution_completed",
                &obligation.id,
                &format!("Nova execution {label}: {error_text}"),
                Some(serde_json::json!({ "result": label })),
            );

            // Store failure note.
            if let Some(store_arc) = &deps.obligation_store {
                if let Ok(store) = store_arc.lock() {
                    let note = format!("[Attempt {label} {now_str}] {error_text}");
                    if let Err(e) = store.append_execution_note(&obligation.id, &note) {
                        tracing::warn!(error = %e, "failed to append failure note");
                    }
                    // Keep status as in_progress.
                    if let Err(e) = store.update_status(
                        &obligation.id,
                        &nv_core::types::ObligationStatus::InProgress,
                    ) {
                        tracing::warn!(error = %e, "failed to set in_progress status");
                    }
                }
            }

            // Send Telegram error message.
            if let Some(chat_id) = telegram_chat_id {
                let action_short = if obligation.detected_action.len() > 80 {
                    format!("{}…", &obligation.detected_action[..77])
                } else {
                    obligation.detected_action.clone()
                };
                let text = format!(
                    "Failed to complete: {action_short}\n\nReason: {error_text}\n\nWill retry after cooldown.",
                );

                if let Some(channel) = deps.channels.get("telegram") {
                    if let Some(tg_chan) = channel
                        .as_any()
                        .downcast_ref::<crate::channels::telegram::TelegramChannel>()
                    {
                        if let Err(e) =
                            tg_chan.client.send_message(chat_id, &text, None, None).await
                        {
                            tracing::warn!(error = %e, "failed to send execution failure notification");
                        }
                    }
                }
                let _ = chat_id;
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::types::{Obligation, ObligationOwner, ObligationStatus};

    fn make_obligation(id: &str, action: &str, priority: i32) -> Obligation {
        Obligation {
            id: id.to_string(),
            source_channel: "telegram".to_string(),
            source_message: Some("test source".to_string()),
            detected_action: action.to_string(),
            project_code: Some("NV".to_string()),
            priority,
            status: ObligationStatus::Open,
            owner: ObligationOwner::Nova,
            owner_reason: None,
            deadline: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            last_attempt_at: None,
        }
    }

    #[test]
    fn build_obligation_context_includes_action() {
        let ob = make_obligation("test-id", "Pull Teams messages and build org profiles", 1);
        let ctx = build_obligation_context(&ob, None);
        assert!(ctx.contains("Pull Teams messages and build org profiles"));
        assert!(ctx.contains("P1"));
        assert!(ctx.contains("NV"));
    }

    #[test]
    fn build_obligation_context_includes_research() {
        let ob = make_obligation("test-id", "do something", 2);
        let research = "Found Jira ticket OO-42 in code review.";
        let ctx = build_obligation_context(&ob, Some(research));
        assert!(ctx.contains("OO-42"));
    }

    #[test]
    fn proposed_done_keyboard_has_correct_callbacks() {
        let kb = proposed_done_keyboard("obligation-123");
        assert_eq!(kb.rows.len(), 1);
        let row = &kb.rows[0];
        assert_eq!(row.len(), 2);
        assert_eq!(row[0].callback_data, "confirm_done:obligation-123");
        assert_eq!(row[1].callback_data, "reopen:obligation-123");
        assert_eq!(row[0].text, "Confirm Done");
        assert_eq!(row[1].text, "Reopen");
    }
}
