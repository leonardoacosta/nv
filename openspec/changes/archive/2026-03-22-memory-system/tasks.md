# memory-system — Tasks

## Directory Initialization
- [x] Create `init_memory_dir()` in `nv-daemon/src/memory.rs` — creates `~/.nv/memory/` if missing
- [x] Create default MEMORY.md index file with table header
- [x] Create default topic files: conversations.md, tasks.md, decisions.md, people.md
- [x] Create default state files: last-digest.json, pending-actions.json, channel-state.json (empty JSON `{}`)
- [x] Call `init_memory_dir()` during daemon startup before agent loop starts
- [x] Idempotent: skip creation for files/dirs that already exist

## MemorySystem Struct
- [x] Define `Memory` struct with `base_path: PathBuf`
- [x] Implement `Memory::new(base_dir)` constructor
- [x] Implement `sanitize_topic()` helper: lowercase, spaces to hyphens, strip non-alphanumeric except hyphens
- [x] Unit test: sanitize_topic("My Decisions!") == "my-decisions"

## MEMORY.md Index
- [x] Define index format: markdown table with Topic, File, Description, Last Updated columns
- [x] Implement `update_index()` — add or update a row in MEMORY.md when a topic is written
- [x] Parse existing index table to avoid duplicates
- [x] Unit test: write new topic, verify index row added; write again, verify row updated not duplicated

## read_memory Tool
- [x] Implement `read_memory(topic)` — read `~/.nv/memory/{topic}.md`, return content
- [x] Return friendly message if topic file does not exist
- [x] Implement `truncate_to_recent_entries()` — if file exceeds MAX_MEMORY_READ_CHARS (20000), return only recent H2 sections
- [x] Wire into agent loop `execute_tool()` dispatch, replacing spec-4 stub

## search_memory Tool
- [x] Implement `search_memory(query)` — case-insensitive search across all .md files in memory dir
- [x] Collect matching lines with 2 lines of surrounding context (before and after)
- [x] Format results as `[filename:line_num]\n{context}` blocks separated by `---`
- [x] Limit to MAX_SEARCH_RESULTS (10) matches to prevent context explosion
- [x] Show truncation notice if results were limited
- [x] Return friendly message if no matches found
- [x] Wire into agent loop `execute_tool()` dispatch, replacing spec-4 stub

## write_memory Tool
- [x] Implement `write_memory(topic, content)` — append entry to topic file
- [x] New topic: create file with H1 header + first entry as H2
- [x] Existing topic: append H2 entry with timestamp
- [x] Update frontmatter: bump `updated` timestamp and `entries` count
- [x] Implement `update_frontmatter()` — parse and rewrite YAML frontmatter in-place
- [x] Call `update_index()` to keep MEMORY.md current
- [x] Wire into agent loop `execute_tool()` dispatch, replacing spec-4 stub
- [x] After write, check entry count and trigger summarization if over threshold

## State File Operations
- [x] Implement `save_pending_action(action)` — read pending-actions.json, append action, write back
- [x] Implement `load_pending_actions()` — deserialize Vec<PendingAction> from JSON
- [x] Implement `update_pending_action(id, status)` — find by ID, update status, write back
- [x] Implement `remove_pending_action(id)` — remove completed/cancelled actions
- [x] Implement `save_channel_state(channel, state)` — update channel entry in channel-state.json
- [x] Implement `load_channel_state(channel)` — read channel's cursor/offset
- [x] All state operations use atomic write via temp file + rename to prevent corruption

## Context Injection
- [x] Implement `get_context_summary()` — returns formatted string for Claude's context
- [x] Always include MEMORY.md index (truncated to 1000 chars) as first context block
- [x] Extract text from message triggers for keyword matching
- [x] Implement `find_relevant_topics(text)` — simple keyword match against topic filenames and first 500 chars of content
- [x] Load topic files up to MAX_CONTEXT_CHARS (4000 chars) budget
- [x] Format each loaded topic as `[Memory: {topic}]\n{content}` separated by `---`
- [x] Stop loading topics when remaining char budget drops below 200
- [x] Wire into agent loop: call before building Claude API request, inject as first part of user message

## Summarization
- [x] Define SUMMARIZE_THRESHOLD constant (default 20 entries)
- [x] Implement `count_entries(content)` — count H2 sections in a memory file
- [x] Implement `maybe_summarize(topic)` — check entry count, trigger if over threshold
- [x] Build summarization prompt: "Summarize, preserve key facts and decisions, keep last 5 entries verbatim"
- [x] Call Claude API with summarization prompt (no tools, just text completion)
- [x] Replace file content with summarized output, preserve frontmatter
- [x] Update frontmatter: reset entries count, add `summarized_at` timestamp
- [x] Run summarization asynchronously (tokio::spawn) — do not block main agent response
- [x] Handle summarization failure gracefully: log error, keep original file intact
