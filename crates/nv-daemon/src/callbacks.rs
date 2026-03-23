//! Callback handlers for Telegram inline keyboard actions.
//!
//! Handles `approve:{uuid}`, `edit:{uuid}`, and `cancel:{uuid}` callbacks
//! dispatched from the agent loop when a callback query arrives.

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::jira;
use crate::nexus;
use crate::state::{PendingStatus, State};
use crate::telegram::client::TelegramClient;
use crate::tools;

use std::collections::HashMap;
use std::path::PathBuf;

// ── Approve Handler ─────────────────────────────────────────────────

/// Execute a confirmed pending action.
///
/// Loads the action from state, detects the action type, and routes to
/// the appropriate executor (Jira, Nexus, Home Assistant, etc.).
#[allow(clippy::too_many_arguments)]
pub async fn handle_approve(
    uuid_str: &str,
    jira_registry: Option<&jira::JiraRegistry>,
    nexus_client: Option<&nexus::client::NexusClient>,
    project_registry: &HashMap<String, PathBuf>,
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

    // Map daemon state action to core action type for execution
    let action_type = detect_action_type(&action.payload);

    let result = match action_type {
        nv_core::types::ActionType::NexusStartSession => {
            execute_nexus_start_session(
                &action.payload,
                nexus_client,
                project_registry,
            )
            .await
        }
        nv_core::types::ActionType::NexusStopSession => {
            execute_nexus_stop_session(&action.payload, nexus_client).await
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

    // Resolve project path from registry
    let cwd = project_registry
        .get(project)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("/home/nyaptor/dev/{project}"));

    let args: Vec<String> = command
        .split_whitespace()
        .map(String::from)
        .collect();

    let (session_id, tmux_session) = client
        .start_session(project, &cwd, &args)
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
            "NexusStartSession" => nv_core::types::ActionType::NexusStartSession,
            "NexusStopSession" => nv_core::types::ActionType::NexusStopSession,
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
}
