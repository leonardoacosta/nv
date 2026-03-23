# Implementation Tasks

<!-- beads:epic:TBD -->

## Strip Called-Tool Artifacts

- [x] [1.1] [P-1] Add `[Called tool: ...]` pattern removal to `strip_tool_call_artifacts()` in worker.rs — match `[Called tool: <name> with <json>]` lines, remove entire line including preamble [owner:api-engineer]
- [x] [1.2] [P-2] Add tests: single `[Called tool: ...]` line stripped, multiple lines stripped, text-only preserved, similar-looking non-tool brackets preserved [owner:api-engineer]

## Emoji Mapping

- [x] [2.1] [P-1] Change `humanize_tool()` return type from `String` to `(String, String)` — (emoji, description) pairs for all existing mappings, fallback uses `⚙️` [owner:api-engineer]
- [x] [2.2] [P-1] Update `handle_worker_event()` call site for `ToolCalled` to use new `(emoji, description)` return — format as `"{emoji} {description}"` for stage tracking [owner:api-engineer]

## Thinking Message Tool Status

- [x] [3.1] [P-1] Thread `thinking_msg_id` from agent to orchestrator — add `Option<i64>` field to `WorkerEvent::StageStarted` on the first stage event (context_build), store in orchestrator alongside `worker_stage_started` map [owner:api-engineer]
- [x] [3.2] [P-1] On `ToolCalled` event in orchestrator, edit the thinking message via `edit_message(chat_id, thinking_msg_id, "{emoji} {description}")` — skip if no thinking_msg_id or no Telegram channel [owner:api-engineer]
- [x] [3.3] [P-2] Add debounce: track last edit timestamp per worker, skip edit if <500ms since last edit to avoid Telegram rate limits [owner:api-engineer]

## Verify

- [x] [4.1] `cargo build` passes [owner:api-engineer]
- [x] [4.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [4.3] `cargo test` — existing tests pass, new tests for `[Called tool: ...]` stripping [owner:api-engineer]
