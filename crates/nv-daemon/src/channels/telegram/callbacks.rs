//! In-process Telegram callback handlers for reminder actions.
//!
//! Reminder callbacks (`reminder_done:`, `reminder_snooze:`, `reminder_backlog:`, and
//! the snooze sub-options) are handled here without routing through the agent loop.
//! This avoids Claude API tokens for structural state transitions (DB writes).

use std::sync::{Arc, Mutex};

use nv_core::types::{InlineButton, InlineKeyboard, ObligationStatus, OutboundMessage};

use crate::channels::telegram::client::TelegramClient;
use crate::obligation_store::ObligationStore;
use crate::reminders::{parse_relative_time, ReminderStore};

/// Dispatch a `reminder_*` callback to the appropriate handler.
///
/// Called from `poll_messages()` after `answer_callback_query` has already sent the toast.
/// Does not dispatch to `Trigger::Message`.
pub async fn handle_reminder_callback(
    data: &str,
    reminder_store: &Option<Arc<Mutex<ReminderStore>>>,
    obligation_store: &Option<Arc<Mutex<ObligationStore>>>,
    client: &TelegramClient,
    chat_id: i64,
    timezone: &str,
) -> anyhow::Result<()> {
    if data.starts_with("reminder_done:") {
        let id = parse_id(data, "reminder_done:")?;
        handle_done(id, reminder_store, obligation_store).await
    } else if data.starts_with("reminder_snooze_1h:") {
        let id = parse_id(data, "reminder_snooze_1h:")?;
        handle_snooze_duration(id, "1h", reminder_store, client, chat_id, timezone).await
    } else if data.starts_with("reminder_snooze_4h:") {
        let id = parse_id(data, "reminder_snooze_4h:")?;
        handle_snooze_duration(id, "4h", reminder_store, client, chat_id, timezone).await
    } else if data.starts_with("reminder_snooze_tomorrow:") {
        let id = parse_id(data, "reminder_snooze_tomorrow:")?;
        handle_snooze_duration(id, "tomorrow", reminder_store, client, chat_id, timezone).await
    } else if data.starts_with("reminder_snooze:") {
        let id = parse_id(data, "reminder_snooze:")?;
        handle_snooze_picker(id, client, chat_id).await
    } else if data.starts_with("reminder_backlog:") {
        let id = parse_id(data, "reminder_backlog:")?;
        handle_backlog(id, reminder_store, obligation_store).await
    } else {
        tracing::warn!(data, "unrecognised reminder_ callback data — ignoring");
        Ok(())
    }
}

// ── Handlers ─────────────────────────────────────────────────────────

/// Mark reminder delivered; if obligation linked, set status to Done.
async fn handle_done(
    reminder_id: i64,
    reminder_store: &Option<Arc<Mutex<ReminderStore>>>,
    obligation_store: &Option<Arc<Mutex<ObligationStore>>>,
) -> anyhow::Result<()> {
    let obligation_id = lookup_obligation_id(reminder_id, reminder_store)?;

    if let Some(ob_id) = &obligation_id {
        update_obligation_status(ob_id, ObligationStatus::Done, obligation_store);
    }

    mark_delivered(reminder_id, reminder_store);
    Ok(())
}

/// Send the snooze time-picker keyboard as a new message.
async fn handle_snooze_picker(
    reminder_id: i64,
    client: &TelegramClient,
    chat_id: i64,
) -> anyhow::Result<()> {
    let keyboard = InlineKeyboard {
        rows: vec![vec![
            InlineButton {
                text: "+1h".to_string(),
                callback_data: format!("reminder_snooze_1h:{reminder_id}"),
            },
            InlineButton {
                text: "+4h".to_string(),
                callback_data: format!("reminder_snooze_4h:{reminder_id}"),
            },
            InlineButton {
                text: "Tomorrow".to_string(),
                callback_data: format!("reminder_snooze_tomorrow:{reminder_id}"),
            },
        ]],
    };

    let msg = OutboundMessage {
        channel: "telegram".to_string(),
        content: "Choose snooze duration:".to_string(),
        reply_to: None,
        keyboard: Some(keyboard),
    };

    client
        .send_message(chat_id, &msg.content, None, msg.keyboard.as_ref())
        .await?;

    Ok(())
}

/// Mark original reminder delivered, create a new reminder with the snooze offset,
/// and send a confirmation message.
async fn handle_snooze_duration(
    reminder_id: i64,
    offset: &str,
    reminder_store: &Option<Arc<Mutex<ReminderStore>>>,
    client: &TelegramClient,
    chat_id: i64,
    timezone: &str,
) -> anyhow::Result<()> {
    let Some(store_arc) = reminder_store else {
        tracing::warn!("reminder_store not available — cannot snooze reminder {reminder_id}");
        return Ok(());
    };

    // Fetch original reminder to copy message/channel/obligation_id
    let original = {
        let store = store_arc
            .lock()
            .map_err(|_| anyhow::anyhow!("reminder store mutex poisoned"))?;
        store.get_reminder(reminder_id)?
    };

    let Some(original) = original else {
        tracing::warn!(reminder_id, "snooze: reminder not found");
        return Ok(());
    };

    let due_at = parse_relative_time(offset, timezone)?;
    let human_time = due_at.format("%Y-%m-%d %H:%M UTC").to_string();

    {
        let store = store_arc
            .lock()
            .map_err(|_| anyhow::anyhow!("reminder store mutex poisoned"))?;

        // Mark original as delivered
        store.mark_delivered(reminder_id)?;

        // Create new reminder copying original fields
        store.create_reminder(
            &original.message,
            &due_at,
            &original.channel,
            original.obligation_id.as_deref(),
        )?;
    }

    // Send confirmation
    client
        .send_message(
            chat_id,
            &format!("Snoozed. New reminder set for {human_time}."),
            None,
            None,
        )
        .await?;

    Ok(())
}

/// If obligation linked, set status to Dismissed (backlog semantics). Mark reminder delivered.
async fn handle_backlog(
    reminder_id: i64,
    reminder_store: &Option<Arc<Mutex<ReminderStore>>>,
    obligation_store: &Option<Arc<Mutex<ObligationStore>>>,
) -> anyhow::Result<()> {
    let obligation_id = lookup_obligation_id(reminder_id, reminder_store)?;

    if let Some(ob_id) = &obligation_id {
        update_obligation_status(ob_id, ObligationStatus::Dismissed, obligation_store);
    }

    mark_delivered(reminder_id, reminder_store);
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────

fn parse_id(data: &str, prefix: &str) -> anyhow::Result<i64> {
    let id_str = data
        .strip_prefix(prefix)
        .ok_or_else(|| anyhow::anyhow!("expected prefix '{prefix}' in '{data}'"))?;
    id_str
        .parse::<i64>()
        .map_err(|_| anyhow::anyhow!("invalid reminder id '{id_str}' in callback data '{data}'"))
}

/// Look up the obligation_id for a reminder, returning None if the store is unavailable
/// or the reminder is not found.
fn lookup_obligation_id(
    reminder_id: i64,
    reminder_store: &Option<Arc<Mutex<ReminderStore>>>,
) -> anyhow::Result<Option<String>> {
    let Some(store_arc) = reminder_store else {
        return Ok(None);
    };
    let store = store_arc
        .lock()
        .map_err(|_| anyhow::anyhow!("reminder store mutex poisoned"))?;
    let reminder = store.get_reminder(reminder_id)?;
    Ok(reminder.and_then(|r| r.obligation_id))
}

/// Mark a reminder as delivered, logging on error (non-fatal).
fn mark_delivered(reminder_id: i64, reminder_store: &Option<Arc<Mutex<ReminderStore>>>) {
    let Some(store_arc) = reminder_store else {
        return;
    };
    match store_arc.lock() {
        Ok(store) => {
            if let Err(e) = store.mark_delivered(reminder_id) {
                tracing::warn!(reminder_id, error = %e, "failed to mark reminder delivered");
            }
        }
        Err(e) => {
            tracing::warn!(reminder_id, error = %e, "reminder store mutex poisoned on mark_delivered");
        }
    }
}

/// Update an obligation's status, logging on error (non-fatal).
fn update_obligation_status(
    obligation_id: &str,
    status: ObligationStatus,
    obligation_store: &Option<Arc<Mutex<ObligationStore>>>,
) {
    let Some(store_arc) = obligation_store else {
        return;
    };
    match store_arc.lock() {
        Ok(store) => {
            if let Err(e) = store.update_status(obligation_id, &status) {
                tracing::warn!(
                    obligation_id,
                    status = %status,
                    error = %e,
                    "failed to update obligation status"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                obligation_id,
                error = %e,
                "obligation store mutex poisoned on update_status"
            );
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as ChronoDuration, Utc};
    use std::path::Path;

    fn make_reminder_store(db_path: &Path) -> Arc<Mutex<ReminderStore>> {
        Arc::new(Mutex::new(ReminderStore::new(db_path).unwrap()))
    }

    fn make_obligation_store(db_path: &Path) -> Arc<Mutex<ObligationStore>> {
        Arc::new(Mutex::new(ObligationStore::new(db_path).unwrap()))
    }

    // ── parse_id ──────────────────────────────────────────────────────

    #[test]
    fn parse_id_valid() {
        assert_eq!(parse_id("reminder_done:42", "reminder_done:").unwrap(), 42);
        assert_eq!(parse_id("reminder_snooze:7", "reminder_snooze:").unwrap(), 7);
    }

    #[test]
    fn parse_id_invalid_not_a_number() {
        assert!(parse_id("reminder_done:abc", "reminder_done:").is_err());
    }

    #[test]
    fn parse_id_wrong_prefix() {
        assert!(parse_id("reminder_done:42", "reminder_backlog:").is_err());
    }

    // ── handle_done ───────────────────────────────────────────────────

    #[tokio::test]
    async fn done_without_obligation_marks_delivered() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("r.db");
        let store = make_reminder_store(&db_path);

        let due = Utc::now() + ChronoDuration::hours(1);
        let id = store
            .lock()
            .unwrap()
            .create_reminder("test", &due, "telegram", None)
            .unwrap();

        handle_done(id, &Some(store.clone()), &None)
            .await
            .unwrap();

        // Reminder should be delivered (no longer active)
        let active = store.lock().unwrap().list_active_reminders().unwrap();
        assert!(active.is_empty());
    }

    #[tokio::test]
    async fn done_without_obligation_store_is_noop() {
        // Should not panic when reminder_store is None
        handle_done(99, &None, &None).await.unwrap();
    }

    // ── handle_backlog ────────────────────────────────────────────────

    #[tokio::test]
    async fn backlog_without_obligation_marks_delivered() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("r.db");
        let store = make_reminder_store(&db_path);

        let due = Utc::now() + ChronoDuration::hours(1);
        let id = store
            .lock()
            .unwrap()
            .create_reminder("backlog me", &due, "telegram", None)
            .unwrap();

        handle_backlog(id, &Some(store.clone()), &None)
            .await
            .unwrap();

        let active = store.lock().unwrap().list_active_reminders().unwrap();
        assert!(active.is_empty());
    }

    #[tokio::test]
    async fn backlog_without_stores_is_noop() {
        handle_backlog(99, &None, &None).await.unwrap();
    }

    // ── snooze_1h creates new reminder ────────────────────────────────

    #[tokio::test]
    async fn snooze_1h_creates_new_reminder_and_marks_original_delivered() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("r.db");
        let store = make_reminder_store(&db_path);

        let due = Utc::now() + ChronoDuration::hours(1);
        let id = store
            .lock()
            .unwrap()
            .create_reminder("snooze me", &due, "telegram", None)
            .unwrap();

        // We can't call handle_snooze_duration without a real TelegramClient, so we test
        // the store interactions directly.

        // Simulate what handle_snooze_duration does:
        let original = store.lock().unwrap().get_reminder(id).unwrap().unwrap();
        let new_due = Utc::now() + ChronoDuration::hours(1);
        {
            let s = store.lock().unwrap();
            s.mark_delivered(id).unwrap();
            s.create_reminder(&original.message, &new_due, &original.channel, original.obligation_id.as_deref()).unwrap();
        }

        // Original delivered, one new active reminder
        let active = store.lock().unwrap().list_active_reminders().unwrap();
        assert_eq!(active.len(), 1);
        assert_ne!(active[0].id, id);
        assert_eq!(active[0].message, "snooze me");
    }

    // ── obligation_id round-trips through snooze ───────────────────────

    #[test]
    fn lookup_obligation_id_returns_none_for_missing_store() {
        let result = lookup_obligation_id(99, &None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn lookup_obligation_id_returns_some_when_linked() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("r.db");
        let store = make_reminder_store(&db_path);

        let due = Utc::now() + ChronoDuration::hours(1);
        let id = store
            .lock()
            .unwrap()
            .create_reminder("linked", &due, "telegram", Some("obl-xyz"))
            .unwrap();

        let result = lookup_obligation_id(id, &Some(store)).unwrap();
        assert_eq!(result.as_deref(), Some("obl-xyz"));
    }

    // ── make_obligation_store used to suppress dead-code warning ──────
    #[test]
    fn obligation_store_can_be_constructed() {
        let dir = tempfile::TempDir::new().unwrap();
        // messages.db requires MessageStore to have initialized the schema,
        // but ObligationStore::new just opens the connection — schema check is deferred.
        // We verify construction succeeds.
        let db_path = dir.path().join("msg.db");
        // Initialize via ReminderStore so the file exists
        let _ = ReminderStore::new(&db_path).unwrap();
        // ObligationStore does not run migrations itself — it piggybacks on MessageStore.
        // For test purposes, just verify the constructor does not panic on a missing table.
        let _store = make_obligation_store(&db_path);
    }
}
