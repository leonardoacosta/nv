# Implementation Tasks

<!-- beads:epic:nv-4u1 -->

## API Batch

- [x] [1.1] Create `conversation.rs` -- ConversationStore struct with `push(user, assistant)`, `load() -> Vec<Message>`, session expiry (`SESSION_TIMEOUT` 600s), and trim to `MAX_HISTORY_TURNS` (20) / `MAX_HISTORY_CHARS` (50,000) [beads:nv-xw9] [owner:api-engineer]
- [x] [1.2] Register ConversationStore in SharedDeps (`Arc<Mutex<ConversationStore>>`) and construct in `main.rs` [beads:nv-am6] [owner:api-engineer]
- [x] [1.3] Wire ConversationStore into `Worker::run` -- call `store.load()` before Claude API call to prepend prior turns, call `store.push(user_msg, assistant_msg)` after response [beads:nv-vra] [owner:api-engineer]
- [x] [1.4] Add tool_result truncation in `ConversationStore::push` -- truncate `ToolResult` content blocks exceeding 1,000 chars, append `...[truncated]` marker [beads:nv-2un] [owner:api-engineer]
- [x] [1.5] Bump `format_recent_for_context` content truncation from 500 to 2,000 chars and add turn-pair grouping (`--- turn ---` / `--- end turn ---` markers) [beads:nv-37e] [owner:api-engineer]
- [x] [1.6] Move history constants from `agent.rs` to `conversation.rs` (if any remain), remove stale `#[allow(dead_code)]` attrs on now-used constants [beads:nv-56z] [owner:api-engineer]
- [x] [1.7] Populate `config/identity.md` with full Nova identity details -- remove placeholder content [beads:nv-4ym] [owner:api-engineer]
- [x] [1.8] Populate `config/user.md` with actual operator details -- remove placeholder content [beads:nv-flo] [owner:api-engineer]

## Verify Batch

- [x] [2.1] Unit tests for ConversationStore: push/load round-trip, session expiry clears turns, trim by turn count, trim by char limit, activity timer resets [beads:nv-4eq] [owner:api-engineer]
- [x] [2.2] Unit test for tool_result truncation: content >1,000 chars is truncated with `...[truncated]` marker, content <=1,000 chars passes through unchanged [beads:nv-wcd] [owner:api-engineer]
- [x] [2.3] Unit test for format_recent_for_context: 2,000-char truncation limit applied, turn-pair grouping markers present in output [beads:nv-20g] [owner:api-engineer]
- [x] [2.4] `cargo build` passes [owner:api-engineer]
- [x] [2.5] `cargo test` passes (all existing + new tests) [owner:api-engineer]
- [x] [2.6] `cargo clippy -- -D warnings` passes [owner:api-engineer]
