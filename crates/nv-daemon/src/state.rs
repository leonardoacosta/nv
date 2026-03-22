use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── State Types ─────────────────────────────────────────────────────

/// Last digest metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LastDigest {
    pub timestamp: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub actions_suggested: u32,
    pub actions_taken: u32,
}

/// A pending action awaiting user confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    pub id: Uuid,
    pub description: String,
    pub payload: serde_json::Value,
    pub status: PendingStatus,
    pub created_at: DateTime<Utc>,
    /// Telegram message ID where the confirmation keyboard was sent.
    #[serde(default)]
    pub telegram_message_id: Option<i64>,
    /// Telegram chat ID where the confirmation keyboard was sent.
    #[serde(default)]
    pub telegram_chat_id: Option<i64>,
}

/// Status of a pending action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PendingStatus {
    AwaitingConfirmation,
    Approved,
    Rejected,
    Executed,
    Cancelled,
    Expired,
}

/// Wrapper for the pending-actions.json file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PendingActionsFile {
    pub actions: Vec<PendingAction>,
}

/// Per-channel state (cursor/offset for polling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    pub last_update_id: Option<i64>,
    pub last_poll_at: Option<DateTime<Utc>>,
}

// ── State System ────────────────────────────────────────────────────

/// Daemon state persistence backed by JSON files in `~/.nv/state/`.
pub struct State {
    base_path: PathBuf,
}

impl State {
    /// Create a new State instance rooted at `base_path/state/`.
    pub fn new(base_path: &Path) -> Self {
        Self {
            base_path: base_path.join("state"),
        }
    }

    /// Initialize the state directory and default files.
    ///
    /// Creates the directory and empty JSON files if they do not exist.
    /// Idempotent — safe to call multiple times.
    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.base_path)
            .with_context(|| format!("failed to create state dir: {}", self.base_path.display()))?;

        for name in &[
            "last-digest.json",
            "pending-actions.json",
            "channel-state.json",
        ] {
            let path = self.base_path.join(name);
            if !path.exists() {
                fs::write(&path, "{}")
                    .with_context(|| format!("failed to create {name}"))?;
            }
        }

        tracing::info!(path = %self.base_path.display(), "state directory initialized");
        Ok(())
    }

    // ── Last Digest ─────────────────────────────────────────────────

    /// Read the last digest metadata.
    pub fn read_last_digest(&self) -> Result<LastDigest> {
        let path = self.base_path.join("last-digest.json");
        let content = fs::read_to_string(&path)
            .with_context(|| "failed to read last-digest.json")?;

        // Handle empty object "{}" gracefully
        if content.trim() == "{}" {
            return Ok(LastDigest::default());
        }

        serde_json::from_str(&content)
            .with_context(|| "failed to parse last-digest.json")
    }

    /// Write the last digest metadata.
    pub fn write_last_digest(&self, digest: &LastDigest) -> Result<()> {
        let path = self.base_path.join("last-digest.json");
        let content = serde_json::to_string_pretty(digest)?;
        atomic_write(&path, &content)
    }

    // ── Pending Actions ─────────────────────────────────────────────

    /// Load all pending actions.
    pub fn load_pending_actions(&self) -> Result<Vec<PendingAction>> {
        let path = self.base_path.join("pending-actions.json");
        let content = fs::read_to_string(&path)
            .with_context(|| "failed to read pending-actions.json")?;

        // Handle empty object "{}" gracefully
        if content.trim() == "{}" {
            return Ok(Vec::new());
        }

        let file: PendingActionsFile = serde_json::from_str(&content)
            .with_context(|| "failed to parse pending-actions.json")?;
        Ok(file.actions)
    }

    /// Save a new pending action (appends to existing list).
    pub fn save_pending_action(&self, action: &PendingAction) -> Result<()> {
        let mut actions = self.load_pending_actions()?;
        actions.push(action.clone());
        self.write_pending_actions(&actions)
    }

    /// Update the status of a pending action by ID.
    pub fn update_pending_action(&self, id: &Uuid, status: PendingStatus) -> Result<()> {
        let mut actions = self.load_pending_actions()?;
        if let Some(action) = actions.iter_mut().find(|a| a.id == *id) {
            action.status = status;
        }
        self.write_pending_actions(&actions)
    }

    /// Find a pending action by ID.
    pub fn find_pending_action(&self, id: &Uuid) -> Result<Option<PendingAction>> {
        let actions = self.load_pending_actions()?;
        Ok(actions.into_iter().find(|a| a.id == *id))
    }

    /// Update the payload of a pending action by ID.
    pub fn update_pending_action_payload(&self, id: &Uuid, payload: serde_json::Value) -> Result<()> {
        let mut actions = self.load_pending_actions()?;
        if let Some(action) = actions.iter_mut().find(|a| a.id == *id) {
            action.payload = payload;
        }
        self.write_pending_actions(&actions)
    }

    /// Remove a pending action by ID.
    pub fn remove_pending_action(&self, id: &Uuid) -> Result<()> {
        let mut actions = self.load_pending_actions()?;
        actions.retain(|a| a.id != *id);
        self.write_pending_actions(&actions)
    }

    /// Get the state directory path (for external callers that need it).
    pub fn base_path(&self) -> &std::path::Path {
        &self.base_path
    }

    fn write_pending_actions(&self, actions: &[PendingAction]) -> Result<()> {
        let path = self.base_path.join("pending-actions.json");
        let file = PendingActionsFile {
            actions: actions.to_vec(),
        };
        let content = serde_json::to_string_pretty(&file)?;
        atomic_write(&path, &content)
    }

    // ── Channel State ───────────────────────────────────────────────

    /// Load the state for a specific channel.
    pub fn load_channel_state(&self, channel: &str) -> Result<Option<ChannelState>> {
        let path = self.base_path.join("channel-state.json");
        let content = fs::read_to_string(&path)
            .with_context(|| "failed to read channel-state.json")?;

        // Handle empty object "{}" gracefully
        if content.trim() == "{}" {
            return Ok(None);
        }

        let map: HashMap<String, ChannelState> = serde_json::from_str(&content)
            .with_context(|| "failed to parse channel-state.json")?;
        Ok(map.get(channel).cloned())
    }

    /// Save the state for a specific channel.
    pub fn save_channel_state(&self, channel: &str, state: &ChannelState) -> Result<()> {
        let path = self.base_path.join("channel-state.json");
        let content = fs::read_to_string(&path)
            .with_context(|| "failed to read channel-state.json")?;

        let mut map: HashMap<String, ChannelState> = if content.trim() == "{}" {
            HashMap::new()
        } else {
            serde_json::from_str(&content)
                .with_context(|| "failed to parse channel-state.json")?
        };

        map.insert(channel.to_string(), state.clone());

        let updated = serde_json::to_string_pretty(&map)?;
        atomic_write(&path, &updated)
    }
}

/// Write content to a file atomically (write to .tmp, then rename).
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

    fn setup() -> (TempDir, State) {
        let dir = TempDir::new().unwrap();
        let state = State::new(dir.path());
        state.init().unwrap();
        (dir, state)
    }

    #[test]
    fn init_creates_directory_and_files() {
        let dir = TempDir::new().unwrap();
        let state = State::new(dir.path());
        state.init().unwrap();

        assert!(dir.path().join("state").exists());
        assert!(dir.path().join("state/last-digest.json").exists());
        assert!(dir.path().join("state/pending-actions.json").exists());
        assert!(dir.path().join("state/channel-state.json").exists());
    }

    #[test]
    fn init_is_idempotent() {
        let (dir, state) = setup();

        // Write some state
        let digest = LastDigest {
            timestamp: Some(Utc::now()),
            content_hash: Some("abc".into()),
            actions_suggested: 3,
            actions_taken: 1,
        };
        state.write_last_digest(&digest).unwrap();

        // Re-init should not overwrite
        state.init().unwrap();

        let loaded = state.read_last_digest().unwrap();
        assert_eq!(loaded.content_hash.as_deref(), Some("abc"));

        drop(dir);
    }

    #[test]
    fn last_digest_read_empty() {
        let (_dir, state) = setup();
        let digest = state.read_last_digest().unwrap();
        assert!(digest.timestamp.is_none());
        assert_eq!(digest.actions_suggested, 0);
    }

    #[test]
    fn last_digest_write_and_read() {
        let (_dir, state) = setup();

        let now = Utc::now();
        let digest = LastDigest {
            timestamp: Some(now),
            content_hash: Some("sha256:abc123".into()),
            actions_suggested: 5,
            actions_taken: 2,
        };

        state.write_last_digest(&digest).unwrap();
        let loaded = state.read_last_digest().unwrap();

        assert_eq!(
            loaded.content_hash.as_deref(),
            Some("sha256:abc123")
        );
        assert_eq!(loaded.actions_suggested, 5);
        assert_eq!(loaded.actions_taken, 2);
    }

    #[test]
    fn pending_actions_empty() {
        let (_dir, state) = setup();
        let actions = state.load_pending_actions().unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn pending_action_save_and_load() {
        let (_dir, state) = setup();

        let action = PendingAction {
            id: Uuid::new_v4(),
            description: "Create P1 bug".into(),
            payload: serde_json::json!({"project": "NV"}),
            status: PendingStatus::AwaitingConfirmation,
            created_at: Utc::now(),
            telegram_message_id: None,
            telegram_chat_id: None,
        };

        state.save_pending_action(&action).unwrap();

        let loaded = state.load_pending_actions().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, action.id);
        assert_eq!(loaded[0].description, "Create P1 bug");
        assert_eq!(loaded[0].status, PendingStatus::AwaitingConfirmation);
    }

    #[test]
    fn pending_action_update_status() {
        let (_dir, state) = setup();

        let action = PendingAction {
            id: Uuid::new_v4(),
            description: "Test action".into(),
            payload: serde_json::json!({}),
            status: PendingStatus::AwaitingConfirmation,
            created_at: Utc::now(),
            telegram_message_id: None,
            telegram_chat_id: None,
        };

        state.save_pending_action(&action).unwrap();
        state
            .update_pending_action(&action.id, PendingStatus::Approved)
            .unwrap();

        let loaded = state.load_pending_actions().unwrap();
        assert_eq!(loaded[0].status, PendingStatus::Approved);
    }

    #[test]
    fn pending_action_remove() {
        let (_dir, state) = setup();

        let a1 = PendingAction {
            id: Uuid::new_v4(),
            description: "First".into(),
            payload: serde_json::json!({}),
            status: PendingStatus::AwaitingConfirmation,
            created_at: Utc::now(),
            telegram_message_id: None,
            telegram_chat_id: None,
        };
        let a2 = PendingAction {
            id: Uuid::new_v4(),
            description: "Second".into(),
            payload: serde_json::json!({}),
            status: PendingStatus::AwaitingConfirmation,
            created_at: Utc::now(),
            telegram_message_id: None,
            telegram_chat_id: None,
        };

        state.save_pending_action(&a1).unwrap();
        state.save_pending_action(&a2).unwrap();

        state.remove_pending_action(&a1.id).unwrap();

        let loaded = state.load_pending_actions().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, a2.id);
    }

    #[test]
    fn channel_state_empty() {
        let (_dir, state) = setup();
        let cs = state.load_channel_state("telegram").unwrap();
        assert!(cs.is_none());
    }

    #[test]
    fn channel_state_save_and_load() {
        let (_dir, state) = setup();

        let cs = ChannelState {
            last_update_id: Some(12345),
            last_poll_at: Some(Utc::now()),
        };

        state.save_channel_state("telegram", &cs).unwrap();

        let loaded = state.load_channel_state("telegram").unwrap().unwrap();
        assert_eq!(loaded.last_update_id, Some(12345));
    }

    #[test]
    fn channel_state_multiple_channels() {
        let (_dir, state) = setup();

        let tg = ChannelState {
            last_update_id: Some(100),
            last_poll_at: Some(Utc::now()),
        };
        let dc = ChannelState {
            last_update_id: Some(200),
            last_poll_at: Some(Utc::now()),
        };

        state.save_channel_state("telegram", &tg).unwrap();
        state.save_channel_state("discord", &dc).unwrap();

        let tg_loaded = state.load_channel_state("telegram").unwrap().unwrap();
        let dc_loaded = state.load_channel_state("discord").unwrap().unwrap();

        assert_eq!(tg_loaded.last_update_id, Some(100));
        assert_eq!(dc_loaded.last_update_id, Some(200));
    }
}
