# Context: Remove Dead Code and Duplications

## Source: Audit 2026-03-23 (cross-domain, recurring pattern)

## Problem
~1500+ lines of dead code masked by #[allow(dead_code)] across 6 modules, plus 3 duplicated implementations.

## Findings

### P2 — AgentLoop is ~700 lines of dead code
- `crates/nv-daemon/src/agent.rs:200`
- Entire AgentLoop impl block annotated #[allow(dead_code)]
- Never instantiated — real path is Worker + WorkerPool + Orchestrator
- Duplicates callback handling, context building, tool loop, session expiry
- Will silently diverge from active implementation
- Fix: Delete AgentLoop and its impl

### P2 — execute_tool() duplicates execute_tool_send()
- `crates/nv-daemon/src/tools/mod.rs:2267`
- #[allow(dead_code)], replicates full dispatch logic
- O(N) maintenance burden across 100 tools
- Fix: Delete entirely

### P2 — send_messages_cold_start() superseded
- `crates/nv-daemon/src/claude.rs:755`
- #[allow(dead_code)], ~160 lines duplicating with_image variant
- Fix: Delete

### P2 — truncate_history() duplicated with different implementations
- `crates/nv-daemon/src/agent.rs:1304`
- `crates/nv-daemon/src/worker.rs:1632`
- Two copies with slightly different internals, worker.rs is the live path
- Fix: Unify in conversation.rs or shared module

### P2 — extract_cli_response_channels() duplicated
- `crates/nv-daemon/src/agent.rs:1330`
- `crates/nv-daemon/src/orchestrator.rs:1688`
- Fix: Extract to shared module, remove agent.rs copy

### P3 — flush_error_batch_if_expired() never called
- `crates/nv-daemon/src/orchestrator.rs:1427`
- #[allow(dead_code)] — final error in any sequence silently dropped
- Fix: Either wire into periodic tick or delete

### P3 — get_context_summary() dead code
- `crates/nv-daemon/src/memory.rs:272`
- #[allow(dead_code)] — decide: reserved for future or delete

### P4 — drain_with_timeout not wired into shutdown
- `crates/nv-daemon/src/shutdown.rs:37`
- Fix: Use in shutdown select arm, or remove

### P4 — Various #[allow(dead_code)] on used structs
- SessionSummary, SessionDetail in nexus — all fields used, attr is noise
- query/mod.rs blanket suppression on all four submodules

### P4 — Migration test asserts wrong user_version
- `crates/nv-daemon/src/messages.rs:768`
- `assert_eq!(version, 1)` but 4 migrations → version should be 4
- Fix: Change to assert_eq!(version, 4)

## Files to Modify
- `crates/nv-daemon/src/agent.rs` (AgentLoop deletion, truncate_history, extract_cli_response_channels)
- `crates/nv-daemon/src/tools/mod.rs` (execute_tool deletion)
- `crates/nv-daemon/src/claude.rs` (send_messages_cold_start deletion)
- `crates/nv-daemon/src/worker.rs` (move truncate_history to shared)
- `crates/nv-daemon/src/orchestrator.rs` (flush_error_batch, extract_cli_response_channels)
- `crates/nv-daemon/src/memory.rs` (get_context_summary)
- `crates/nv-daemon/src/shutdown.rs` (drain_with_timeout)
- `crates/nv-daemon/src/messages.rs` (test assertion)
- `crates/nv-daemon/src/nexus/client.rs` (dead_code attrs)
- `crates/nv-daemon/src/query/mod.rs` (dead_code attrs)
