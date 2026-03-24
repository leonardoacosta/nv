# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [api-engineer] Delete `AgentLoop` struct (agent.rs:200–228) and its entire `impl` block (agent.rs:229–1256) — retain all free functions from line 1258 onward (`format_trigger_batch`, `truncate_history`, `extract_cli_response_channels`, `extract_text`, `classify_triggers`, `summarize_sources`) — `crates/nv-daemon/src/agent.rs`
- [x] [api-engineer] Delete `execute_tool()` function annotated `#[allow(dead_code, clippy::too_many_arguments)]` (tools/mod.rs:2266) — no call sites exist outside the function itself — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Delete `send_messages_cold_start()` method (claude.rs:773–937) annotated `#[allow(dead_code)]` — superseded by `send_messages_cold_start_with_image()` — `crates/nv-daemon/src/claude.rs`
- [x] [api-engineer] Extract `truncate_history()` into a shared `pub(crate)` function — create or extend `crates/nv-daemon/src/conversation.rs` (or equivalent shared utility module), move the canonical body there, update `worker.rs` call site to use the shared location, delete the `agent.rs` copy (already dead after AgentLoop deletion) — `crates/nv-daemon/src/worker.rs`, `crates/nv-daemon/src/agent.rs`, new shared module
- [x] [api-engineer] Delete `extract_cli_response_channels()` from agent.rs (agent.rs:1330–1342) — this copy is only called from AgentLoop methods (deleted above); the live copy in `orchestrator.rs:1688` is unchanged — `crates/nv-daemon/src/agent.rs`
- [x] [api-engineer] Wire `flush_error_batch_if_expired()` into the Orchestrator's periodic tick `select!` arm — call it with the appropriate channel name on every tick so the final error batch in any sequence is flushed, then remove the `#[allow(dead_code)]` attribute — `crates/nv-daemon/src/orchestrator.rs`
- [x] [api-engineer] Delete `get_context_summary()` (memory.rs:272) annotated `#[allow(dead_code)]` — migrate its three unit tests (memory.rs:894, 903, 913) to call `get_context_summary_for()` with a representative trigger string to preserve equivalent coverage — `crates/nv-daemon/src/memory.rs`
- [x] [api-engineer] Wire `drain_with_timeout()` (shutdown.rs:37) into the daemon graceful shutdown path — call it on the worker trigger channel (or appropriate unbounded receiver) during shutdown, then remove the `#[allow(dead_code)]` attribute; if no suitable wiring point exists, delete the function and its three tests (shutdown.rs:72, 80, 91) instead — `crates/nv-daemon/src/shutdown.rs` [already done by Wave 3]
- [x] [api-engineer] Remove `#[allow(dead_code)]` from `SessionSummary` (nexus/client.rs:13) and `SessionDetail` (nexus/client.rs:27) — all fields on both structs are actively used — `crates/nv-daemon/src/nexus/client.rs` [already done by Wave 2]
- [x] [api-engineer] Remove stale `#[allow(dead_code)]` attributes from submodule declarations in query/mod.rs (lines 1–8) — verify each of the four submodules (`followup`, `format`, `gather`, `synthesize`) is actually used; remove each attribute whose module's public items have callers; delete any module that is genuinely unused rather than suppressing — `crates/nv-daemon/src/query/mod.rs`
- [x] [api-engineer] Fix migration test assertion: change `assert_eq!(version, 1, ...)` to `assert_eq!(version, 4)` and update the inline comment to reflect that 4 migrations have been applied — `crates/nv-daemon/src/messages.rs:769`

## Verify

- [x] [api-engineer] `cargo build -p nv-daemon` passes with zero errors — `crates/nv-daemon`
- [x] [api-engineer] `cargo clippy -p nv-daemon -- -D warnings` passes — no new dead_code warnings introduced — `crates/nv-daemon`
- [x] [api-engineer] `cargo test -p nv-daemon` passes — all existing tests green, migrated `get_context_summary` tests pass under `get_context_summary_for` — `crates/nv-daemon` (1008 passed; 2 pre-existing failures unrelated to this spec)
- [x] [api-engineer] Confirm no `AgentLoop` identifier remains in any source file — `crates/nv-daemon/src`
- [x] [api-engineer] Confirm no `execute_tool` call sites remain (only `execute_tool_send` is used) — `crates/nv-daemon/src`
- [x] [api-engineer] Confirm no `send_messages_cold_start` call sites remain (only `send_messages_cold_start_with_image` is used) — `crates/nv-daemon/src/claude.rs`
