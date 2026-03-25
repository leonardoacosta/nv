use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{Local, NaiveDate};

// ── Diary Entry ─────────────────────────────────────────────────────

/// A single interaction diary entry written after each trigger batch.
pub struct DiaryEntry {
    /// When the entry was created.
    pub timestamp: chrono::DateTime<Local>,
    /// The type of trigger (message, cron, nexus, cli).
    pub trigger_type: String,
    /// The source of the trigger (channel name, cron event, agent name, etc.).
    pub trigger_source: String,
    /// Number of triggers in the batch.
    pub trigger_count: usize,
    /// Names of tools called during the tool use loop.
    pub tools_called: Vec<String>,
    /// Summary of sources checked (e.g. "jira: 2 issues, memory: decisions").
    pub sources_checked: String,
    /// Narrative summary extracted from Claude's [SUMMARY:] tag or first sentence.
    pub result_summary: String,
    /// Input tokens from the Claude API response.
    pub tokens_in: u32,
    /// Output tokens from the Claude API response.
    pub tokens_out: u32,
    /// Human-readable session slug (e.g. "check-jira-sprint").
    pub slug: String,
}

// ── Diary Writer ────────────────────────────────────────────────────

/// Writes interaction diary entries to daily rolling markdown files.
pub struct DiaryWriter {
    base_path: PathBuf,
}

impl DiaryWriter {
    /// Create a new DiaryWriter with the given base directory (e.g. `~/.nv/diary/`).
    pub fn new(base_path: &Path) -> Self {
        Self {
            base_path: base_path.to_path_buf(),
        }
    }

    /// Create the diary directory if it doesn't exist.
    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.base_path)?;
        tracing::debug!(path = %self.base_path.display(), "diary directory initialized");
        Ok(())
    }

    /// Append a diary entry to the daily file.
    ///
    /// The file is named `YYYY-MM-DD.md` and created on first write of the day.
    pub fn write_entry(&self, entry: &DiaryEntry) -> Result<()> {
        let date = entry.timestamp.date_naive();
        let file_path = self.daily_file_path(date);

        let markdown = format_entry(entry);

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        file.write_all(markdown.as_bytes())?;

        tracing::debug!(
            file = %file_path.display(),
            trigger_type = %entry.trigger_type,
            "diary entry written"
        );

        Ok(())
    }

    /// Return the path for a given date's diary file.
    fn daily_file_path(&self, date: NaiveDate) -> PathBuf {
        self.base_path.join(format!("{}.md", date.format("%Y-%m-%d")))
    }
}

// ── Formatting ──────────────────────────────────────────────────────

/// Format a diary entry as markdown.
fn format_entry(entry: &DiaryEntry) -> String {
    let time = entry.timestamp.format("%H:%M");
    let tools = if entry.tools_called.is_empty() {
        "none".to_string()
    } else {
        entry.tools_called.join(", ")
    };

    format!(
        "## {time} — {} ({}) · {}\n\n\
         **Triggers:** {} ({})\n\
         **Tools called:** {tools}\n\
         **Sources checked:** {}\n\
         **Result:** {}\n\
         **Cost:** {} in + {} out tokens\n\n",
        entry.trigger_type,
        entry.trigger_source,
        entry.slug,
        entry.trigger_count,
        entry.trigger_type,
        entry.sources_checked,
        entry.result_summary,
        entry.tokens_in,
        entry.tokens_out,
    )
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_creates_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let diary_path = tmp.path().join("diary");
        let writer = DiaryWriter::new(&diary_path);
        writer.init().unwrap();
        assert!(diary_path.exists());
    }

    #[test]
    fn test_write_entry_creates_daily_file() {
        let tmp = tempfile::tempdir().unwrap();
        let diary_path = tmp.path().join("diary");
        let writer = DiaryWriter::new(&diary_path);
        writer.init().unwrap();

        let now = Local::now();
        let entry = DiaryEntry {
            timestamp: now,
            trigger_type: "message".into(),
            trigger_source: "telegram".into(),
            trigger_count: 1,
            tools_called: vec!["read_memory".into(), "jira_search".into()],
            sources_checked: "memory: decisions, jira: 2 issues".into(),
            result_summary: "sent reply".into(),
            tokens_in: 500,
            tokens_out: 120,
            slug: "sent-reply".into(),
        };

        writer.write_entry(&entry).unwrap();

        let expected_file = diary_path.join(format!("{}.md", now.format("%Y-%m-%d")));
        assert!(expected_file.exists());

        let content = fs::read_to_string(&expected_file).unwrap();
        assert!(content.contains("## "));
        assert!(content.contains("message"));
        assert!(content.contains("telegram"));
        assert!(content.contains("read_memory, jira_search"));
        assert!(content.contains("500 in + 120 out tokens"));
    }

    #[test]
    fn test_write_entry_appends_to_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let diary_path = tmp.path().join("diary");
        let writer = DiaryWriter::new(&diary_path);
        writer.init().unwrap();

        let now = Local::now();
        let entry1 = DiaryEntry {
            timestamp: now,
            trigger_type: "message".into(),
            trigger_source: "telegram".into(),
            trigger_count: 1,
            tools_called: vec![],
            sources_checked: "none".into(),
            result_summary: "sent reply".into(),
            tokens_in: 100,
            tokens_out: 50,
            slug: "sent-reply".into(),
        };

        let entry2 = DiaryEntry {
            timestamp: now,
            trigger_type: "cron".into(),
            trigger_source: "digest".into(),
            trigger_count: 1,
            tools_called: vec!["jira_search".into()],
            sources_checked: "jira: 5 issues".into(),
            result_summary: "suppressed digest".into(),
            tokens_in: 800,
            tokens_out: 200,
            slug: "digest".into(),
        };

        writer.write_entry(&entry1).unwrap();
        writer.write_entry(&entry2).unwrap();

        let file = diary_path.join(format!("{}.md", now.format("%Y-%m-%d")));
        let content = fs::read_to_string(file).unwrap();

        // Both entries should be present
        assert!(content.contains("message"));
        assert!(content.contains("cron"));
        assert!(content.contains("suppressed digest"));
    }

    #[test]
    fn test_format_entry_no_tools() {
        let now = Local::now();
        let entry = DiaryEntry {
            timestamp: now,
            trigger_type: "cron".into(),
            trigger_source: "digest".into(),
            trigger_count: 1,
            tools_called: vec![],
            sources_checked: "none".into(),
            result_summary: "suppressed digest".into(),
            tokens_in: 0,
            tokens_out: 0,
            slug: "digest".into(),
        };

        let formatted = format_entry(&entry);
        assert!(formatted.contains("**Tools called:** none"));
    }

    #[test]
    fn test_format_entry_includes_slug_in_heading() {
        let now = Local::now();
        let entry = DiaryEntry {
            timestamp: now,
            trigger_type: "message".into(),
            trigger_source: "telegram".into(),
            trigger_count: 1,
            tools_called: vec![],
            sources_checked: "none".into(),
            result_summary: "sent reply".into(),
            tokens_in: 10,
            tokens_out: 5,
            slug: "check-jira-sprint".into(),
        };

        let formatted = format_entry(&entry);
        // Heading must contain the slug separated by " · "
        assert!(
            formatted.contains(" · check-jira-sprint"),
            "heading must include slug with separator; got: {formatted:?}"
        );
        // Heading structure: ## HH:MM — trigger_type (trigger_source) · slug
        let heading_line = formatted.lines().next().expect("must have heading line");
        assert!(heading_line.starts_with("## "), "must be an h2 heading");
        assert!(heading_line.contains("message"), "must include trigger type");
        assert!(heading_line.contains("telegram"), "must include trigger source");
        assert!(heading_line.ends_with("check-jira-sprint"), "slug must be last in heading");
    }

    #[test]
    fn test_daily_file_path() {
        let writer = DiaryWriter::new(Path::new("/tmp/diary"));
        let date = NaiveDate::from_ymd_opt(2026, 3, 22).unwrap();
        let path = writer.daily_file_path(date);
        assert_eq!(path, PathBuf::from("/tmp/diary/2026-03-22.md"));
    }
}
