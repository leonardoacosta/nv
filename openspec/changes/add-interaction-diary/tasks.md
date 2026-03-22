# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [x] [1.1] [P-1] Create crates/nv-daemon/src/diary.rs — DiaryWriter struct with base_path, init() to create ~/.nv/diary/, write_entry() that appends markdown to daily file [owner:api-engineer]
- [x] [1.2] [P-1] Define DiaryEntry struct — timestamp, trigger_type, trigger_source, trigger_count, tools_called (Vec<String>), sources_checked (summary string), result (summary string), tokens_in, tokens_out [owner:api-engineer]
- [x] [1.3] [P-2] Update agent.rs — collect tool call names during tool use loop into a Vec<String>, capture token counts from ApiResponse.usage [owner:api-engineer]
- [x] [1.4] [P-2] Update agent.rs — after response routing, build DiaryEntry from collected data and call diary.write_entry() [owner:api-engineer]
- [x] [1.5] [P-2] Update main.rs — init diary directory on startup, pass DiaryWriter to AgentLoop [owner:api-engineer]
- [x] [1.6] [P-2] Add mod diary declaration in main.rs [owner:api-engineer]

## Verify

- [x] [2.1] cargo build passes [owner:api-engineer]
- [x] [2.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [2.3] cargo test — new diary module tests (init, write_entry, daily rolling, entry format) + existing tests pass [owner:api-engineer]
