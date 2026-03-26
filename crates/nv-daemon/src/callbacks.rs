//! Callback handlers for Telegram inline keyboard actions.
//!
//! Handles `approve:{uuid}`, `edit:{uuid}`, and `cancel:{uuid}` callbacks
//! dispatched from the agent loop when a callback query arrives.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use nv_core::channel::Channel;

use crate::tools::jira;
use crate::nexus::backend::NexusBackend;
use crate::cc_sessions::CcSessionManager;
use crate::obligation_store::ObligationStore;
use crate::reminders::{parse_relative_time, ReminderStore};
use crate::tools::schedule::ScheduleStore;
use crate::state::{PendingStatus, State};
use crate::channels::telegram::client::TelegramClient;
use crate::tools;

// ── Approve Handler ─────────────────────────────────────────────────

/// Execute a confirmed pending action.
///
/// Loads the action from state, detects the action type, and routes to
/// the appropriate executor (Jira, CcSessionManager, Home Assistant, channel send, etc.).
#[allow(clippy::too_many_arguments)]
pub async fn handle_approve_with_backend(
    uuid_str: &str,
    jira_registry: Option<&jira::JiraRegistry>,
    nexus_backend: Option<&NexusBackend>,
    cc_session_manager: Option<&CcSessionManager>,
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
                Err(anyhow::anyhow!("Team agents not configured"))
            }
        }
        nv_core::types::ActionType::NexusStopSession => {
            if let Some(backend) = nexus_backend {
                backend.execute_stop_session(&action.payload).await
            } else {
                Err(anyhow::anyhow!("Team agents not configured"))
            }
        }
        nv_core::types::ActionType::CcStartSession => {
            if let Some(mgr) = cc_session_manager {
                mgr.execute_start(&action.payload, project_registry).await
            } else {
                Err(anyhow::anyhow!("CC session manager not configured"))
            }
        }
        nv_core::types::ActionType::CcStopSession => {
            if let Some(mgr) = cc_session_manager {
                mgr.execute_stop(&action.payload).await
            } else {
                Err(anyhow::anyhow!("CC session manager not configured"))
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
            "CcStartSession" => nv_core::types::ActionType::CcStartSession,
            "CcStopSession" => nv_core::types::ActionType::CcStopSession,
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

// ── Obligation Handlers ──────────────────────────────────────────────

/// Handle the "ob_cancel" callback — dismiss the obligation and edit the message.
///
/// Sets status=Dismissed on the obligation and edits the original Telegram message
/// to show "Cancelled: {detected_action}".
pub async fn handle_ob_cancel(
    ob_id: &str,
    telegram: &TelegramClient,
    chat_id: i64,
    original_message_id: Option<i64>,
    ob_store: &ObligationStore,
) -> Result<()> {
    // Look up detected_action before changing status so we can build the message.
    let detected_action = ob_store
        .get_by_id(ob_id)?
        .map(|ob| ob.detected_action)
        .unwrap_or_else(|| ob_id.to_string());

    ob_store.update_status(ob_id, &nv_core::types::ObligationStatus::Dismissed)?;

    if let Some(msg_id) = original_message_id {
        let text = format!("Cancelled: {detected_action}");
        let _ = telegram.edit_message(chat_id, msg_id, &text, None).await;
    }

    Ok(())
}

/// Handle the "ob_expiry" callback — acknowledge request for more time.
///
/// Does not mutate obligation status (no deadline column). Edits the original
/// Telegram message to confirm the extension, and optionally creates a 24h
/// reminder via `ReminderStore`.
pub async fn handle_ob_expiry(
    ob_id: &str,
    telegram: &TelegramClient,
    chat_id: i64,
    original_message_id: Option<i64>,
    ob_store: &ObligationStore,
    reminder_store: Option<&Mutex<ReminderStore>>,
    timezone: &str,
) -> Result<()> {
    if let Some(store_lock) = reminder_store {
        if let Ok(ob) = ob_store.get_by_id(ob_id).map(|r| r) {
            if let Some(ob) = ob {
                if let Ok(due_at) = parse_relative_time("24h", timezone) {
                    if let Ok(store) = store_lock.lock() {
                        let message = format!("Obligation still open: {}", ob.detected_action);
                        let _ = store.create_reminder(&message, &due_at, "telegram");
                    }
                }
            }
        }
    }

    if let Some(msg_id) = original_message_id {
        let text = "Deadline extended by 24h. Obligation remains open.".to_string();
        let _ = telegram.edit_message(chat_id, msg_id, &text, None).await;
    }

    Ok(())
}

/// Handle the "ob_snooze" callback — create a reminder for the obligation at a future time.
///
/// Parses `offset` via `parse_relative_time` (supports `"1h"`, `"4h"`, `"tomorrow"`),
/// looks up the obligation, creates a reminder, and edits the original message.
pub async fn handle_ob_snooze(
    ob_id: &str,
    offset: &str,
    telegram: &TelegramClient,
    chat_id: i64,
    original_message_id: Option<i64>,
    ob_store: &ObligationStore,
    reminder_store: &Mutex<ReminderStore>,
    timezone: &str,
) -> Result<()> {
    let due_at = parse_relative_time(offset, timezone)?;

    let detected_action = ob_store
        .get_by_id(ob_id)?
        .map(|ob| ob.detected_action)
        .unwrap_or_else(|| ob_id.to_string());

    {
        let store = reminder_store
            .lock()
            .map_err(|_| anyhow::anyhow!("reminder store mutex poisoned"))?;
        let message = format!("Obligation: {detected_action}");
        store.create_reminder(&message, &due_at, "telegram")?;
    }

    if let Some(msg_id) = original_message_id {
        // Format human-readable time in the target timezone.
        let human_time = format_snooze_time(&due_at, timezone);
        let text = format!("Snoozed until {human_time}.");
        let _ = telegram.edit_message(chat_id, msg_id, &text, None).await;
    }

    Ok(())
}

/// Format a UTC datetime into a short human-readable string.
///
/// Returns an RFC 3339 short representation in UTC.
fn format_snooze_time(dt: &chrono::DateTime<Utc>, _timezone: &str) -> String {
    dt.format("%a %d %b %H:%M UTC").to_string()
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
