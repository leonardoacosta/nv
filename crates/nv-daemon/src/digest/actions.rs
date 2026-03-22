use anyhow::Result;

use super::state::{DigestActionStatus, DigestStateManager, SuggestedAction};

// ── Action Handling ─────────────────────────────────────────────────

/// Handle a digest action callback from Telegram.
///
/// Loads the current digest state, finds the matching action, marks it
/// as completed, and returns the action details for execution.
pub fn handle_digest_action(
    state_mgr: &DigestStateManager,
    action_id: &str,
) -> Result<Option<SuggestedAction>> {
    state_mgr.update_action_status(action_id, DigestActionStatus::Completed)
}

/// Handle the "Dismiss All" callback.
///
/// Marks all pending actions as dismissed.
pub fn dismiss_all_actions(state_mgr: &DigestStateManager) -> Result<u32> {
    let state = state_mgr.load()?;
    let mut dismissed_count = 0;

    for action in &state.suggested_actions {
        if action.status == DigestActionStatus::Pending {
            state_mgr.update_action_status(&action.id, DigestActionStatus::Dismissed)?;
            dismissed_count += 1;
        }
    }

    Ok(dismissed_count)
}

/// Check if a callback_data string is a digest action.
pub fn is_digest_callback(callback_data: &str) -> bool {
    callback_data.starts_with("digest_act:") || callback_data == "digest_dismiss"
}

/// Extract the action ID from a "digest_act:ACTION_ID" callback.
pub fn extract_action_id(callback_data: &str) -> Option<&str> {
    callback_data.strip_prefix("digest_act:")
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::state::{DigestActionType, SuggestedAction};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn setup() -> (TempDir, DigestStateManager) {
        let dir = TempDir::new().unwrap();
        let state_dir = dir.path().join("state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("last-digest.json"), "{}").unwrap();
        let mgr = DigestStateManager::new(dir.path());
        (dir, mgr)
    }

    #[test]
    fn handle_action_completes_it() {
        let (_dir, mgr) = setup();
        let actions = vec![SuggestedAction {
            id: "act_1".into(),
            label: "Close OO-142".into(),
            action_type: DigestActionType::JiraTransition,
            payload: serde_json::json!({"issue_key": "OO-142"}),
            status: DigestActionStatus::Pending,
        }];
        mgr.record_sent("sha256:x", actions, HashMap::new())
            .unwrap();

        let result = handle_digest_action(&mgr, "act_1").unwrap();
        assert!(result.is_some());
        let action = result.unwrap();
        assert_eq!(action.id, "act_1");
        assert_eq!(action.status, DigestActionStatus::Completed);
    }

    #[test]
    fn handle_action_not_found() {
        let (_dir, mgr) = setup();
        mgr.record_sent("sha256:x", vec![], HashMap::new())
            .unwrap();
        let result = handle_digest_action(&mgr, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn dismiss_all_marks_pending() {
        let (_dir, mgr) = setup();
        let actions = vec![
            SuggestedAction {
                id: "act_1".into(),
                label: "First".into(),
                action_type: DigestActionType::JiraTransition,
                payload: serde_json::json!({}),
                status: DigestActionStatus::Pending,
            },
            SuggestedAction {
                id: "act_2".into(),
                label: "Second".into(),
                action_type: DigestActionType::FollowUpQuery,
                payload: serde_json::json!({}),
                status: DigestActionStatus::Pending,
            },
        ];
        mgr.record_sent("sha256:x", actions, HashMap::new())
            .unwrap();

        let count = dismiss_all_actions(&mgr).unwrap();
        assert_eq!(count, 2);

        let state = mgr.load().unwrap();
        assert!(state
            .suggested_actions
            .iter()
            .all(|a| a.status == DigestActionStatus::Dismissed));
    }

    #[test]
    fn is_digest_callback_matches() {
        assert!(is_digest_callback("digest_act:act_1"));
        assert!(is_digest_callback("digest_dismiss"));
        assert!(!is_digest_callback("approve:abc-123"));
        assert!(!is_digest_callback("action:xyz"));
    }

    #[test]
    fn extract_action_id_works() {
        assert_eq!(extract_action_id("digest_act:act_1"), Some("act_1"));
        assert_eq!(
            extract_action_id("digest_act:digest_act_3"),
            Some("digest_act_3")
        );
        assert_eq!(extract_action_id("digest_dismiss"), None);
        assert_eq!(extract_action_id("other:data"), None);
    }
}
