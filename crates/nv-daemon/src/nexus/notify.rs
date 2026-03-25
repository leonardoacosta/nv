use nv_core::types::{InlineButton, InlineKeyboard, OutboundMessage, SessionEvent, SessionEventType};

/// Format a session completed event into a Telegram notification.
///
/// Informational only — no action buttons needed.
#[allow(dead_code)] // Kept for future opt-in verbose mode
pub fn format_session_completed(event: &SessionEvent) -> OutboundMessage {
    let details = event
        .details
        .as_deref()
        .unwrap_or("completed");

    let content = format!(
        "Session completed on {}\n\
         Session: {}\n\
         Reason: {}",
        event.agent_name, event.session_id, details
    );

    OutboundMessage {
        channel: "telegram".into(),
        content,
        reply_to: None,
        keyboard: None,
    }
}

/// Format a session error event into a Telegram alert with action buttons.
///
/// The `event_id` is used to key error metadata for retry/create-bug callbacks.
/// When `event_id` is provided, uses the new `[Retry] [Create Bug]` keyboard.
/// Falls back to `[View Error] [Create Bug]` using session_id otherwise.
pub fn format_session_error(event: &SessionEvent, event_id: Option<&str>) -> OutboundMessage {
    let details = event
        .details
        .as_deref()
        .unwrap_or("unknown error");

    let content = format!(
        "Session error on {}\n\
         Session: {}\n\
         Error: {}",
        event.agent_name, event.session_id, details
    );

    let keyboard = if let Some(eid) = event_id {
        InlineKeyboard::session_error(eid)
    } else {
        InlineKeyboard {
            rows: vec![vec![
                InlineButton {
                    text: "View Error".into(),
                    callback_data: format!("nexus_err:view:{}", event.session_id),
                },
                InlineButton {
                    text: "Create Bug".into(),
                    callback_data: format!("nexus_err:bug:{}", event.session_id),
                },
            ]],
        }
    };

    OutboundMessage {
        channel: "telegram".into(),
        content,
        reply_to: None,
        keyboard: Some(keyboard),
    }
}

/// Route a Nexus session event to the appropriate notification format.
///
/// For failed events, pass `event_id` to enable retry/create-bug callbacks.
pub fn format_nexus_notification(
    event: &SessionEvent,
    event_id: Option<&str>,
) -> Option<OutboundMessage> {
    match event.event_type {
        SessionEventType::Failed => Some(format_session_error(event, event_id)),
        // Completed, Started, and Progress are informational — log only, no Telegram.
        // Completed events were spamming Telegram with "Session completed on omarchy"
        // for every CC session that ended normally.
        SessionEventType::Completed => {
            tracing::debug!(
                agent = %event.agent_name,
                session = %event.session_id,
                "nexus: session completed (suppressed notification)"
            );
            None
        }
        SessionEventType::Started | SessionEventType::Progress => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(event_type: SessionEventType, details: Option<String>) -> SessionEvent {
        SessionEvent {
            agent_name: "homelab".into(),
            session_id: "s-abc123".into(),
            event_type,
            details,
        }
    }

    #[test]
    fn completed_notification_format() {
        let event = make_event(SessionEventType::Completed, Some("all tasks done".into()));
        let msg = format_session_completed(&event);

        assert_eq!(msg.channel, "telegram");
        assert!(msg.content.contains("completed"));
        assert!(msg.content.contains("homelab"));
        assert!(msg.content.contains("s-abc123"));
        assert!(msg.content.contains("all tasks done"));
        assert!(msg.keyboard.is_none());
    }

    #[test]
    fn completed_notification_no_details() {
        let event = make_event(SessionEventType::Completed, None);
        let msg = format_session_completed(&event);
        assert!(msg.content.contains("completed"));
    }

    #[test]
    fn error_notification_has_buttons_legacy() {
        let event = make_event(SessionEventType::Failed, Some("OOM killed".into()));
        let msg = format_session_error(&event, None);

        assert!(msg.content.contains("error"));
        assert!(msg.content.contains("OOM killed"));
        assert!(msg.content.contains("homelab"));

        let kb = msg.keyboard.unwrap();
        assert_eq!(kb.rows.len(), 1);
        assert_eq!(kb.rows[0].len(), 2);
        assert_eq!(kb.rows[0][0].text, "View Error");
        assert!(kb.rows[0][0].callback_data.contains("nexus_err:view:s-abc123"));
        assert_eq!(kb.rows[0][1].text, "Create Bug");
        assert!(kb.rows[0][1].callback_data.contains("nexus_err:bug:s-abc123"));
    }

    #[test]
    fn error_notification_with_event_id_has_retry_buttons() {
        let event = make_event(SessionEventType::Failed, Some("OOM killed".into()));
        let msg = format_session_error(&event, Some("evt-123"));

        let kb = msg.keyboard.unwrap();
        assert_eq!(kb.rows.len(), 1);
        assert_eq!(kb.rows[0].len(), 2);
        assert!(kb.rows[0][0].text.contains("Retry"));
        assert_eq!(kb.rows[0][0].callback_data, "retry:evt-123");
        assert!(kb.rows[0][1].text.contains("Create Bug"));
        assert_eq!(kb.rows[0][1].callback_data, "bug:evt-123");
    }

    #[test]
    fn format_nexus_notification_completed_suppressed() {
        let event = make_event(SessionEventType::Completed, None);
        let msg = format_nexus_notification(&event, None);
        assert!(msg.is_none(), "completed events should be suppressed");
    }

    #[test]
    fn format_nexus_notification_failed() {
        let event = make_event(SessionEventType::Failed, None);
        let msg = format_nexus_notification(&event, None);
        assert!(msg.is_some());
        assert!(msg.unwrap().keyboard.is_some());
    }

    #[test]
    fn format_nexus_notification_started_none() {
        let event = make_event(SessionEventType::Started, None);
        assert!(format_nexus_notification(&event, None).is_none());
    }

    #[test]
    fn format_nexus_notification_progress_none() {
        let event = make_event(SessionEventType::Progress, None);
        assert!(format_nexus_notification(&event, None).is_none());
    }

}
