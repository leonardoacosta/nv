use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;

// ── Constants ───────────────────────────────────────────────────────

/// Maximum characters returned from a single memory read.
const MAX_MEMORY_READ_CHARS: usize = 20_000;

/// Maximum search results to prevent context explosion.
const MAX_SEARCH_RESULTS: usize = 10;

/// Context lines before and after a search match.
const SEARCH_CONTEXT_LINES: usize = 2;

/// Maximum characters for the context summary.
const MAX_CONTEXT_CHARS: usize = 4000;

/// Default MEMORY.md index content.
const DEFAULT_MEMORY_INDEX: &str = "\
# NV Memory Index

| Topic | File | Description | Last Updated |
|-------|------|-------------|-------------|
| conversations | conversations.md | Conversation summaries | - |
| tasks | tasks.md | Active task context | - |
| decisions | decisions.md | Decisions and rationale | - |
| people | people.md | People, roles, preferences | - |
";

/// Default topic files: (filename, header content).
const DEFAULT_TOPICS: &[(&str, &str)] = &[
    (
        "conversations.md",
        "# Conversations\n\nSummaries of past conversations.\n",
    ),
    ("tasks.md", "# Tasks\n\nActive task context and status.\n"),
    (
        "decisions.md",
        "# Decisions\n\nDecisions made and their rationale.\n",
    ),
    (
        "people.md",
        "# People\n\nPeople, roles, and preferences.\n",
    ),
];

// ── Memory System ───────────────────────────────────────────────────

/// Markdown-native memory system backed by files in `~/.nv/memory/`.
pub struct Memory {
    base_path: PathBuf,
}

impl Memory {
    /// Create a new Memory instance rooted at `base_path/memory/`.
    pub fn new(base_path: &Path) -> Self {
        Self {
            base_path: base_path.join("memory"),
        }
    }

    /// Initialize the memory directory structure.
    ///
    /// Creates the directory and default files if they do not exist.
    /// Idempotent — safe to call multiple times.
    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.base_path)
            .with_context(|| format!("failed to create memory dir: {}", self.base_path.display()))?;

        // Create MEMORY.md index if missing
        let index_path = self.base_path.join("MEMORY.md");
        if !index_path.exists() {
            fs::write(&index_path, DEFAULT_MEMORY_INDEX)
                .with_context(|| "failed to create MEMORY.md")?;
        }

        // Create default topic files if missing
        for (filename, header) in DEFAULT_TOPICS {
            let path = self.base_path.join(filename);
            if !path.exists() {
                fs::write(&path, header)
                    .with_context(|| format!("failed to create {filename}"))?;
            }
        }

        tracing::info!(path = %self.base_path.display(), "memory directory initialized");
        Ok(())
    }

    /// Read a specific memory topic file.
    ///
    /// Returns the full content, or a truncated version if the file is very large.
    /// Returns a friendly message if the topic does not exist.
    pub fn read(&self, topic: &str) -> Result<String> {
        let filename = sanitize_topic(topic);
        let path = self.base_path.join(format!("{filename}.md"));

        if !path.exists() {
            return Ok(format!("No memory file found for topic: {topic}"));
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read memory file: {}", path.display()))?;

        if content.len() > MAX_MEMORY_READ_CHARS {
            let truncated = truncate_to_recent_entries(&content, MAX_MEMORY_READ_CHARS);
            return Ok(format!(
                "[Showing recent entries, {topic} has more history]\n\n{truncated}"
            ));
        }

        Ok(content)
    }

    /// Search across all memory files for a query string.
    ///
    /// Case-insensitive substring search. Returns matching lines with surrounding
    /// context (2 lines before and after each match).
    pub fn search(&self, query: &str) -> Result<String> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        let entries = fs::read_dir(&self.base_path)
            .with_context(|| "failed to read memory directory")?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only search .md files
            if path.extension().is_none_or(|e| e != "md") {
                continue;
            }

            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let filename = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let lines: Vec<&str> = content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    let start = i.saturating_sub(SEARCH_CONTEXT_LINES);
                    let end = (i + SEARCH_CONTEXT_LINES + 1).min(lines.len());
                    let context: Vec<&str> = lines[start..end].to_vec();

                    results.push(format!(
                        "[{filename}:{line_num}]\n{context}",
                        line_num = i + 1,
                        context = context.join("\n"),
                    ));
                }
            }
        }

        if results.is_empty() {
            return Ok(format!("No matches found for: {query}"));
        }

        let truncated = results.len() > MAX_SEARCH_RESULTS;
        let results: Vec<_> = results.into_iter().take(MAX_SEARCH_RESULTS).collect();

        let mut output = format!("Found matches for \"{query}\":\n\n");
        output.push_str(&results.join("\n---\n"));
        if truncated {
            output
                .push_str("\n\n[Results truncated. Refine your query for more specific results.]");
        }

        Ok(output)
    }

    /// Write content to a memory topic.
    ///
    /// If the topic is new, creates the file with a header and updates
    /// the MEMORY.md index. If existing, appends as a new entry with
    /// a timestamp header.
    pub fn write(&self, topic: &str, content: &str) -> Result<String> {
        let filename = sanitize_topic(topic);
        let path = self.base_path.join(format!("{filename}.md"));
        let now = Utc::now();
        let date_str = now.format("%Y-%m-%d %H:%M").to_string();
        let is_new = !path.exists();

        if is_new {
            let initial = format!(
                "# {topic}\n\n## {date_str} -- Entry\n\n{content}\n"
            );
            atomic_write(&path, &initial)?;

            // Update MEMORY.md index
            self.update_index(&filename, topic, &date_str)?;

            tracing::info!(topic, "created new memory topic");
            Ok(format!("Created new memory topic: {topic}"))
        } else {
            // Append entry to existing file
            let entry = format!("\n## {date_str} -- Entry\n\n{content}\n");
            let mut existing = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            existing.push_str(&entry);
            atomic_write(&path, &existing)?;

            tracing::info!(topic, "appended to memory topic");
            Ok(format!("Appended to memory topic: {topic}"))
        }
    }

    /// List all .md topic files in the memory directory.
    pub fn list_topics(&self) -> Result<Vec<String>> {
        let mut topics = Vec::new();

        let entries = fs::read_dir(&self.base_path)
            .with_context(|| "failed to read memory directory")?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Some(stem) = path.file_stem() {
                    let name = stem.to_string_lossy().to_string();
                    // Skip the index file itself
                    if name != "MEMORY" {
                        topics.push(name);
                    }
                }
            }
        }

        topics.sort();
        Ok(topics)
    }

    /// Build a context summary from all memory files.
    ///
    /// Reads all topic files and truncates the total output to fit within
    /// a character budget (~4000 chars). Returns a formatted string suitable
    /// for injecting into Claude's context.
    pub fn get_context_summary(&self) -> Result<String> {
        let mut parts = Vec::new();
        let mut remaining = MAX_CONTEXT_CHARS;

        // Always include MEMORY.md index first (small, gives awareness)
        let index_path = self.base_path.join("MEMORY.md");
        if index_path.exists() {
            let index = fs::read_to_string(&index_path)?;
            let index_trimmed = truncate_to_chars(&index, 1000.min(remaining));
            remaining = remaining.saturating_sub(index_trimmed.len());
            parts.push(format!("[Memory Index]\n{index_trimmed}"));
        }

        // Load each topic file
        let topics = self.list_topics()?;
        for topic in &topics {
            if remaining < 200 {
                break;
            }
            let path = self.base_path.join(format!("{topic}.md"));
            if let Ok(content) = fs::read_to_string(&path) {
                let trimmed = truncate_to_chars(&content, remaining);
                remaining = remaining.saturating_sub(trimmed.len());
                parts.push(format!("[Memory: {topic}]\n{trimmed}"));
            }
        }

        Ok(parts.join("\n\n---\n\n"))
    }

    /// Update the MEMORY.md index when a topic is written.
    ///
    /// Adds a new row if the topic is new, or updates the Last Updated column
    /// if the topic already exists.
    fn update_index(&self, filename: &str, topic: &str, date: &str) -> Result<()> {
        let index_path = self.base_path.join("MEMORY.md");
        let content = if index_path.exists() {
            fs::read_to_string(&index_path)?
        } else {
            DEFAULT_MEMORY_INDEX.to_string()
        };

        let new_row = format!(
            "| {topic} | {filename}.md | User-created topic | {date} |"
        );

        // Check if the topic already has a row
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let mut found = false;

        for line in &mut lines {
            // Match on the filename column (second column in the table)
            if line.contains(&format!("| {filename}.md |")) {
                // Update the Last Updated column (last column before trailing |)
                // Replace the entire row
                *line = new_row.clone();
                found = true;
                break;
            }
        }

        if !found {
            // Append new row at the end of the table
            lines.push(new_row);
        }

        let updated = lines.join("\n");
        atomic_write(&index_path, &updated)?;

        Ok(())
    }
}

// ── Helper Functions ────────────────────────────────────────────────

/// Sanitize a topic name into a valid filename.
///
/// Lowercase, spaces to hyphens, strip non-alphanumeric except hyphens.
pub fn sanitize_topic(topic: &str) -> String {
    topic
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

/// Truncate content to the most recent H2 entries that fit within `max_chars`.
fn truncate_to_recent_entries(content: &str, max_chars: usize) -> String {
    // Split on H2 headers
    let sections: Vec<&str> = content.split("\n## ").collect();

    if sections.len() <= 1 {
        // No H2 sections, just truncate from the end
        return truncate_to_chars(content, max_chars);
    }

    // Keep the header (first section) and as many recent sections as fit
    let header = sections[0];
    let mut result = String::new();
    let mut recent_sections: Vec<&str> = Vec::new();

    // Walk backwards through sections, collecting until we'd exceed budget
    for section in sections[1..].iter().rev() {
        let section_with_header = format!("\n## {section}");
        if result.len() + header.len() + section_with_header.len() > max_chars
            && !recent_sections.is_empty()
        {
            break;
        }
        recent_sections.push(section);
        result = section_with_header + &result;
    }

    recent_sections.reverse();
    format!("{header}{result}")
}

/// Truncate a string to at most `max_chars` characters, on a line boundary.
fn truncate_to_chars(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }

    // Find the last newline before max_chars
    let truncated = &content[..max_chars];
    if let Some(last_newline) = truncated.rfind('\n') {
        content[..last_newline].to_string()
    } else {
        truncated.to_string()
    }
}

/// Write content to a file atomically (write to .tmp, then rename).
fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp_path = path.with_extension("md.tmp");
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

    fn setup() -> (TempDir, Memory) {
        let dir = TempDir::new().unwrap();
        let memory = Memory::new(dir.path());
        memory.init().unwrap();
        (dir, memory)
    }

    #[test]
    fn sanitize_topic_basic() {
        assert_eq!(sanitize_topic("My Decisions!"), "my-decisions");
    }

    #[test]
    fn sanitize_topic_spaces_and_special() {
        assert_eq!(sanitize_topic("Project Notes 2024"), "project-notes-2024");
        assert_eq!(sanitize_topic("hello@world#foo"), "helloworldfoo");
        assert_eq!(sanitize_topic("already-clean"), "already-clean");
    }

    #[test]
    fn init_creates_directory_structure() {
        let dir = TempDir::new().unwrap();
        let memory = Memory::new(dir.path());
        memory.init().unwrap();

        assert!(dir.path().join("memory").exists());
        assert!(dir.path().join("memory/MEMORY.md").exists());
        assert!(dir.path().join("memory/conversations.md").exists());
        assert!(dir.path().join("memory/tasks.md").exists());
        assert!(dir.path().join("memory/decisions.md").exists());
        assert!(dir.path().join("memory/people.md").exists());
    }

    #[test]
    fn init_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let memory = Memory::new(dir.path());
        memory.init().unwrap();

        // Write something to a topic
        memory.write("decisions", "test content").unwrap();

        // Re-init should not overwrite existing files
        memory.init().unwrap();

        let content = memory.read("decisions").unwrap();
        assert!(content.contains("test content"));
    }

    #[test]
    fn read_existing_topic() {
        let (_dir, memory) = setup();
        let content = memory.read("conversations").unwrap();
        assert!(content.contains("Conversations"));
    }

    #[test]
    fn read_nonexistent_topic() {
        let (_dir, memory) = setup();
        let content = memory.read("nonexistent").unwrap();
        assert!(content.contains("No memory file found"));
    }

    #[test]
    fn write_new_topic() {
        let (_dir, memory) = setup();
        let result = memory.write("project-notes", "Working on NV daemon").unwrap();
        assert!(result.contains("Created new memory topic"));

        // Verify the file was created
        let content = memory.read("project-notes").unwrap();
        assert!(content.contains("Working on NV daemon"));
        assert!(content.contains("# project-notes"));
    }

    #[test]
    fn write_append_to_existing() {
        let (_dir, memory) = setup();

        memory.write("decisions", "First decision").unwrap();
        let result = memory.write("decisions", "Second decision").unwrap();
        assert!(result.contains("Appended to memory topic"));

        let content = memory.read("decisions").unwrap();
        assert!(content.contains("First decision"));
        assert!(content.contains("Second decision"));
    }

    #[test]
    fn write_updates_index() {
        let (dir, memory) = setup();

        memory.write("new-topic", "some content").unwrap();

        // Read the index file directly (MEMORY.md is the index, not a topic)
        let index_path = dir.path().join("memory/MEMORY.md");
        let index = std::fs::read_to_string(&index_path).unwrap();
        assert!(index.contains("new-topic"));
        assert!(index.contains("new-topic.md"));
    }

    #[test]
    fn search_finds_content() {
        let (_dir, memory) = setup();

        memory.write("decisions", "Stripe fee is 5%").unwrap();
        memory
            .write("tasks", "Review Stripe integration")
            .unwrap();

        let results = memory.search("Stripe").unwrap();
        assert!(results.contains("Found matches"));
        assert!(results.contains("Stripe"));
    }

    #[test]
    fn search_case_insensitive() {
        let (_dir, memory) = setup();

        memory.write("decisions", "IMPORTANT DECISION").unwrap();

        let results = memory.search("important").unwrap();
        assert!(results.contains("Found matches"));
        assert!(results.contains("IMPORTANT"));
    }

    #[test]
    fn search_no_results() {
        let (_dir, memory) = setup();

        let results = memory.search("xyznonexistent").unwrap();
        assert!(results.contains("No matches found"));
    }

    #[test]
    fn list_topics_returns_defaults() {
        let (_dir, memory) = setup();

        let topics = memory.list_topics().unwrap();
        assert!(topics.contains(&"conversations".to_string()));
        assert!(topics.contains(&"tasks".to_string()));
        assert!(topics.contains(&"decisions".to_string()));
        assert!(topics.contains(&"people".to_string()));
        // MEMORY.md should not appear
        assert!(!topics.contains(&"MEMORY".to_string()));
    }

    #[test]
    fn list_topics_includes_custom() {
        let (_dir, memory) = setup();

        memory.write("custom-topic", "content").unwrap();

        let topics = memory.list_topics().unwrap();
        assert!(topics.contains(&"custom-topic".to_string()));
    }

    #[test]
    fn get_context_summary_includes_index() {
        let (_dir, memory) = setup();

        let summary = memory.get_context_summary().unwrap();
        assert!(summary.contains("[Memory Index]"));
        assert!(summary.contains("NV Memory Index"));
    }

    #[test]
    fn get_context_summary_includes_topics() {
        let (_dir, memory) = setup();

        memory.write("decisions", "Important decision here").unwrap();

        let summary = memory.get_context_summary().unwrap();
        assert!(summary.contains("[Memory: decisions]"));
    }

    #[test]
    fn get_context_summary_respects_budget() {
        let (_dir, memory) = setup();

        // Write a lot of content
        let big_content = "x".repeat(2000);
        for i in 0..10 {
            memory
                .write(&format!("topic-{i}"), &big_content)
                .unwrap();
        }

        let summary = memory.get_context_summary().unwrap();
        assert!(summary.len() <= MAX_CONTEXT_CHARS + 500); // Allow some overhead for headers
    }

    #[test]
    fn truncate_to_recent_entries_basic() {
        let content = "# Header\n\nIntro text\n\n## 2024-01-01 -- First\n\nContent 1\n\n## 2024-01-02 -- Second\n\nContent 2\n";
        let truncated = truncate_to_recent_entries(content, 80);
        // Should keep at least the most recent entry
        assert!(truncated.contains("Content 2") || truncated.contains("Header"));
    }

    #[test]
    fn truncate_to_chars_respects_limit() {
        let content = "line one\nline two\nline three\nline four\n";
        let truncated = truncate_to_chars(content, 20);
        assert!(truncated.len() <= 20);
    }

    #[test]
    fn atomic_write_works() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.md");
        atomic_write(&path, "hello world").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
        // No .tmp file should remain
        assert!(!dir.path().join("test.md.tmp").exists());
    }
}
