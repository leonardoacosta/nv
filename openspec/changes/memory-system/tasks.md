# memory-system — Tasks

## Directory Initialization
- [ ] Create `init_memory_dir()` in `nv-core/src/memory.rs` — creates `~/.nv/memory/` and `~/.nv/state/` if missing
- [ ] Create default MEMORY.md index file with table header
- [ ] Create default topic files: conversations.md, tasks.md, decisions.md, people.md
- [ ] Create default state files: last-digest.json, pending-actions.json, channel-state.json (empty JSON `{}`)
- [ ] Call `init_memory_dir()` during daemon startup before agent loop starts
- [ ] Idempotent: skip creation for files/dirs that already exist

## MemorySystem Struct
- [ ] Define `MemorySystem` struct with `memory_dir: PathBuf` and `state_dir: PathBuf`
- [ ] Implement `MemorySystem::new(base_dir)` constructor
- [ ] Implement `sanitize_topic()` helper: lowercase, spaces to hyphens, strip non-alphanumeric except hyphens
- [ ] Unit test: sanitize_topic("My Decisions!") == "my-decisions"

## MEMORY.md Index
- [ ] Define index format: markdown table with Topic, File, Description, Last Updated columns
- [ ] Implement `update_index()` — add or update a row in MEMORY.md when a topic is written
- [ ] Parse existing index table to avoid duplicates
- [ ] Unit test: write new topic, verify index row added; write again, verify row updated not duplicated

## read_memory Tool
- [ ] Implement `read_memory(topic)` — read `~/.nv/memory/{topic}.md`, return content
- [ ] Return friendly message if topic file does not exist
- [ ] Implement `truncate_to_recent_entries()` — if file exceeds MAX_MEMORY_READ_CHARS (20000), return only recent H2 sections
- [ ] Wire into agent loop `execute_tool()` dispatch, replacing spec-4 stub

## search_memory Tool
- [ ] Implement `search_memory(query)` — case-insensitive search across all .md files in memory dir
- [ ] Collect matching lines with 2 lines of surrounding context (before and after)
- [ ] Format results as `[filename:line_num]\n{context}` blocks separated by `---`
- [ ] Limit to MAX_SEARCH_RESULTS (10) matches to prevent context explosion
- [ ] Show truncation notice if results were limited
- [ ] Return friendly message if no matches found
- [ ] Wire into agent loop `execute_tool()` dispatch, replacing spec-4 stub

## write_memory Tool
- [ ] Implement `write_memory(topic, content)` — append entry to topic file
- [ ] New topic: create file with YAML frontmatter (topic, created, updated, entries) + H1 header + first entry as H2
- [ ] Existing topic: append H2 entry with timestamp
- [ ] Update frontmatter: bump `updated` timestamp and `entries` count
- [ ] Implement `update_frontmatter()` — parse and rewrite YAML frontmatter in-place
- [ ] Call `update_index()` to keep MEMORY.md current
- [ ] Wire into agent loop `execute_tool()` dispatch, replacing spec-4 stub
- [ ] After write, check entry count and trigger summarization if over threshold

## State File Operations
- [ ] Implement `save_pending_action(action)` — read pending-actions.json, append action, write back
- [ ] Implement `load_pending_actions()` — deserialize Vec<PendingAction> from JSON
- [ ] Implement `update_pending_action(id, status)` — find by ID, update status, write back
- [ ] Implement `remove_pending_action(id)` — remove completed/cancelled actions
- [ ] Implement `save_channel_state(channel, state)` — update channel entry in channel-state.json
- [ ] Implement `load_channel_state(channel)` — read channel's cursor/offset
- [ ] All state operations use file locking (or atomic write via temp file + rename) to prevent corruption

## Context Injection
- [ ] Implement `load_relevant_context(triggers)` — returns formatted string for Claude's context
- [ ] Always include MEMORY.md index (truncated to 1000 chars) as first context block
- [ ] Extract text from message triggers for keyword matching
- [ ] Implement `find_relevant_topics(text)` — simple keyword match against topic filenames and first 500 chars of content
- [ ] Load relevant topic files up to MEMORY_TOKEN_BUDGET (2000 tokens / ~8000 chars)
- [ ] Format each loaded topic as `[Memory: {topic}]\n{content}` separated by `---`
- [ ] Stop loading topics when remaining char budget drops below 500
- [ ] Wire into agent loop: call before building Claude API request, inject as first part of user message

## Summarization
- [ ] Define SUMMARIZE_THRESHOLD constant (default 20 entries)
- [ ] Implement `count_entries(content)` — count H2 sections in a memory file
- [ ] Implement `maybe_summarize(topic)` — check entry count, trigger if over threshold
- [ ] Build summarization prompt: "Summarize, preserve key facts and decisions, keep last 5 entries verbatim"
- [ ] Call Claude API with summarization prompt (no tools, just text completion)
- [ ] Replace file content with summarized output, preserve frontmatter
- [ ] Update frontmatter: reset entries count, add `summarized_at` timestamp
- [ ] Run summarization asynchronously (tokio::spawn) — do not block main agent response
- [ ] Handle summarization failure gracefully: log error, keep original file intact
