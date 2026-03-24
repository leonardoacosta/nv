# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [1.1] [P-1] Rewrite `build_stream_input()` to extract only the latest user message content from `messages` instead of calling `build_prompt()` -- `crates/nv-daemon/src/claude.rs` [owner:api-engineer]
- [x] [1.2] [P-1] In `send_messages_cold_start_with_image()`, build the prompt as user message content ONLY (no system prompt, no tool schemas) since `--system-prompt` flag already passes system context -- `crates/nv-daemon/src/claude.rs` [owner:api-engineer]
- [x] [1.3] [P-2] Remove tool schema serialization from `build_prompt()` when called from cold-start path (tool definitions are not used by CC's `--tools` flag and custom tools are executed by the daemon's tool loop) -- `crates/nv-daemon/src/claude.rs` [owner:api-engineer]
- [x] [1.4] [P-2] Verify payload size logging (prompt_bytes, system_bytes, messages, tools) remains on both persistent and cold-start paths after refactor -- `crates/nv-daemon/src/claude.rs` [owner:api-engineer]

## Verify

- [x] [2.1] Unit test: `build_stream_input()` with system prompt + 5 tools + 3 messages returns only the last user message content (not system prompt, not tool schemas) [owner:api-engineer]
- [x] [2.2] Unit test: cold-start prompt excludes system prompt text and tool schemas [owner:api-engineer]
- [x] [2.3] Integration check: send a "ping" message via Telegram, verify `prompt_bytes` in logs is <8KB (was 53KB — remaining 7KB is intentional context enrichment from worker) [owner:api-engineer]
- [x] [2.4] Existing tests pass: `cargo test -p nv-daemon claude` — 82 passed [owner:api-engineer]
