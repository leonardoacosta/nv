//! Callback handlers for Telegram inline keyboard actions.
//!
//! Handles `approve:{uuid}`, `edit:{uuid}`, and `cancel:{uuid}` callbacks
//! dispatched from the agent loop when a callback query arrives.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use nv_core::channel::Channel;

use crate::tools::jira;
use crate::nexus;
use crate::tools::schedule::ScheduleStore;
use crate::state::{PendingStatus, State};
use crate::channels::telegram::client::TelegramClient;
use crate::tools;

// ── Approve Handler ─────────────────────────────────────────────────

/// Execute a confirmed pending action.
///
/// Loads the action from state, detects the action type, and routes to
/// the appropriate executor (Jira, Nexus/TeamAgent, Home Assistant, channel send, etc.).
///
/// When `nexus_backend` is `Some`, NexusStartSession and NexusStopSession actions
/// are routed through it (supporting both gRPC Nexus and team-agent modes).
/// When `None`, `nexus_client` is used as fallback (legacy path).
#[allow(clippy::too_many_arguments)]
pub async fn handle_approve(
    uuid_str: &str,
    jira_registry: Option<&jira::JiraRegistry>,
    nexus_client: Option<&nexus::client::NexusClient>,
    project_registry: &HashMap<String, PathBuf>,
    channels: &HashMap<String, Arc<dyn Channel>>,
    telegram: &TelegramClient,
    chat_id: i64,
    original_message_id: Option<i64>,
    state: &State,
    schedule_store: Option<&std::sync::Mutex<ScheduleStore>>,
) -> Result<()> {
    handle_approve_with_backend(
        uuid_str,
        jira_registry,
        nexus_client,
        None,
        project_registry,
        channels,
        telegram,
        chat_id,
        original_message_id,
        state,
        schedule_store,
    )
    .await
}

/// Variant of `handle_approve` that accepts an optional `NexusBackend`.
///
/// When `nexus_backend` is `Some`, NexusStartSession / NexusStopSession actions
/// are dispatched through it instead of the raw `nexus_client`.
#[allow(clippy::too_many_arguments)]
pub async fn handle_approve_with_backend(
    uuid_str: &str,
    jira_registry: Option<&jira::JiraRegistry>,
    nexus_client: Option<&nexus::client::NexusClient>,
    nexus_backend: Option<&nexus::backend::NexusBackend>,
    project_registry: &HashMap<String, PathBuf>,
    channels: &HashMap<String, Arc<dyn Channel>>,
    telegram: &TelegramClient,
    chat_id: i64,
    original_message_id: Option<i64>,
    state: &State,
    schedule_store: Option<&std::sync::Mutex<ScheduleStore>>,
) -> Result<()> {
    let uuid = Uuid::parse_str(uuid_str)?;

    let action = state
        .find_pending_action(&uuid)?
        .ok_or_else(|| anyhow::anyhow!("Pending action {uuid} not found"))?;

    if action.status != PendingStatus::AwaitingConfirmation {
        anyhow::bail!("Action {uuid} is not awaiting confirmation (status: {:?})", action.status);
    }

    // Map daemon state action to core action type for execution
    let action_type = detect_action_type(&action.payload);

    let result = match action_type {
        nv_core::types::ActionType::NexusStartSession => {
            if let Some(backend) = nexus_backend {
                backend.execute_start_session(&action.payload, project_registry).await
            } else {
                execute_nexus_start_session(
                    &action.payload,
                    nexus_client,
                    project_registry,
                )
                .await
            }
        }
        nv_core::types::ActionType::NexusStopSession => {
            if let Some(backend) = nexus_backend {
                backend.execute_stop_session(&action.payload).await
            } else {
                execute_nexus_stop_session(&action.payload, nexus_client).await
            }
        }
        nv_core::types::ActionType::ChannelSend => {
            tools::execute_channel_send(channels, &action.payload).await
        }
        nv_core::types::ActionType::ScheduleAdd => {
            execute_schedule_add(&action.payload, schedule_store)
        }
        nv_core::types::ActionType::ScheduleModify => {
            execute_schedule_modify(&action.payload, schedule_store)
        }
        nv_core::types::ActionType::ScheduleRemove => {
            execute_schedule_remove(&action.payload, schedule_store)
        }
        nv_core::types::ActionType::HaServiceCall => {
            execute_ha_service_call(&action.payload).await
        }
        _ => {
            // Jira and other action types
            if let Some(registry) = jira_registry {
                tools::execute_jira_action(registry, &action_type, &action.payload).await
            } else {
                Err(anyhow::anyhow!("Jira not configured"))
            }
        }
    };

    match result {
        Ok(result_text) => {
            state.update_pending_action(&uuid, PendingStatus::Executed)?;

            // Edit the original Telegram message with the result
            if let Some(msg_id) = original_message_id {
                let text = format!("Done: {result_text}");
                let _ = telegram.edit_message(chat_id, msg_id, &text, None).await;
            }
        }
        Err(e) => {
            tracing::error!(error = %e, uuid = %uuid, "failed to execute approved action");

            if let Some(msg_id) = original_message_id {
                let text = format!("Failed: {e}");
                let _ = telegram.edit_message(chat_id, msg_id, &text, None).await;
            }
        }
    }

    Ok(())
}

/// Execute a confirmed NexusStartSession action.
async fn execute_nexus_start_session(
    payload: &serde_json::Value,
    nexus_client: Option<&nexus::client::NexusClient>,
    project_registry: &HashMap<String, PathBuf>,
) -> Result<String> {
    let client = nexus_client.ok_or_else(|| anyhow::anyhow!("Nexus not configured"))?;
    let project = payload["project"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'project' in payload"))?;
    let command = payload["command"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'command' in payload"))?;

    // Pre-launch dedup guard: skip if a session is already active/idle for
    // this project. Prevents duplicate session storms on batch approvals.
    if client.has_active_session_for_project(project).await {
        tracing::info!(
            project,
            dedup = true,
            "session launch skipped — already active"
        );
        return Ok(format!(
            "Session already active for {project} \u{2014} launch skipped"
        ));
    }

    // Resolve project path from registry.
    // Fall back to $HOME/dev/{project} when not in the registry.
    let cwd = project_registry
        .get(project)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/dev/{project}")
        });

    let agent = payload["agent"].as_str();

    let args: Vec<String> = command
        .split_whitespace()
        .map(String::from)
        .collect();

    let (session_id, tmux_session) = client
        .start_session(project, &cwd, &args, agent)
        .await?;

    Ok(format!(
        "Session started: {session_id} (tmux: {tmux_session})"
    ))
}

/// Execute a confirmed NexusStopSession action.
async fn execute_nexus_stop_session(
    payload: &serde_json::Value,
    nexus_client: Option<&nexus::client::NexusClient>,
) -> Result<String> {
    let client = nexus_client.ok_or_else(|| anyhow::anyhow!("Nexus not configured"))?;
    let session_id = payload["session_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'session_id' in payload"))?;

    client.stop_session(session_id).await
}

// ── Edit Handler ────────────────────────────────────────────────────

/// Handle the "edit" callback — prompt the user for changes.
///
/// Replies asking what to change. The caller should set `editing_action_id`
/// in the agent loop state so the next user message is treated as an
/// edit instruction.
pub async fn handle_edit(
    uuid_str: &str,
    telegram: &TelegramClient,
    chat_id: i64,
    state: &State,
) -> Result<Option<Uuid>> {
    let uuid = Uuid::parse_str(uuid_str)?;

    let action = state
        .find_pending_action(&uuid)?
        .ok_or_else(|| anyhow::anyhow!("Pending action {uuid} not found"))?;

    if action.status != PendingStatus::AwaitingConfirmation {
        anyhow::bail!("Action {uuid} is not awaiting confirmation (status: {:?})", action.status);
    }

    // Send a message asking what to change
    let msg = format!(
        "Editing: {}\n\nWhat would you like to change?",
        action.description
    );
    telegram
        .send_message(chat_id, &msg, None, None)
        .await?;

    // Return the UUID so the caller can track the editing state
    Ok(Some(uuid))
}

// ── Cancel Handler ──────────────────────────────────────────────────

/// Handle the "cancel" callback — mark the action as cancelled and
/// edit the original Telegram message with a cancellation notice.
pub async fn handle_cancel(
    uuid_str: &str,
    telegram: &TelegramClient,
    chat_id: i64,
    original_message_id: Option<i64>,
    state: &State,
) -> Result<()> {
    let uuid = Uuid::parse_str(uuid_str)?;

    let action = state
        .find_pending_action(&uuid)?
        .ok_or_else(|| anyhow::anyhow!("Pending action {uuid} not found"))?;

    if action.status != PendingStatus::AwaitingConfirmation {
        anyhow::bail!("Action {uuid} is not awaiting confirmation (status: {:?})", action.status);
    }

    state.update_pending_action(&uuid, PendingStatus::Cancelled)?;

    // Edit the original Telegram message with cancellation notice
    if let Some(msg_id) = original_message_id {
        let text = format!("Cancelled: {}", action.description);
        let _ = telegram.edit_message(chat_id, msg_id, &text, None).await;
    }

    Ok(())
}

// ── Expiry Sweep ────────────────────────────────────────────────────

/// Scan pending actions and mark any older than 1 hour as expired.
///
/// Edits the original Telegram message with an expiry notice for each
/// expired action.
pub async fn check_expired_actions(
    telegram: &TelegramClient,
    chat_id: i64,
    state: &State,
) -> Result<u32> {
    let actions = state.load_pending_actions()?;
    let now = Utc::now();
    let expiry_duration = chrono::Duration::hours(1);
    let mut expired_count = 0u32;

    for action in &actions {
        if action.status != PendingStatus::AwaitingConfirmation {
            continue;
        }

        let age = now.signed_duration_since(action.created_at);
        if age <= expiry_duration {
            continue;
        }

        // Mark as expired
        state.update_pending_action(&action.id, PendingStatus::Expired)?;
        expired_count += 1;

        // Edit the original Telegram message with expiry notice
        if let Some(msg_id) = action.telegram_message_id {
            let text = format!(
                "Expired: {} (no response after 1 hour)",
                action.description
            );
            let _ = telegram.edit_message(chat_id, msg_id, &text, None).await;
        }

        tracing::info!(
            action_id = %action.id,
            description = %action.description,
            "pending action expired"
        );
    }

    if expired_count > 0 {
        tracing::info!(expired_count, "expired pending actions sweep complete");
    }

    Ok(expired_count)
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Detect the ActionType from a pending action payload.
///
/// Uses the `_action_type` field stored in the payload, or infers from
/// field presence.
fn detect_action_type(payload: &serde_json::Value) -> nv_core::types::ActionType {
    // Check for explicit action_type field (stored by agent loop)
    if let Some(at) = payload.get("_action_type").and_then(|v| v.as_str()) {
        return match at {
            "JiraCreate" => nv_core::types::ActionType::JiraCreate,
            "JiraTransition" => nv_core::types::ActionType::JiraTransition,
            "JiraAssign" => nv_core::types::ActionType::JiraAssign,
            "JiraComment" => nv_core::types::ActionType::JiraComment,
            "ChannelSend" => nv_core::types::ActionType::ChannelSend,
            "NexusStartSession" => nv_core::types::ActionType::NexusStartSession,
            "NexusStopSession" => nv_core::types::ActionType::NexusStopSession,
            "ScheduleAdd" => nv_core::types::ActionType::ScheduleAdd,
            "ScheduleModify" => nv_core::types::ActionType::ScheduleModify,
            "ScheduleRemove" => nv_core::types::ActionType::ScheduleRemove,
            "HaServiceCall" => nv_core::types::ActionType::HaServiceCall,
            _ => nv_core::types::ActionType::JiraCreate,
        };
    }

    // Infer from payload fields — Nexus actions
    if payload.get("command").is_some()
        && payload.get("project").is_some()
        && payload.get("issue_key").is_none()
    {
        return nv_core::types::ActionType::NexusStartSession;
    }
    if payload.get("session_id").is_some()
        && payload.get("text").is_none()
        && payload.get("issue_key").is_none()
    {
        return nv_core::types::ActionType::NexusStopSession;
    }

    // Infer ChannelSend from payload fields
    if payload.get("channel").is_some() && payload.get("message").is_some() {
        return nv_core::types::ActionType::ChannelSend;
    }

    // Infer HaServiceCall from payload fields
    if payload.get("domain").is_some() && payload.get("service").is_some() {
        return nv_core::types::ActionType::HaServiceCall;
    }

    // Infer from payload fields — Jira actions
    if payload.get("transition_name").is_some() {
        nv_core::types::ActionType::JiraTransition
    } else if payload.get("assignee_account_id").is_some() || payload.get("assignee").is_some() {
        if payload.get("project").is_some() {
            // JiraCreate with assignee
            nv_core::types::ActionType::JiraCreate
        } else {
            nv_core::types::ActionType::JiraAssign
        }
    } else if payload.get("body").is_some() && payload.get("issue_key").is_some() {
        nv_core::types::ActionType::JiraComment
    } else {
        nv_core::types::ActionType::JiraCreate
    }
}

// ── Schedule Executors ───────────────────────────────────────────────

/// Execute an approved ScheduleAdd action — insert the row into SQLite.
fn execute_schedule_add(
    payload: &serde_json::Value,
    schedule_store: Option<&std::sync::Mutex<crate::tools::schedule::ScheduleStore>>,
) -> anyhow::Result<String> {
    let store_lock = schedule_store
        .ok_or_else(|| anyhow::anyhow!("schedule store not available"))?;
    let store = store_lock.lock().unwrap();

    let name = payload["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'name' in payload"))?;
    let cron_expr = payload["cron_expr"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'cron_expr' in payload"))?;
    let action = payload["action"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'action' in payload"))?;
    let channel = payload["channel"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'channel' in payload"))?;

    let schedule = crate::tools::schedule::build_schedule(
        name.to_string(),
        cron_expr.to_string(),
        action.to_string(),
        channel.to_string(),
    );
    store.insert(&schedule)?;
    Ok(format!("Schedule '{}' added ({cron_expr}, action: {action})", name))
}

/// Execute an approved ScheduleModify action — update cron and/or enabled state.
fn execute_schedule_modify(
    payload: &serde_json::Value,
    schedule_store: Option<&std::sync::Mutex<crate::tools::schedule::ScheduleStore>>,
) -> anyhow::Result<String> {
    let store_lock = schedule_store
        .ok_or_else(|| anyhow::anyhow!("schedule store not available"))?;
    let store = store_lock.lock().unwrap();

    let name = payload["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'name' in payload"))?;

    let mut changes = Vec::new();

    if let Some(cron_expr) = payload.get("cron_expr").and_then(|v| v.as_str()) {
        store.update_cron(name, cron_expr)?;
        changes.push(format!("cron → {cron_expr}"));
    }
    if let Some(enabled) = payload.get("enabled").and_then(|v| v.as_bool()) {
        store.set_enabled(name, enabled)?;
        changes.push(format!("enabled → {enabled}"));
    }

    if changes.is_empty() {
        return Ok(format!("No changes applied to '{name}'"));
    }
    Ok(format!("Schedule '{}' updated: {}", name, changes.join(", ")))
}

/// Execute an approved ScheduleRemove action — delete the row from SQLite.
fn execute_schedule_remove(
    payload: &serde_json::Value,
    schedule_store: Option<&std::sync::Mutex<crate::tools::schedule::ScheduleStore>>,
) -> anyhow::Result<String> {
    let store_lock = schedule_store
        .ok_or_else(|| anyhow::anyhow!("schedule store not available"))?;
    let store = store_lock.lock().unwrap();

    let name = payload["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'name' in payload"))?;

    let deleted = store.delete(name)?;
    if deleted {
        Ok(format!("Schedule '{}' removed", name))
    } else {
        Ok(format!("Schedule '{}' not found (already deleted?)", name))
    }
}

/// Execute an approved HaServiceCall action.
async fn execute_ha_service_call(payload: &serde_json::Value) -> anyhow::Result<String> {
    let domain = payload["domain"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'domain' in payload"))?;
    let service = payload["service"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'service' in payload"))?;
    let data = payload
        .get("data")
        .ok_or_else(|| anyhow::anyhow!("missing 'data' in payload"))?;

    crate::tools::ha::ha_service_call_execute(domain, service, data).await
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_action_type_from_explicit_field() {
        let payload = serde_json::json!({"_action_type": "JiraTransition", "issue_key": "OO-1"});
        assert!(matches!(detect_action_type(&payload), nv_core::types::ActionType::JiraTransition));
    }

    #[test]
    fn detect_action_type_infers_transition() {
        let payload = serde_json::json!({"issue_key": "OO-1", "transition_name": "Done"});
        assert!(matches!(detect_action_type(&payload), nv_core::types::ActionType::JiraTransition));
    }

    #[test]
    fn detect_action_type_infers_assign() {
        let payload = serde_json::json!({"issue_key": "OO-1", "assignee_account_id": "abc"});
        assert!(matches!(detect_action_type(&payload), nv_core::types::ActionType::JiraAssign));
    }

    #[test]
    fn detect_action_type_infers_comment() {
        let payload = serde_json::json!({"issue_key": "OO-1", "body": "A comment"});
        assert!(matches!(detect_action_type(&payload), nv_core::types::ActionType::JiraComment));
    }

    #[test]
    fn detect_action_type_infers_create() {
        let payload = serde_json::json!({"project": "OO", "title": "Bug", "issue_type": "Bug"});
        assert!(matches!(detect_action_type(&payload), nv_core::types::ActionType::JiraCreate));
    }

    #[test]
    fn detect_action_type_explicit_nexus_start() {
        let payload = serde_json::json!({
            "_action_type": "NexusStartSession",
            "project": "oo",
            "command": "/apply fix-chat"
        });
        assert!(matches!(
            detect_action_type(&payload),
            nv_core::types::ActionType::NexusStartSession
        ));
    }

    #[test]
    fn detect_action_type_explicit_nexus_stop() {
        let payload = serde_json::json!({
            "_action_type": "NexusStopSession",
            "session_id": "s-123"
        });
        assert!(matches!(
            detect_action_type(&payload),
            nv_core::types::ActionType::NexusStopSession
        ));
    }

    #[test]
    fn detect_action_type_infers_nexus_start() {
        let payload = serde_json::json!({"project": "oo", "command": "/apply fix-chat"});
        assert!(matches!(
            detect_action_type(&payload),
            nv_core::types::ActionType::NexusStartSession
        ));
    }

    #[test]
    fn detect_action_type_infers_nexus_stop() {
        let payload = serde_json::json!({"session_id": "s-123"});
        assert!(matches!(
            detect_action_type(&payload),
            nv_core::types::ActionType::NexusStopSession
        ));
    }

    #[test]
    fn detect_action_type_explicit_ha_service_call() {
        let payload = serde_json::json!({
            "_action_type": "HaServiceCall",
            "domain": "light",
            "service": "turn_off",
            "data": {"entity_id": "light.office"}
        });
        assert!(matches!(
            detect_action_type(&payload),
            nv_core::types::ActionType::HaServiceCall
        ));
    }

    #[test]
    fn detect_action_type_infers_ha_service_call() {
        let payload = serde_json::json!({
            "domain": "light",
            "service": "turn_off",
            "data": {"entity_id": "light.office"}
        });
        assert!(matches!(
            detect_action_type(&payload),
            nv_core::types::ActionType::HaServiceCall
        ));
    }
}
