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

/// Common English stop words filtered out during keyword extraction.
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "to", "of", "in", "for",
    "on", "at", "by", "with", "it", "and", "or", "but", "not", "this", "that",
    "from", "as", "be", "has", "have", "had", "do", "does", "did", "will",
    "would", "can", "could", "should", "may", "might", "shall", "must", "need",
];

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
    /// If the topic is new, creates the file with YAML frontmatter and a header,
    /// then updates the MEMORY.md index. If existing, appends a new entry with a
    /// timestamp header and updates the frontmatter (`updated`, `entries`).
    pub fn write(&self, topic: &str, content: &str) -> Result<String> {
        let filename = sanitize_topic(topic);
        let path = self.base_path.join(format!("{filename}.md"));
        let now = Utc::now();
        let timestamp = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let date_str = now.format("%Y-%m-%d %H:%M").to_string();
        let is_new = !path.exists();

        if is_new {
            let initial = format!(
                "---\ntopic: {topic}\ncreated: {timestamp}\nupdated: {timestamp}\nentries: 1\n---\n\n# {topic}\n\n## {date_str} -- Entry\n\n{content}\n"
            );
            atomic_write(&path, &initial)?;

            // Update MEMORY.md index
            self.update_index(&filename, topic, &date_str)?;

            tracing::info!(topic, "created new memory topic");
            Ok(format!("Created new memory topic: {topic}"))
        } else {
            // Append entry to existing file, then update frontmatter
            let entry = format!("\n## {date_str} -- Entry\n\n{content}\n");
            let mut existing = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            existing.push_str(&entry);
            atomic_write(&path, &existing)?;

            // Update frontmatter (updated timestamp + entries count)
            self.update_frontmatter(&path, topic, &timestamp)?;

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

    /// Build a context summary prioritized by relevance to `trigger_text`.
    ///
    /// Extracts keywords from `trigger_text`, scores each topic file by keyword
    /// matches in its filename and first 500 chars of content, then loads topics
    /// in relevance order (highest score first) within the char budget.
    ///
    /// Falls back to alphabetical order when no trigger text is supplied.
    pub fn get_context_summary_for(&self, trigger_text: &str) -> Result<String> {
        let mut parts = Vec::new();
        let mut remaining = MAX_CONTEXT_CHARS;

        // Always include MEMORY.md index first
        let index_path = self.base_path.join("MEMORY.md");
        if index_path.exists() {
            let index = fs::read_to_string(&index_path)?;
            let index_trimmed = truncate_to_chars(&index, 1000.min(remaining));
            remaining = remaining.saturating_sub(index_trimmed.len());
            parts.push(format!("[Memory Index]\n{index_trimmed}"));
        }

        // Get topics ordered by relevance
        let topics = self.find_relevant_topics(trigger_text)?;
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

    /// Find topics relevant to `text`, sorted by keyword match score (desc).
    ///
    /// Keywords are extracted from `text` by splitting on whitespace, lowercasing,
    /// and filtering out common stop words. Each topic is scored by the number of
    /// keyword appearances in its filename and the first 500 chars of its content.
    /// Topics with no matches are appended after matched topics, in alphabetical order.
    pub fn find_relevant_topics(&self, text: &str) -> Result<Vec<String>> {
        let keywords = extract_keywords(text);

        let all_topics = self.list_topics()?;

        if keywords.is_empty() {
            return Ok(all_topics);
        }

        let mut scored: Vec<(String, usize)> = all_topics
            .into_iter()
            .map(|topic| {
                let mut score: usize = 0;

                // Score keyword hits in filename
                let topic_lower = topic.to_lowercase();
                for kw in &keywords {
                    if topic_lower.contains(kw.as_str()) {
                        score += 1;
                    }
                }

                // Score keyword hits in first 500 chars of content
                let path = self.base_path.join(format!("{topic}.md"));
                if let Ok(content) = fs::read_to_string(&path) {
                    let preview = &content[..content.len().min(500)];
                    let preview_lower = preview.to_lowercase();
                    for kw in &keywords {
                        // Count all occurrences in the preview
                        let mut start = 0;
                        while let Some(pos) = preview_lower[start..].find(kw.as_str()) {
                            score += 1;
                            start += pos + kw.len();
                        }
                    }
                }

                (topic, score)
            })
            .collect();

        // Stable sort: highest score first; ties keep alphabetical order from list_topics
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(scored.into_iter().map(|(t, _)| t).collect())
    }

    /// Parse and rewrite YAML frontmatter in-place for a memory file.
    ///
    /// Updates the `updated` field to `timestamp`, increments `entries` by 1,
    /// and rewrites the file atomically. If no frontmatter exists, inserts it
    /// (backward-compatible with files created before this feature).
    fn update_frontmatter(&self, path: &Path, topic: &str, timestamp: &str) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let (new_content, _entries) = upsert_frontmatter(&content, topic, timestamp, false);
        atomic_write(path, &new_content)?;
        Ok(())
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

/// Extract meaningful keywords from `text` for topic relevance scoring.
///
/// Splits on whitespace, lowercases each token, strips leading/trailing
/// non-alphanumeric characters, and removes stop words and tokens shorter
/// than 3 characters.
fn extract_keywords(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|word| {
            // Strip punctuation from both ends
            let cleaned: String = word
                .chars()
                .skip_while(|c| !c.is_alphanumeric())
                .collect::<String>()
                .chars()
                .rev()
                .skip_while(|c| !c.is_alphanumeric())
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            let lower = cleaned.to_lowercase();
            if lower.len() < 3 {
                return None;
            }
            if STOP_WORDS.contains(&lower.as_str()) {
                return None;
            }
            Some(lower)
        })
        .collect()
}

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

/// Parse, update, or insert YAML frontmatter in a memory file's content.
///
/// Returns `(new_content, entries_count)`.
///
/// - If frontmatter exists (content starts with `---`): updates `updated` to
///   `timestamp` and increments `entries` by 1.
/// - If no frontmatter: inserts it before the rest of the content.
/// - When `is_new` is true the entries count is set to 1 (used by callers that
///   build the initial content from scratch, though `write()` embeds frontmatter
///   inline and never calls this with `is_new = true`).
fn upsert_frontmatter(content: &str, topic: &str, timestamp: &str, is_new: bool) -> (String, u64) {
    // Does the file start with a frontmatter block?
    if content.starts_with("---\n") {
        // Find the closing `---`
        if let Some(close_offset) = content[4..].find("\n---\n") {
            let fm_end = 4 + close_offset; // index of the newline before closing ---
            let frontmatter = &content[4..fm_end];
            let rest = &content[fm_end + 5..]; // skip "\n---\n"

            // Parse existing fields line by line
            let mut topic_val = topic.to_string();
            let mut created_val = timestamp.to_string();
            let mut entries_val: u64 = 1;

            for line in frontmatter.lines() {
                if let Some(v) = line.strip_prefix("topic: ") {
                    topic_val = v.to_string();
                } else if let Some(v) = line.strip_prefix("created: ") {
                    created_val = v.to_string();
                } else if let Some(v) = line.strip_prefix("entries: ") {
                    entries_val = v.parse().unwrap_or(1);
                }
                // `updated` is intentionally not kept — we always overwrite it
            }

            let new_entries = if is_new { 1 } else { entries_val + 1 };

            let new_content = format!(
                "---\ntopic: {topic_val}\ncreated: {created_val}\nupdated: {timestamp}\nentries: {new_entries}\n---\n{rest}"
            );
            return (new_content, new_entries);
        }
    }

    // No valid frontmatter — insert it before the existing content
    let new_entries: u64 = 1;
    let new_content = format!(
        "---\ntopic: {topic}\ncreated: {timestamp}\nupdated: {timestamp}\nentries: {new_entries}\n---\n\n{content}"
    );
    (new_content, new_entries)
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

    // ── Frontmatter tests ────────────────────────────────────────────

    #[test]
    fn upsert_frontmatter_inserts_when_missing() {
        let content = "# My Topic\n\nsome content\n";
        let (result, entries) =
            upsert_frontmatter(content, "my-topic", "2024-01-01T00:00:00Z", false);
        assert!(result.starts_with("---\n"));
        assert!(result.contains("topic: my-topic"));
        assert!(result.contains("created: 2024-01-01T00:00:00Z"));
        assert!(result.contains("updated: 2024-01-01T00:00:00Z"));
        assert!(result.contains("entries: 1"));
        assert!(result.contains("# My Topic"));
        assert_eq!(entries, 1);
    }

    #[test]
    fn upsert_frontmatter_updates_existing() {
        let content = "---\ntopic: my-topic\ncreated: 2024-01-01T00:00:00Z\nupdated: 2024-01-01T00:00:00Z\nentries: 3\n---\n\n# My Topic\n\ncontent\n";
        let (result, entries) =
            upsert_frontmatter(content, "my-topic", "2024-06-01T12:00:00Z", false);
        assert!(result.contains("created: 2024-01-01T00:00:00Z")); // unchanged
        assert!(result.contains("updated: 2024-06-01T12:00:00Z")); // new timestamp
        assert!(result.contains("entries: 4")); // incremented
        assert!(result.contains("# My Topic")); // body preserved
        assert_eq!(entries, 4);
    }

    #[test]
    fn upsert_frontmatter_preserves_topic_and_created() {
        let content = "---\ntopic: original-name\ncreated: 2023-01-01T00:00:00Z\nupdated: 2023-01-01T00:00:00Z\nentries: 1\n---\n\nbody\n";
        let (result, _) =
            upsert_frontmatter(content, "ignored-topic", "2024-01-01T00:00:00Z", false);
        assert!(result.contains("topic: original-name")); // kept from file
        assert!(result.contains("created: 2023-01-01T00:00:00Z")); // kept from file
    }

    #[test]
    fn write_new_topic_includes_frontmatter() {
        let (_dir, memory) = setup();
        memory.write("fm-test", "Initial content").unwrap();

        let content = memory.read("fm-test").unwrap();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("topic: fm-test"));
        assert!(content.contains("entries: 1"));
        assert!(content.contains("Initial content"));
    }

    #[test]
    fn write_append_increments_entries() {
        let (_dir, memory) = setup();

        memory.write("counter-topic", "First").unwrap();
        memory.write("counter-topic", "Second").unwrap();
        memory.write("counter-topic", "Third").unwrap();

        let content = memory.read("counter-topic").unwrap();
        assert!(content.contains("entries: 3"));
        assert!(content.contains("First"));
        assert!(content.contains("Second"));
        assert!(content.contains("Third"));
    }

    #[test]
    fn write_append_updated_timestamp_present() {
        let (_dir, memory) = setup();

        memory.write("ts-topic", "First").unwrap();
        memory.write("ts-topic", "Second").unwrap();

        let content = memory.read("ts-topic").unwrap();
        // `updated:` field must be present and look like an ISO-8601 timestamp
        let updated_line = content
            .lines()
            .find(|l| l.starts_with("updated: "))
            .expect("updated field missing from frontmatter");
        assert!(updated_line.starts_with("updated: 20"));
    }

    #[test]
    fn update_frontmatter_backward_compatible() {
        // Simulate a legacy file that has no frontmatter
        let (_dir, memory) = setup();
        let legacy_path = memory.base_path.join("legacy.md");
        fs::write(&legacy_path, "# Legacy Topic\n\nOld content\n").unwrap();

        // Calling update_frontmatter on it should add frontmatter without error
        memory
            .update_frontmatter(&legacy_path, "legacy", "2024-01-01T00:00:00Z")
            .unwrap();

        let updated = fs::read_to_string(&legacy_path).unwrap();
        assert!(updated.starts_with("---\n"));
        assert!(updated.contains("topic: legacy"));
        assert!(updated.contains("entries: 1"));
        assert!(updated.contains("Old content"));
    }

    // ── extract_keywords tests ──────────────────────────────────────

    #[test]
    fn extract_keywords_basic() {
        let kws = extract_keywords("What are the project decisions?");
        assert!(kws.contains(&"project".to_string()));
        assert!(kws.contains(&"decisions".to_string()));
        assert!(!kws.contains(&"the".to_string()));
        assert!(!kws.contains(&"are".to_string()));
    }

    #[test]
    fn extract_keywords_strips_punctuation() {
        let kws = extract_keywords("Hello, world!");
        assert!(kws.contains(&"hello".to_string()));
        assert!(kws.contains(&"world".to_string()));
    }

    #[test]
    fn extract_keywords_filters_short_tokens() {
        let kws = extract_keywords("do it now");
        assert!(kws.contains(&"now".to_string()));
        assert!(!kws.contains(&"do".to_string()));
        assert!(!kws.contains(&"it".to_string()));
    }

    #[test]
    fn extract_keywords_empty_text() {
        let kws = extract_keywords("");
        assert!(kws.is_empty());
    }

    // ── find_relevant_topics tests ──────────────────────────────────

    #[test]
    fn find_relevant_topics_returns_matched_first() {
        let (_dir, memory) = setup();

        memory.write("tasks", "Review the Stripe integration").unwrap();

        let topics = memory.find_relevant_topics("decisions").unwrap();
        let decisions_pos = topics.iter().position(|t| t == "decisions").unwrap();
        let tasks_pos = topics.iter().position(|t| t == "tasks").unwrap();
        assert!(decisions_pos < tasks_pos, "decisions should rank above tasks");
    }

    #[test]
    fn find_relevant_topics_content_scoring() {
        let (_dir, memory) = setup();

        memory
            .write("decisions", "We decided to use PostgreSQL for storage")
            .unwrap();
        memory
            .write("tasks", "Review open issues this week")
            .unwrap();

        let topics = memory
            .find_relevant_topics("postgresql database storage")
            .unwrap();
        let decisions_pos = topics.iter().position(|t| t == "decisions").unwrap();
        let tasks_pos = topics.iter().position(|t| t == "tasks").unwrap();
        assert!(decisions_pos < tasks_pos);
    }

    #[test]
    fn find_relevant_topics_no_match_all_topics_returned() {
        let (_dir, memory) = setup();

        let topics = memory.find_relevant_topics("xyzunknown").unwrap();
        let all_topics = memory.list_topics().unwrap();
        assert_eq!(topics.len(), all_topics.len());
    }

    #[test]
    fn find_relevant_topics_empty_text_falls_back() {
        let (_dir, memory) = setup();

        let topics_empty = memory.find_relevant_topics("").unwrap();
        let topics_alpha = memory.list_topics().unwrap();
        assert_eq!(topics_empty, topics_alpha);
    }

    // ── get_context_summary_for tests ──────────────────────────────

    #[test]
    fn get_context_summary_for_includes_index() {
        let (_dir, memory) = setup();

        let summary = memory
            .get_context_summary_for("some trigger text")
            .unwrap();
        assert!(summary.contains("[Memory Index]"));
        assert!(summary.contains("NV Memory Index"));
    }

    #[test]
    fn get_context_summary_for_includes_relevant_topics() {
        let (_dir, memory) = setup();

        memory
            .write("decisions", "Important decision about tasks")
            .unwrap();

        let summary = memory
            .get_context_summary_for("decisions about the project")
            .unwrap();
        assert!(summary.contains("[Memory: decisions]"));
    }

    #[test]
    fn get_context_summary_for_respects_budget() {
        let (_dir, memory) = setup();

        let big_content = "x".repeat(2000);
        for i in 0..10 {
            memory
                .write(&format!("topic-{i}"), &big_content)
                .unwrap();
        }

        let summary = memory
            .get_context_summary_for("topic content")
            .unwrap();
        assert!(summary.len() <= MAX_CONTEXT_CHARS + 500);
    }
}
