use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use nv_core::types::FollowUpAction;
use serde::{Deserialize, Serialize};

/// Follow-up context TTL in minutes.
const FOLLOWUP_TTL_MINUTES: i64 = 5;

/// Persistent follow-up state stored in `~/.nv/state/query-context.json`.
///
/// After a query answer, stores the suggested follow-up actions so the
/// user can reference them in subsequent messages ("do the first one").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUpState {
    pub query_id: String,
    pub asked_at: DateTime<Utc>,
    pub ttl_minutes: i64,
    pub original_question: String,
    pub answer_summary: String,
    pub followups: Vec<FollowUpAction>,
}

impl FollowUpState {
    /// Check if this follow-up state has expired.
    pub fn is_expired(&self) -> bool {
        let elapsed = Utc::now() - self.asked_at;
        elapsed.num_minutes() >= self.ttl_minutes
    }
}

/// Manager for follow-up state persistence.
pub struct FollowUpManager {
    state_path: PathBuf,
}

impl FollowUpManager {
    /// Create a new FollowUpManager rooted at `base_path/state/`.
    pub fn new(base_path: &Path) -> Self {
        Self {
            state_path: base_path.join("state").join("query-context.json"),
        }
    }

    /// Store follow-up state after a query answer.
    pub fn store(&self, state: &FollowUpState) -> Result<()> {
        let content = serde_json::to_string_pretty(state)
            .context("failed to serialize follow-up state")?;
        fs::write(&self.state_path, content)
            .with_context(|| {
                format!(
                    "failed to write follow-up state: {}",
                    self.state_path.display()
                )
            })?;
        tracing::debug!(query_id = %state.query_id, "stored follow-up state");
        Ok(())
    }

    /// Load follow-up state if it exists and has not expired.
    ///
    /// Returns `None` if no state exists, or if the state has expired
    /// (in which case the file is also cleaned up).
    pub fn load(&self) -> Result<Option<FollowUpState>> {
        if !self.state_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.state_path)
            .with_context(|| "failed to read query-context.json")?;

        let state: FollowUpState = serde_json::from_str(&content)
            .with_context(|| "failed to parse query-context.json")?;

        if state.is_expired() {
            tracing::debug!(
                query_id = %state.query_id,
                "follow-up state expired, clearing"
            );
            self.clear()?;
            return Ok(None);
        }

        Ok(Some(state))
    }

    /// Clear the follow-up state file.
    pub fn clear(&self) -> Result<()> {
        if self.state_path.exists() {
            fs::remove_file(&self.state_path)
                .with_context(|| "failed to remove query-context.json")?;
            tracing::debug!("cleared follow-up state");
        }
        Ok(())
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::types::ActionType;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FollowUpManager) {
        let dir = TempDir::new().unwrap();
        let state_dir = dir.path().join("state");
        fs::create_dir_all(&state_dir).unwrap();
        let manager = FollowUpManager::new(dir.path());
        (dir, manager)
    }

    fn sample_state() -> FollowUpState {
        FollowUpState {
            query_id: "q_test123".into(),
            asked_at: Utc::now(),
            ttl_minutes: FOLLOWUP_TTL_MINUTES,
            original_question: "What's blocking OO?".into(),
            answer_summary: "OO-42 is blocking the release".into(),
            followups: vec![FollowUpAction {
                index: 1,
                label: "Transition OO-42 to In Progress".into(),
                action_type: ActionType::JiraTransition,
                payload: serde_json::json!({
                    "issue_key": "OO-42",
                    "transition_name": "In Progress"
                }),
            }],
        }
    }

    #[test]
    fn store_and_load_followup() {
        let (_dir, manager) = setup();
        let state = sample_state();

        manager.store(&state).unwrap();

        let loaded = manager.load().unwrap().unwrap();
        assert_eq!(loaded.query_id, "q_test123");
        assert_eq!(loaded.followups.len(), 1);
        assert_eq!(loaded.followups[0].label, "Transition OO-42 to In Progress");
    }

    #[test]
    fn load_returns_none_when_no_file() {
        let (_dir, manager) = setup();
        let loaded = manager.load().unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn load_returns_none_when_expired() {
        let (_dir, manager) = setup();
        let mut state = sample_state();
        // Set asked_at to 10 minutes ago (past TTL)
        state.asked_at = Utc::now() - chrono::Duration::minutes(10);

        manager.store(&state).unwrap();

        let loaded = manager.load().unwrap();
        assert!(loaded.is_none());
        // File should be cleaned up
        assert!(!manager.state_path.exists());
    }

    #[test]
    fn clear_removes_file() {
        let (_dir, manager) = setup();
        let state = sample_state();

        manager.store(&state).unwrap();
        assert!(manager.state_path.exists());

        manager.clear().unwrap();
        assert!(!manager.state_path.exists());
    }

    #[test]
    fn clear_is_idempotent() {
        let (_dir, manager) = setup();
        // Clear when file doesn't exist should not error
        manager.clear().unwrap();
        manager.clear().unwrap();
    }

    #[test]
    fn is_expired_checks_ttl() {
        let mut state = sample_state();
        assert!(!state.is_expired());

        state.asked_at = Utc::now() - chrono::Duration::minutes(6);
        assert!(state.is_expired());
    }
}
