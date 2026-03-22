# memory-system

## Summary

Markdown-native memory system at `~/.nv/memory/` with a MEMORY.md index file and topic-based markdown files. Provides agent tools (`read_memory`, `search_memory`, `write_memory`) that replace the spec-4 stubs, a state directory for daemon persistence, and context injection that loads relevant memory into each Claude API call.

## Motivation

Without persistent memory, NV forgets everything between Claude API calls. Memory enables NV to store decisions, preferences, people context, and task state across sessions. The markdown-native approach is grep-searchable, human-readable, and requires no database — matching the weekend MVP constraint.

## Design

### Directory Structure

```
~/.nv/
├── nv.toml                    # Config (spec-2)
├── system-prompt.md           # Optional system prompt override (spec-4)
├── memory/
│   ├── MEMORY.md              # Index: lists all topic files with descriptions
│   ├── conversations.md       # Conversation summaries
│   ├── tasks.md               # Active task context
│   ├── decisions.md           # Decisions made and rationale
│   ├── people.md              # Who's who, preferences, roles
│   └── {user-created}.md      # Additional topics created by the agent
└── state/
    ├── last-digest.json       # Last digest metadata
    ├── pending-actions.json   # Actions awaiting confirmation
    └── channel-state.json     # Per-channel cursor/offset
```

### Initialization

On first daemon start, create the directory structure if it does not exist:

```rust
pub async fn init_memory_dir(base: &Path) -> Result<()> {
    let memory_dir = base.join("memory");
    let state_dir = base.join("state");

    fs::create_dir_all(&memory_dir).await?;
    fs::create_dir_all(&state_dir).await?;

    let index_path = memory_dir.join("MEMORY.md");
    if !index_path.exists() {
        fs::write(&index_path, DEFAULT_MEMORY_INDEX).await?;
    }

    // Create default topic files if they don't exist
    for (name, header) in &[
        ("conversations.md", "# Conversations\n\nSummaries of past conversations.\n"),
        ("tasks.md", "# Tasks\n\nActive task context and status.\n"),
        ("decisions.md", "# Decisions\n\nDecisions made and their rationale.\n"),
        ("people.md", "# People\n\nPeople, roles, and preferences.\n"),
    ] {
        let path = memory_dir.join(name);
        if !path.exists() {
            fs::write(&path, header).await?;
        }
    }

    // Initialize state files with empty JSON
    for name in &["last-digest.json", "pending-actions.json", "channel-state.json"] {
        let path = state_dir.join(name);
        if !path.exists() {
            fs::write(&path, "{}").await?;
        }
    }

    Ok(())
}
```

### Memory File Format

Each memory file is a markdown document with a YAML frontmatter header:

```markdown
---
topic: decisions
created: 2026-03-21T10:00:00Z
updated: 2026-03-21T14:30:00Z
entries: 5
---

# Decisions

## 2026-03-21 14:30 — Stripe fee structure

The Stripe fee is 5% per transaction. Decided to absorb the fee rather than pass to customers.

## 2026-03-21 10:00 — Default Jira project

Using OO (Otaku Odyssey) as the default Jira project for issue creation.
```

**Format rules:**
- Frontmatter tracks metadata for the index and summarization
- Each entry is an H2 section with ISO date and short title
- Newest entries appended at the bottom
- Content is free-form markdown within each entry

### MEMORY.md Index

The index file provides a table of contents for the agent to understand what memory is available:

```markdown
# NV Memory Index

| Topic | File | Description | Last Updated |
|-------|------|-------------|-------------|
| conversations | conversations.md | Conversation summaries | 2026-03-21 |
| tasks | tasks.md | Active task context | 2026-03-21 |
| decisions | decisions.md | Decisions and rationale | 2026-03-21 |
| people | people.md | People, roles, preferences | 2026-03-21 |
```

Updated automatically when `write_memory` creates a new topic or appends to an existing one.

### Tool Implementations

#### read_memory(topic: string) -> string

Read a specific memory topic file. Returns the full content of the file.

```rust
pub async fn read_memory(&self, topic: &str) -> Result<String> {
    let filename = sanitize_topic(topic);
    let path = self.memory_dir.join(format!("{filename}.md"));

    if !path.exists() {
        return Ok(format!("No memory file found for topic: {topic}"));
    }

    let content = fs::read_to_string(&path).await?;

    // If file is very large, return only the last N entries
    if content.len() > MAX_MEMORY_READ_CHARS {
        let truncated = truncate_to_recent_entries(&content, MAX_MEMORY_READ_CHARS);
        return Ok(format!("[Showing recent entries, {topic} has more history]\n\n{truncated}"));
    }

    Ok(content)
}
```

`sanitize_topic()` converts a topic string to a valid filename: lowercase, spaces to hyphens, strip non-alphanumeric chars except hyphens.

#### search_memory(query: string) -> string

Grep across all memory files. Returns matching lines with surrounding context.

```rust
pub async fn search_memory(&self, query: &str) -> Result<String> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();

    let mut entries = fs::read_dir(&self.memory_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "md") {
            let content = fs::read_to_string(&path).await?;
            let filename = path.file_stem().unwrap().to_string_lossy().to_string();

            let lines: Vec<&str> = content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    // Collect surrounding context (2 lines before, 2 after)
                    let start = i.saturating_sub(2);
                    let end = (i + 3).min(lines.len());
                    let context: Vec<&str> = lines[start..end].to_vec();

                    results.push(format!(
                        "[{filename}:{line_num}]\n{context}",
                        line_num = i + 1,
                        context = context.join("\n"),
                    ));
                }
            }
        }
    }

    if results.is_empty() {
        return Ok(format!("No matches found for: {query}"));
    }

    // Limit total results to prevent context explosion
    let truncated = results.len() > MAX_SEARCH_RESULTS;
    let results: Vec<_> = results.into_iter().take(MAX_SEARCH_RESULTS).collect();

    let mut output = format!("Found matches for \"{query}\":\n\n");
    output.push_str(&results.join("\n---\n"));
    if truncated {
        output.push_str("\n\n[Results truncated. Refine your query for more specific results.]");
    }

    Ok(output)
}
```

**Constants:**
- `MAX_SEARCH_RESULTS = 10` — prevent flooding Claude's context
- Context window: 2 lines before and after each match

#### write_memory(topic: string, content: string) -> string

Append content to a topic file. Creates the file and updates MEMORY.md if the topic is new.

```rust
pub async fn write_memory(&self, topic: &str, content: &str) -> Result<String> {
    let filename = sanitize_topic(topic);
    let path = self.memory_dir.join(format!("{filename}.md"));
    let now = Utc::now();
    let is_new = !path.exists();

    if is_new {
        // Create new topic file with frontmatter
        let initial = format!(
            "---\ntopic: {filename}\ncreated: {now}\nupdated: {now}\nentries: 1\n---\n\n# {topic}\n\n## {date} — Entry\n\n{content}\n",
            date = now.format("%Y-%m-%d %H:%M"),
        );
        fs::write(&path, initial).await?;

        // Update MEMORY.md index
        self.update_index(&filename, topic, &now).await?;

        Ok(format!("Created new memory topic: {topic}"))
    } else {
        // Append entry to existing file
        let entry = format!(
            "\n## {date} — Entry\n\n{content}\n",
            date = now.format("%Y-%m-%d %H:%M"),
        );
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await?;
        file.write_all(entry.as_bytes()).await?;

        // Update frontmatter (updated timestamp, increment entries)
        self.update_frontmatter(&path, &now).await?;

        Ok(format!("Appended to memory topic: {topic}"))
    }
}
```

### State Files

State files use JSON and are managed by the daemon, not the memory tool system. They persist daemon state across restarts.

**pending-actions.json:**
```json
{
  "actions": [
    {
      "id": "uuid-here",
      "description": "Create P1 bug: Checkout crash",
      "jira_payload": { "project": "OO", "type": "Bug", "priority": "P1", "summary": "..." },
      "status": "awaiting_confirmation",
      "created_at": "2026-03-21T14:30:00Z"
    }
  ]
}
```

**last-digest.json:**
```json
{
  "timestamp": "2026-03-21T08:00:00Z",
  "content_hash": "sha256:...",
  "actions_suggested": 3,
  "actions_taken": 1
}
```

**channel-state.json:**
```json
{
  "telegram": {
    "last_update_id": 12345678,
    "last_poll_at": "2026-03-21T14:30:00Z"
  }
}
```

### Context Injection Strategy

Before each Claude API call, the agent loop injects relevant memory. The injection is token-budgeted to avoid consuming too much of the context window.

```rust
const MEMORY_TOKEN_BUDGET: usize = 2000; // ~8000 chars at 4 chars/token
const CHARS_PER_TOKEN: usize = 4;

pub async fn load_relevant_context(&self, triggers: &[Trigger]) -> Result<String> {
    let mut context_parts = Vec::new();
    let mut remaining_chars = MEMORY_TOKEN_BUDGET * CHARS_PER_TOKEN;

    // 1. Always include MEMORY.md index (small, gives Claude awareness of what's available)
    let index = fs::read_to_string(self.memory_dir.join("MEMORY.md")).await?;
    let index_trimmed = truncate_to_chars(&index, 1000);
    remaining_chars -= index_trimmed.len();
    context_parts.push(format!("[Memory Index]\n{index_trimmed}"));

    // 2. Load topic files relevant to the trigger content
    let trigger_text = triggers.iter()
        .filter_map(|t| match t {
            Trigger::Message(msg) => Some(msg.content.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Simple keyword matching against topic names and content
    let relevant_topics = self.find_relevant_topics(&trigger_text).await?;

    for topic in relevant_topics {
        if remaining_chars < 500 {
            break; // Stop injecting if budget is nearly exhausted
        }
        let content = self.read_memory(&topic).await?;
        let trimmed = truncate_to_chars(&content, remaining_chars);
        remaining_chars -= trimmed.len();
        context_parts.push(format!("[Memory: {topic}]\n{trimmed}"));
    }

    Ok(context_parts.join("\n\n---\n\n"))
}
```

**Topic relevance** — for the weekend MVP, use simple keyword matching: check if any word from the trigger content appears in a topic filename or in the first 500 chars of the topic file. Future improvement: embeddings-based retrieval.

### Summarization

After N entries in a memory file (configurable, default 20), the agent loop triggers a summarization pass.

**Trigger:** Checked after each `write_memory` call. If entry count exceeds threshold, a summarization trigger is queued.

**Strategy:**
1. Read the full topic file
2. Send to Claude with a summarization prompt: "Summarize this memory file, preserving key facts, decisions, and context. Remove redundant entries. Keep recent entries (last 5) verbatim."
3. Replace file content with the summarized version
4. Update frontmatter with new entry count and `summarized_at` timestamp

```rust
async fn maybe_summarize(&self, topic: &str) -> Result<()> {
    let path = self.memory_dir.join(format!("{}.md", sanitize_topic(topic)));
    let content = fs::read_to_string(&path).await?;

    let entry_count = count_entries(&content);
    if entry_count < SUMMARIZE_THRESHOLD {
        return Ok(());
    }

    tracing::info!(topic, entry_count, "triggering memory summarization");

    let summary = self.client.send_messages(
        SUMMARIZE_SYSTEM_PROMPT,
        &[Message::user(format!(
            "Summarize this memory file. Preserve key facts and decisions. \
             Keep the last 5 entries verbatim. Remove redundancy.\n\n{content}"
        ))],
        &[], // No tools needed for summarization
    ).await?;

    let summary_text = extract_text(&summary.content);
    fs::write(&path, summary_text).await?;

    Ok(())
}
```

**Summarization is not blocking** — it happens asynchronously after the main agent response is sent. If it fails, the original file remains intact.

### MemorySystem Struct

```rust
pub struct MemorySystem {
    memory_dir: PathBuf,
    state_dir: PathBuf,
}

impl MemorySystem {
    pub fn new(base_dir: &Path) -> Self {
        Self {
            memory_dir: base_dir.join("memory"),
            state_dir: base_dir.join("state"),
        }
    }

    pub async fn read_memory(&self, topic: &str) -> Result<String>;
    pub async fn search_memory(&self, query: &str) -> Result<String>;
    pub async fn write_memory(&self, topic: &str, content: &str) -> Result<String>;
    pub async fn load_relevant_context(&self, triggers: &[Trigger]) -> Result<String>;
    pub async fn save_pending_action(&self, action: &PendingAction) -> Result<()>;
    pub async fn load_pending_actions(&self) -> Result<Vec<PendingAction>>;
    pub async fn update_pending_action(&self, id: &str, status: PendingStatus) -> Result<()>;
    pub async fn save_channel_state(&self, channel: &str, state: &ChannelState) -> Result<()>;
    pub async fn load_channel_state(&self, channel: &str) -> Result<Option<ChannelState>>;
}
```

## Verification

- Ask NV "remember that the Stripe fee is 5%" -- NV calls `write_memory("decisions", ...)`, confirms stored
- Ask NV "what's the Stripe fee?" -- NV calls `search_memory("Stripe fee")`, returns correct answer
- Ask NV "what do you remember?" -- NV calls `read_memory` on index, lists available topics
- Create 25+ entries in a topic -- summarization triggers automatically, file is compacted
- Memory context appears in Claude API requests (verify via tracing logs)
- State files persist across daemon restart -- pending actions survive, channel cursors maintained
