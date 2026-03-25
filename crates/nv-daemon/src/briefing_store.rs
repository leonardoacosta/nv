//! BriefingStore — persist and retrieve morning briefing entries as JSONL.
//!
//! Entries are stored in `~/.nv/state/briefing-log.jsonl`, one JSON object per line,
//! capped at the 30 most recent entries. Reads return newest-first.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Data Types ───────────────────────────────────────────────────────

/// A single suggested action extracted from a morning briefing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    /// Short label for the action (e.g. "Review P0 obligations").
    pub label: String,
    /// Optional URL or deep-link associated with the action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A single morning briefing entry stored in the JSONL log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingEntry {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// UTC timestamp when the briefing was generated.
    pub generated_at: DateTime<Utc>,
    /// Full text content of the briefing (HTML or plain).
    pub content: String,
    /// Optional structured action suggestions.
    #[serde(default)]
    pub suggested_actions: Vec<SuggestedAction>,
    /// Status of each data source used to build the briefing
    /// (e.g. `{"obligations": "ok", "calendar": "unavailable"}`).
    #[serde(default)]
    pub sources_status: HashMap<String, String>,
}

impl BriefingEntry {
    /// Create a new entry with a fresh UUID and the current UTC timestamp.
    pub fn new(
        content: impl Into<String>,
        suggested_actions: Vec<SuggestedAction>,
        sources_status: HashMap<String, String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            generated_at: Utc::now(),
            content: content.into(),
            suggested_actions,
            sources_status,
        }
    }
}

// ── Store ────────────────────────────────────────────────────────────

/// Maximum number of briefing entries retained in the log.
const MAX_ENTRIES: usize = 30;

/// Persistent log of morning briefing entries.
///
/// Operations are synchronous and use standard file I/O; intended for
/// low-frequency writes (once per day). Thread-safety is the caller's
/// responsibility — wrap in a `Mutex` if needed.
pub struct BriefingStore {
    path: PathBuf,
}

impl BriefingStore {
    /// Create a new `BriefingStore` rooted at the given `nv_base` directory.
    ///
    /// The log file lives at `<nv_base>/state/briefing-log.jsonl`. The
    /// directory is created lazily on first write.
    pub fn new(nv_base: &std::path::Path) -> Self {
        Self {
            path: nv_base.join("state").join("briefing-log.jsonl"),
        }
    }

    /// Append an entry to the log, then trim to the last [`MAX_ENTRIES`] entries.
    ///
    /// The trim is a read-rewrite cycle: all entries are read, the oldest are
    /// discarded if the cap would be exceeded, and the file is rewritten atomically
    /// (write-to-temp, then rename).
    pub fn append(&self, entry: &BriefingEntry) -> Result<()> {
        // Ensure parent directory exists.
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Read existing entries (newest-first after list()).
        let mut entries = self.list(MAX_ENTRIES)?;

        // Prepend the new entry so it becomes the newest.
        entries.insert(0, entry.clone());

        // Trim to cap.
        entries.truncate(MAX_ENTRIES);

        // Write all entries back (oldest-first on disk — `list()` reverses).
        self.write_all(&entries)?;

        Ok(())
    }

    /// Return up to `limit` entries, newest first.
    pub fn list(&self, limit: usize) -> Result<Vec<BriefingEntry>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }

        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);

        let mut entries: Vec<BriefingEntry> = reader
            .lines()
            .filter_map(|line| {
                let line = line.ok()?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                serde_json::from_str(trimmed).ok()
            })
            .collect();

        // Disk order is oldest-first (as written by write_all). Reverse for newest-first.
        entries.reverse();

        entries.truncate(limit);

        Ok(entries)
    }

    /// Return the most recent entry, or `None` if the store is empty.
    pub fn latest(&self) -> Result<Option<BriefingEntry>> {
        Ok(self.list(1)?.into_iter().next())
    }

    // ── Internal helpers ─────────────────────────────────────────────

    /// Write `entries` (newest-first) to disk, stored oldest-first for append
    /// friendliness. Uses a temp-file + rename for atomicity.
    fn write_all(&self, entries: &[BriefingEntry]) -> Result<()> {
        // Ensure parent exists.
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write to a sibling temp file.
        let tmp_path = self.path.with_extension("jsonl.tmp");
        let mut file = std::fs::File::create(&tmp_path)?;

        // Write oldest-first so new appends keep chronological order.
        for entry in entries.iter().rev() {
            let line = serde_json::to_string(entry)?;
            writeln!(file, "{}", line)?;
        }

        file.flush()?;
        drop(file);

        // Atomic rename.
        std::fs::rename(&tmp_path, &self.path)?;

        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_entry(content: &str) -> BriefingEntry {
        BriefingEntry::new(content, vec![], HashMap::new())
    }

    fn make_store(tmp: &TempDir) -> BriefingStore {
        BriefingStore::new(tmp.path())
    }

    // [2.4] test list_empty_store
    #[test]
    fn list_empty_store() {
        let tmp = TempDir::new().unwrap();
        let store = make_store(&tmp);
        let result = store.list(10).unwrap();
        assert!(result.is_empty(), "expected empty list for new store");
    }

    // [2.1] test append_and_list_round_trip
    #[test]
    fn append_and_list_round_trip() {
        let tmp = TempDir::new().unwrap();
        let store = make_store(&tmp);

        let e1 = make_entry("First briefing");
        let e2 = make_entry("Second briefing");

        store.append(&e1).unwrap();
        store.append(&e2).unwrap();

        let entries = store.list(10).unwrap();
        assert_eq!(entries.len(), 2);

        // Newest first — e2 was appended last.
        assert_eq!(entries[0].content, "Second briefing");
        assert_eq!(entries[1].content, "First briefing");
    }

    // [2.2] test cap_at_30_entries
    #[test]
    fn cap_at_30_entries() {
        let tmp = TempDir::new().unwrap();
        let store = make_store(&tmp);

        // Append 35 entries.
        for i in 0..35_usize {
            let e = make_entry(&format!("Briefing {i}"));
            store.append(&e).unwrap();
        }

        let entries = store.list(100).unwrap();
        assert_eq!(
            entries.len(),
            MAX_ENTRIES,
            "store must not exceed {MAX_ENTRIES} entries"
        );

        // The 5 oldest (0-4) should have been dropped; newest is "Briefing 34".
        assert_eq!(entries[0].content, "Briefing 34");
        assert_eq!(entries[MAX_ENTRIES - 1].content, "Briefing 5");
    }

    // [2.3] test latest_returns_most_recent
    #[test]
    fn latest_returns_most_recent() {
        let tmp = TempDir::new().unwrap();
        let store = make_store(&tmp);

        store.append(&make_entry("Alpha")).unwrap();
        store.append(&make_entry("Beta")).unwrap();
        store.append(&make_entry("Gamma")).unwrap();

        let latest = store.latest().unwrap().expect("should have a latest entry");
        assert_eq!(latest.content, "Gamma");
    }
}
