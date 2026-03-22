use nv_core::types::{InlineButton, InlineKeyboard};

use super::state::SuggestedAction;
use super::synthesize::DigestResult;

/// Maximum message length for Telegram (characters).
const TELEGRAM_CHAR_LIMIT: usize = 4096;

/// Maximum action buttons in a digest keyboard.
const MAX_ACTION_BUTTONS: usize = 5;

// ── Formatting ──────────────────────────────────────────────────────

/// Format a digest result into a Telegram-ready message and inline keyboard.
pub fn format_digest(result: &DigestResult) -> (String, Option<InlineKeyboard>) {
    let text = truncate_for_telegram(&result.content);
    let keyboard = build_action_keyboard(&result.suggested_actions);
    (text, keyboard)
}

/// Truncate text to fit Telegram's 4096-character limit.
///
/// Preserves complete lines where possible. Appends a truncation
/// indicator if the text was shortened.
fn truncate_for_telegram(text: &str) -> String {
    if text.len() <= TELEGRAM_CHAR_LIMIT {
        return text.to_string();
    }

    // Leave room for truncation indicator
    let budget = TELEGRAM_CHAR_LIMIT - 30;
    let truncated = &text[..budget];

    // Cut at the last newline to preserve complete lines
    if let Some(last_newline) = truncated.rfind('\n') {
        format!("{}\n\n[... truncated]", &text[..last_newline])
    } else {
        format!("{}\n\n[... truncated]", truncated)
    }
}

/// Build an inline keyboard with action buttons + dismiss button.
fn build_action_keyboard(actions: &[SuggestedAction]) -> Option<InlineKeyboard> {
    if actions.is_empty() {
        return None;
    }

    let mut rows: Vec<Vec<InlineButton>> = actions
        .iter()
        .take(MAX_ACTION_BUTTONS)
        .map(|action| {
            // Truncate label for Telegram button (max ~64 chars)
            let label = if action.label.len() > 60 {
                format!("{}...", &action.label[..57])
            } else {
                action.label.clone()
            };
            vec![InlineButton {
                text: label,
                callback_data: format!("digest_act:{}", action.id),
            }]
        })
        .collect();

    // Add "Dismiss All" button as the final row
    rows.push(vec![InlineButton {
        text: "Dismiss All".into(),
        callback_data: "digest_dismiss".into(),
    }]);

    Some(InlineKeyboard { rows })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::state::{DigestActionStatus, DigestActionType};

    #[test]
    fn format_short_digest() {
        let result = DigestResult {
            content: "Short digest content".into(),
            suggested_actions: vec![],
        };
        let (text, keyboard) = format_digest(&result);
        assert_eq!(text, "Short digest content");
        assert!(keyboard.is_none());
    }

    #[test]
    fn format_digest_with_actions() {
        let result = DigestResult {
            content: "Digest text".into(),
            suggested_actions: vec![
                SuggestedAction {
                    id: "act_1".into(),
                    label: "Close OO-142".into(),
                    action_type: DigestActionType::JiraTransition,
                    payload: serde_json::json!({}),
                    status: DigestActionStatus::Pending,
                },
                SuggestedAction {
                    id: "act_2".into(),
                    label: "Review PR #234".into(),
                    action_type: DigestActionType::FollowUpQuery,
                    payload: serde_json::json!({}),
                    status: DigestActionStatus::Pending,
                },
            ],
        };
        let (text, keyboard) = format_digest(&result);
        assert_eq!(text, "Digest text");

        let kb = keyboard.unwrap();
        // 2 action rows + 1 dismiss row
        assert_eq!(kb.rows.len(), 3);
        assert_eq!(kb.rows[0][0].text, "Close OO-142");
        assert_eq!(kb.rows[0][0].callback_data, "digest_act:act_1");
        assert_eq!(kb.rows[1][0].text, "Review PR #234");
        assert_eq!(kb.rows[1][0].callback_data, "digest_act:act_2");
        assert_eq!(kb.rows[2][0].text, "Dismiss All");
        assert_eq!(kb.rows[2][0].callback_data, "digest_dismiss");
    }

    #[test]
    fn truncate_short_text() {
        let text = "Short text";
        assert_eq!(truncate_for_telegram(text), "Short text");
    }

    #[test]
    fn truncate_long_text() {
        let text = "line\n".repeat(2000); // ~10000 chars
        let truncated = truncate_for_telegram(&text);
        assert!(truncated.len() <= TELEGRAM_CHAR_LIMIT);
        assert!(truncated.ends_with("[... truncated]"));
    }

    #[test]
    fn truncate_preserves_complete_lines() {
        let mut text = String::new();
        for i in 0..500 {
            text.push_str(&format!("Line {i}: some content here\n"));
        }
        let truncated = truncate_for_telegram(&text);
        assert!(truncated.len() <= TELEGRAM_CHAR_LIMIT);
        // Should not cut mid-line
        let before_indicator = truncated
            .strip_suffix("\n\n[... truncated]")
            .unwrap_or(&truncated);
        assert!(before_indicator.ends_with('\n') || before_indicator.ends_with("content here"));
    }

    #[test]
    fn max_action_buttons() {
        let actions: Vec<SuggestedAction> = (0..10)
            .map(|i| SuggestedAction {
                id: format!("act_{i}"),
                label: format!("Action {i}"),
                action_type: DigestActionType::FollowUpQuery,
                payload: serde_json::json!({}),
                status: DigestActionStatus::Pending,
            })
            .collect();

        let kb = build_action_keyboard(&actions).unwrap();
        // MAX_ACTION_BUTTONS action rows + 1 dismiss row
        assert_eq!(kb.rows.len(), MAX_ACTION_BUTTONS + 1);
    }

    #[test]
    fn long_action_label_truncated() {
        let actions = vec![SuggestedAction {
            id: "act_1".into(),
            label: "A".repeat(100),
            action_type: DigestActionType::FollowUpQuery,
            payload: serde_json::json!({}),
            status: DigestActionStatus::Pending,
        }];
        let kb = build_action_keyboard(&actions).unwrap();
        assert!(kb.rows[0][0].text.len() <= 63);
        assert!(kb.rows[0][0].text.ends_with("..."));
    }

    #[test]
    fn empty_actions_no_keyboard() {
        let kb = build_action_keyboard(&[]);
        assert!(kb.is_none());
    }
}
