use nv_core::types::{
    ActionStatus, ActionType, InlineButton, InlineKeyboard, OutboundMessage, PendingAction,
    SessionEvent, SessionEventType,
};

use super::client::SessionDetail;
use crate::tools::jira::types::JiraCreateParams;

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

/// Format full error details for the "View Error" callback reply.
///
/// Returns a human-readable summary of the session for display in Telegram.
pub fn format_session_error_detail(session: &SessionDetail) -> String {
    let project = session.project.as_deref().unwrap_or("unknown");
    let started = session
        .started_at
        .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "unknown".into());

    format!(
        "Session Error Details\n\
         \n\
         Session: {}\n\
         Project: {project}\n\
         Agent: {}\n\
         Status: {}\n\
         Duration: {}\n\
         Started: {started}\n\
         CWD: {}\n\
         Command: {}\n\
         Model: {}\n\
         Cost: {}",
        session.id,
        session.agent_name,
        session.status,
        session.duration_display,
        session.cwd,
        session.command.as_deref().unwrap_or("n/a"),
        session.model.as_deref().unwrap_or("n/a"),
        session
            .cost_usd
            .map(|c| format!("${c:.2}"))
            .unwrap_or_else(|| "n/a".into()),
    )
}

/// Build a `PendingAction` with `JiraCreate` params pre-filled from a
/// session error context.
///
/// The caller should persist this via `state.save_pending_action` and send
/// the Jira confirmation keyboard.
pub fn create_bug_from_session_error(session: &SessionDetail) -> PendingAction {
    let project_key = session
        .project
        .as_deref()
        .unwrap_or("NV")
        .to_uppercase();

    let title = format!(
        "Session error: {} on {}",
        project_key, session.agent_name
    );

    let description = format!(
        "Automated bug report from Nexus session error.\n\
         \n\
         Session: {}\n\
         Agent: {}\n\
         Status: {}\n\
         Duration: {}\n\
         CWD: {}\n\
         Command: {}\n\
         Model: {}",
        session.id,
        session.agent_name,
        session.status,
        session.duration_display,
        session.cwd,
        session.command.as_deref().unwrap_or("n/a"),
        session.model.as_deref().unwrap_or("n/a"),
    );

    let params = JiraCreateParams {
        project: project_key,
        issue_type: "Bug".into(),
        title: title.clone(),
        description: Some(description),
        priority: Some("High".into()),
        assignee_account_id: None,
        labels: Some(vec!["nexus-error".into(), "auto-created".into()]),
    };

    PendingAction {
        id: uuid::Uuid::new_v4(),
        description: title,
        action_type: ActionType::JiraCreate,
        payload: serde_json::to_value(params).expect("JiraCreateParams serialization"),
        status: ActionStatus::Pending,
        created_at: chrono::Utc::now(),
        telegram_message_id: None,
        telegram_chat_id: None,
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

    fn make_session_detail() -> SessionDetail {
        SessionDetail {
            id: "s-abc123".into(),
            project: Some("oo".into()),
            status: "errored".into(),
            agent_name: "homelab".into(),
            started_at: None,
            duration_display: "5m".into(),
            branch: Some("main".into()),
            spec: None,
            cwd: "/home/user/dev/oo".into(),
            command: Some("claude --spec apply".into()),
            session_type: "managed".into(),
            model: Some("opus-4".into()),
            cost_usd: Some(1.23),
        }
    }

    #[test]
    fn format_session_error_detail_fields() {
        let session = make_session_detail();
        let text = format_session_error_detail(&session);

        assert!(text.contains("s-abc123"));
        assert!(text.contains("oo"));
        assert!(text.contains("homelab"));
        assert!(text.contains("errored"));
        assert!(text.contains("5m"));
        assert!(text.contains("/home/user/dev/oo"));
        assert!(text.contains("claude --spec apply"));
        assert!(text.contains("opus-4"));
        assert!(text.contains("$1.23"));
    }

    #[test]
    fn format_session_error_detail_missing_optionals() {
        let session = SessionDetail {
            id: "s-1".into(),
            project: None,
            status: "errored".into(),
            agent_name: "test".into(),
            started_at: None,
            duration_display: "1s".into(),
            branch: None,
            spec: None,
            cwd: "/tmp".into(),
            command: None,
            session_type: "ad-hoc".into(),
            model: None,
            cost_usd: None,
        };
        let text = format_session_error_detail(&session);

        assert!(text.contains("unknown")); // project fallback
        assert!(text.contains("n/a")); // command, model, cost fallbacks
    }

    #[test]
    fn create_bug_from_session_error_pending_action() {
        let session = make_session_detail();
        let action = create_bug_from_session_error(&session);

        assert_eq!(action.status, ActionStatus::Pending);
        assert!(matches!(action.action_type, ActionType::JiraCreate));
        assert!(action.description.contains("OO"));
        assert!(action.description.contains("homelab"));

        // Verify the payload deserializes back to JiraCreateParams
        let params: JiraCreateParams =
            serde_json::from_value(action.payload).expect("should deserialize");
        assert_eq!(params.project, "OO");
        assert_eq!(params.issue_type, "Bug");
        assert!(params.title.contains("Session error"));
        assert!(params.description.unwrap().contains("s-abc123"));
        assert_eq!(params.priority, Some("High".into()));
        assert!(params.labels.unwrap().contains(&"nexus-error".into()));
    }
}
