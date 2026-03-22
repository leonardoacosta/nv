# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Refactor ClaudeClient to hold persistent subprocess state: Arc<Mutex<Option<PersistentProcess>>> where PersistentProcess wraps tokio::process::Child with stdin (ChildStdin) and stdout (BufReader<ChildStdout>) handles [owner:api-engineer]
- [x] [2.2] [P-1] Implement spawn_persistent() — spawn `claude` with --input-format stream-json, --output-format stream-json, --dangerously-skip-permissions, --model, --tools, --system-prompt, sandboxed HOME; return PersistentProcess [owner:api-engineer]
- [x] [2.3] [P-1] Implement stdin writer: serialize conversation (system, messages, tool results) into stream-json input format, write as newline-delimited JSON to subprocess stdin [owner:api-engineer]
- [x] [2.4] [P-1] Implement stdout reader: parse stream-json events line-by-line (message_start, content_block_delta, content_block_stop, result), accumulate text + tool_use blocks, produce ApiResponse [owner:api-engineer]
- [x] [2.5] [P-1] Implement auto-restart with backoff: detect subprocess death (poll Child), restart with exponential backoff (1s/2s/4s, max 30s), track consecutive failures, switch to fallback after 5 failures [owner:api-engineer]
- [x] [2.6] [P-1] Implement fallback mode: when persistent fails, transparently invoke existing cold-start `claude -p` path; log warning via tracing [owner:api-engineer]
- [x] [2.7] [P-2] Update send_messages() public API: try persistent path first, fall back to cold-start on error; maintain same ApiResponse return type [owner:api-engineer]
- [x] [2.8] [P-2] Wire persistent client startup in main.rs: spawn subprocess on daemon init, pass ClaudeClient with persistent state to AgentLoop [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit tests: stream-json event parsing (text content, tool_use blocks, result with usage) [owner:api-engineer]
- [x] [3.4] Unit tests: stdin message serialization round-trip [owner:api-engineer]
- [x] [3.5] Unit tests: auto-restart backoff timing, fallback transition after 5 failures [owner:api-engineer]
- [x] [3.6] Unit tests: fallback mode produces valid ApiResponse via cold-start path [owner:api-engineer]
- [x] [3.7] Existing tests pass (agent loop, tool parsing, response routing) [owner:api-engineer]
