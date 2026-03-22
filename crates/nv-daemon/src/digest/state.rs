use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest as Sha2Digest, Sha256};

// ── Types ───────────────────────────────────────────────────────────

/// Persisted digest state in `~/.nv/state/last-digest.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DigestState {
    pub last_sent_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub suggested_actions: Vec<SuggestedAction>,
    pub sources_status: HashMap<String, String>,
}

/// A suggested action attached to a digest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub id: String,
    pub label: String,
    pub action_type: DigestActionType,
    pub payload: serde_json::Value,
    pub status: DigestActionStatus,
}

/// The kind of action suggested in a digest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DigestActionType {
    JiraTransition,
    MemoryWrite,
    FollowUpQuery,
}

/// Status of a suggested digest action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DigestActionStatus {
    Pending,
    Completed,
    Dismissed,
}

// ── DigestStateManager ──────────────────────────────────────────────

/// Reads and writes `~/.nv/state/last-digest.json`.
pub struct DigestStateManager {
    path: PathBuf,
}

impl DigestStateManager {
    pub fn new(nv_base: &Path) -> Self {
        Self {
            path: nv_base.join("state").join("last-digest.json"),
        }
    }

    /// Load current digest state (returns default if file is empty/missing).
    pub fn load(&self) -> Result<DigestState> {
        if !self.path.exists() {
            return Ok(DigestState::default());
        }
        let content = fs::read_to_string(&self.path)
            .with_context(|| "failed to read last-digest.json")?;
        if content.trim() == "{}" || content.trim().is_empty() {
            return Ok(DigestState::default());
        }
        serde_json::from_str(&content).with_context(|| "failed to parse last-digest.json")
    }

    /// Persist digest state.
    pub fn save(&self, state: &DigestState) -> Result<()> {
        let content = serde_json::to_string_pretty(state)?;
        atomic_write(&self.path, &content)
    }

    /// Check whether a new digest should be sent.
    ///
    /// Returns `true` if:
    /// - No digest has been sent yet, OR
    /// - Enough time has elapsed since the last digest, OR
    /// - The content hash differs from the last digest
    pub fn should_send(&self, interval_minutes: u64, new_content_hash: Option<&str>) -> Result<bool> {
        let state = self.load()?;

        // Never sent — always send
        let Some(last_sent) = state.last_sent_at else {
            return Ok(true);
        };

        // Check interval
        let interval = ChronoDuration::minutes(interval_minutes as i64);
        if Utc::now() - last_sent >= interval {
            return Ok(true);
        }

        // Check content hash (if provided and differs)
        if let Some(new_hash) = new_content_hash {
            if state.content_hash.as_deref() != Some(new_hash) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Seconds until the next digest is due (0 if overdue or never sent).
    pub fn seconds_until_next(&self, interval_minutes: u64) -> Result<u64> {
        let state = self.load()?;
        let Some(last_sent) = state.last_sent_at else {
            return Ok(0);
        };
        let interval = ChronoDuration::minutes(interval_minutes as i64);
        let next_due = last_sent + interval;
        let now = Utc::now();
        if now >= next_due {
            Ok(0)
        } else {
            Ok((next_due - now).num_seconds() as u64)
        }
    }

    /// Update state after a digest is sent.
    pub fn record_sent(
        &self,
        content_hash: &str,
        actions: Vec<SuggestedAction>,
        sources: HashMap<String, String>,
    ) -> Result<()> {
        let state = DigestState {
            last_sent_at: Some(Utc::now()),
            content_hash: Some(content_hash.to_string()),
            suggested_actions: actions,
            sources_status: sources,
        };
        self.save(&state)
    }

    /// Mark a suggested action as completed or dismissed.
    pub fn update_action_status(&self, action_id: &str, status: DigestActionStatus) -> Result<Option<SuggestedAction>> {
        let mut state = self.load()?;
        let action = state
            .suggested_actions
            .iter_mut()
            .find(|a| a.id == action_id);
        let result = action.map(|a| {
            a.status = status;
            a.clone()
        });
        self.save(&state)?;
        Ok(result)
    }
}

/// Compute SHA-256 hash of content for dedup.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Write content atomically (write to .tmp, then rename).
fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, content)
        .with_context(|| format!("failed to write tmp file: {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path)
        .with_context(|| format!("failed to rename tmp to: {}", path.display()))?;
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, DigestStateManager) {
        let dir = TempDir::new().unwrap();
        let state_dir = dir.path().join("state");
        fs::create_dir_all(&state_dir).unwrap();
        fs::write(state_dir.join("last-digest.json"), "{}").unwrap();
        let mgr = DigestStateManager::new(dir.path());
        (dir, mgr)
    }

    #[test]
    fn load_empty_state() {
        let (_dir, mgr) = setup();
        let state = mgr.load().unwrap();
        assert!(state.last_sent_at.is_none());
        assert!(state.content_hash.is_none());
        assert!(state.suggested_actions.is_empty());
    }

    #[test]
    fn save_and_load_round_trip() {
        let (_dir, mgr) = setup();
        let state = DigestState {
            last_sent_at: Some(Utc::now()),
            content_hash: Some("sha256:abc".into()),
            suggested_actions: vec![SuggestedAction {
                id: "act_1".into(),
                label: "Close OO-142".into(),
                action_type: DigestActionType::JiraTransition,
                payload: serde_json::json!({"issue_key": "OO-142"}),
                status: DigestActionStatus::Pending,
            }],
            sources_status: HashMap::from([
                ("jira".into(), "ok".into()),
                ("memory".into(), "ok".into()),
            ]),
        };
        mgr.save(&state).unwrap();

        let loaded = mgr.load().unwrap();
        assert!(loaded.last_sent_at.is_some());
        assert_eq!(loaded.content_hash.as_deref(), Some("sha256:abc"));
        assert_eq!(loaded.suggested_actions.len(), 1);
        assert_eq!(loaded.suggested_actions[0].id, "act_1");
        assert_eq!(loaded.sources_status.get("jira").unwrap(), "ok");
    }

    #[test]
    fn should_send_when_never_sent() {
        let (_dir, mgr) = setup();
        assert!(mgr.should_send(60, None).unwrap());
    }

    #[test]
    fn should_send_after_interval() {
        let (_dir, mgr) = setup();
        let state = DigestState {
            last_sent_at: Some(Utc::now() - ChronoDuration::minutes(120)),
            content_hash: Some("sha256:old".into()),
            ..Default::default()
        };
        mgr.save(&state).unwrap();
        assert!(mgr.should_send(60, None).unwrap());
    }

    #[test]
    fn should_not_send_within_interval() {
        let (_dir, mgr) = setup();
        let state = DigestState {
            last_sent_at: Some(Utc::now()),
            content_hash: Some("sha256:current".into()),
            ..Default::default()
        };
        mgr.save(&state).unwrap();
        assert!(!mgr.should_send(60, Some("sha256:current")).unwrap());
    }

    #[test]
    fn should_send_when_hash_differs() {
        let (_dir, mgr) = setup();
        let state = DigestState {
            last_sent_at: Some(Utc::now()),
            content_hash: Some("sha256:old".into()),
            ..Default::default()
        };
        mgr.save(&state).unwrap();
        assert!(mgr.should_send(60, Some("sha256:new")).unwrap());
    }

    #[test]
    fn seconds_until_next_never_sent() {
        let (_dir, mgr) = setup();
        assert_eq!(mgr.seconds_until_next(60).unwrap(), 0);
    }

    #[test]
    fn seconds_until_next_overdue() {
        let (_dir, mgr) = setup();
        let state = DigestState {
            last_sent_at: Some(Utc::now() - ChronoDuration::minutes(120)),
            ..Default::default()
        };
        mgr.save(&state).unwrap();
        assert_eq!(mgr.seconds_until_next(60).unwrap(), 0);
    }

    #[test]
    fn seconds_until_next_future() {
        let (_dir, mgr) = setup();
        let state = DigestState {
            last_sent_at: Some(Utc::now()),
            ..Default::default()
        };
        mgr.save(&state).unwrap();
        let secs = mgr.seconds_until_next(60).unwrap();
        // Should be roughly 3600 seconds (60 minutes)
        assert!(secs > 3500 && secs <= 3600);
    }

    #[test]
    fn record_sent_and_load() {
        let (_dir, mgr) = setup();
        let actions = vec![SuggestedAction {
            id: "act_1".into(),
            label: "Test".into(),
            action_type: DigestActionType::FollowUpQuery,
            payload: serde_json::json!({}),
            status: DigestActionStatus::Pending,
        }];
        let sources = HashMap::from([("jira".into(), "ok".into())]);
        mgr.record_sent("sha256:hash", actions, sources).unwrap();

        let loaded = mgr.load().unwrap();
        assert!(loaded.last_sent_at.is_some());
        assert_eq!(loaded.content_hash.as_deref(), Some("sha256:hash"));
        assert_eq!(loaded.suggested_actions.len(), 1);
    }

    #[test]
    fn update_action_status() {
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
                action_type: DigestActionType::MemoryWrite,
                payload: serde_json::json!({}),
                status: DigestActionStatus::Pending,
            },
        ];
        mgr.record_sent("sha256:x", actions, HashMap::new()).unwrap();

        let updated = mgr
            .update_action_status("act_1", DigestActionStatus::Completed)
            .unwrap();
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().status, DigestActionStatus::Completed);

        let loaded = mgr.load().unwrap();
        assert_eq!(loaded.suggested_actions[0].status, DigestActionStatus::Completed);
        assert_eq!(loaded.suggested_actions[1].status, DigestActionStatus::Pending);
    }

    #[test]
    fn update_action_status_not_found() {
        let (_dir, mgr) = setup();
        mgr.record_sent("sha256:x", vec![], HashMap::new()).unwrap();
        let result = mgr
            .update_action_status("nonexistent", DigestActionStatus::Completed)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256:"));
    }

    #[test]
    fn content_hash_differs_for_different_input() {
        let h1 = content_hash("hello");
        let h2 = content_hash("world");
        assert_ne!(h1, h2);
    }
}
