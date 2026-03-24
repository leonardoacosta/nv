# Proposal: Remove Dead Code and Duplications

## Change ID
`cleanup-dead-code`

## Summary

Delete ~1500+ lines of dead code and unify three duplicated implementations across
`nv-daemon`. Targets are all confirmed by `#[allow(dead_code)]` attributes and zero
call-site references: the obsolete `AgentLoop` struct (superseded by Worker/WorkerPool/
Orchestrator), `execute_tool()` (duplicates `execute_tool_send()`),
`send_messages_cold_start()` (superseded by `send_messages_cold_start_with_image()`),
and a scattered set of noise-level P3/P4 items (unused function, wrong test assertion,
stale `#[allow(dead_code)]` attributes).

## Context
- Modifies: `crates/nv-daemon/src/agent.rs`
- Modifies: `crates/nv-daemon/src/tools/mod.rs`
- Modifies: `crates/nv-daemon/src/claude.rs`
- Modifies: `crates/nv-daemon/src/worker.rs`
- Modifies: `crates/nv-daemon/src/orchestrator.rs`
- Modifies: `crates/nv-daemon/src/memory.rs`
- Modifies: `crates/nv-daemon/src/shutdown.rs`
- Modifies: `crates/nv-daemon/src/messages.rs`
- Modifies: `crates/nv-daemon/src/nexus/client.rs`
- Modifies: `crates/nv-daemon/src/query/mod.rs`

## Motivation

Dead code masked by `#[allow(dead_code)]` is a maintenance liability:

1. **Silent divergence** — `AgentLoop` duplicates callback handling, context building,
   the tool loop, and session expiry. As the live path (Orchestrator + Worker) evolves,
   the dead copy drifts silently and misleads future readers into thinking there are two
   real implementations.
2. **O(N) maintenance burden** — `execute_tool()` replicates the full dispatch table of
   `execute_tool_send()`. Every new tool requires edits in two places; the dead copy
   routinely lags and diverges.
3. **Wasted code surface** — `send_messages_cold_start()` (~160 lines) duplicates
   `send_messages_cold_start_with_image()`. Its own doc comment says "kept as a
   reference" — that is what git history is for.
4. **Suppression noise** — blanket `#[allow(dead_code)]` on live structs
   (`SessionSummary`, `SessionDetail`) and on entire submodule declarations
   (`query/mod.rs`) hides real warnings in the future and signals false positives to
   reviewers.
5. **Wrong test assertion** — `messages.rs` test asserts `user_version == 1` but 4
   migrations have been added, so the real version is 4; the test passes only because
   `rusqlite_migration` ignores the assertion if user_version is already at the latest.

## Requirements

### Req-1: Delete AgentLoop

Delete the `AgentLoop` struct (agent.rs:200–228) and its entire `impl` block
(agent.rs:229–1256). The free functions that follow (`format_trigger_batch`,
`truncate_history`, `extract_cli_response_channels`, `extract_text`,
`classify_triggers`, `summarize_sources`) are called by `worker.rs` and must be
retained. No callers of `AgentLoop::new` or any `AgentLoop` method exist outside the
struct itself.

### Req-2: Delete execute_tool()

Delete `execute_tool()` at tools/mod.rs:2266. It is annotated `#[allow(dead_code)]`
and no call site exists. `execute_tool_send()` is the live path used by `worker.rs`.
The function is large (dispatches ~100 tool arms) — deleting it removes significant
duplicated surface.

### Req-3: Delete send_messages_cold_start()

Delete `send_messages_cold_start()` at claude.rs:773–937. It is annotated
`#[allow(dead_code)]`. Its own doc comment reads "kept as a reference; prefer
`send_messages_cold_start_with_image` which supersedes this." No call sites exist.

### Req-4: Unify truncate_history() — move to shared location

`agent.rs:1304` and `worker.rs:1632` each define a private `fn truncate_history()`.
The two implementations are identical in logic. Extract the single canonical copy into
`conversation.rs` (or the closest shared utility module in `nv-daemon`) as a
`pub(crate)` function. Update `worker.rs` to call it. Delete the `agent.rs` copy; the
remaining free functions in `agent.rs` stay.

Note: `agent.rs:truncate_history` is only ever called from within `AgentLoop` methods
(which are deleted in Req-1), so after Req-1 it will already be unreachable. The
consolidation step in Req-4 is still worth doing to make `worker.rs` reference a
shared location rather than its own private copy.

### Req-5: Unify extract_cli_response_channels() — remove agent.rs copy

`agent.rs:1330` and `orchestrator.rs:1688` each define a private
`fn extract_cli_response_channels()`. They are identical. The `agent.rs` copy is only
called from within `AgentLoop` methods (deleted in Req-1), making it dead after Req-1.
Delete the `agent.rs` copy. The `orchestrator.rs` copy is the live path and stays.

### Req-6: Resolve flush_error_batch_if_expired()

`orchestrator.rs:1427` defines `flush_error_batch_if_expired()` annotated
`#[allow(dead_code)]`. Its doc comment states it should be called on a periodic tick
so the final error batch in a sequence is never silently dropped.

Wire it into the Orchestrator's existing periodic tick (the `select!` arm that fires
the expiry sweep for pending actions) so it runs on every tick cycle. Remove the
`#[allow(dead_code)]` attribute once it is called.

### Req-7: Delete get_context_summary()

`memory.rs:272` defines `get_context_summary()` annotated `#[allow(dead_code)]`. All
live call sites use `get_context_summary_for()` (the trigger-aware variant at
memory.rs:310). However, `get_context_summary()` is called directly in three unit
tests (memory.rs:897, 908, 924). Delete `get_context_summary()` and migrate its three
unit tests to call `get_context_summary_for()` with a representative trigger string,
so the same coverage is preserved under the live function.

### Req-8: Remove or wire drain_with_timeout()

`shutdown.rs:37` defines `drain_with_timeout()` annotated `#[allow(dead_code)]`. It
has three unit tests in the same file, so it is tested but never called from
production code. The doc comment suggests it belongs in the shutdown `select!` arm.

Wire `drain_with_timeout()` into the daemon shutdown path — call it on the worker
trigger channel (or whichever unbounded receiver is appropriate) during the graceful
shutdown sequence. Remove the `#[allow(dead_code)]` attribute. If the shutdown path
does not have an obvious wiring point, delete the function and its three tests instead
of leaving it annotated indefinitely.

### Req-9: Remove stale #[allow(dead_code)] from used structs

`nexus/client.rs:13` and `nexus/client.rs:27` annotate `SessionSummary` and
`SessionDetail` with `#[allow(dead_code)]`. Both structs have all fields accessed by
callers. Remove the attributes.

`query/mod.rs:1–8` has four `#[allow(dead_code)]` lines suppressing warnings on the
four submodule declarations (`followup`, `format`, `gather`, `synthesize`). Check
whether the suppression is still needed for each module; remove any attribute whose
module's public items are actually used. If a module is genuinely unused, either wire
it or delete it rather than blanket-suppressing.

### Req-10: Fix migration test user_version assertion

`messages.rs:769` asserts `assert_eq!(version, 1, ...)`. There are 4 migrations in
`messages_migrations()` (lines 78–194). Change the assertion to
`assert_eq!(version, 4)` and update the comment to match.

## Scope
- **IN**: All 10 items above — deletions, consolidation, one wiring task, attribute
  cleanup, one test fix
- **OUT**: Behavioral changes to any live code path, new features, refactoring of
  `execute_tool_send()` internals, any schema or migration changes

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/agent.rs` | Delete AgentLoop struct + impl (~1050 lines); delete `truncate_history` and `extract_cli_response_channels` free functions (superseded by Req-1/Req-4/Req-5) |
| `crates/nv-daemon/src/tools/mod.rs` | Delete `execute_tool()` (~300+ lines) |
| `crates/nv-daemon/src/claude.rs` | Delete `send_messages_cold_start()` (~165 lines) |
| `crates/nv-daemon/src/worker.rs` | Update `truncate_history` call site to use shared location |
| `crates/nv-daemon/src/orchestrator.rs` | Wire `flush_error_batch_if_expired()` into tick; remove `#[allow(dead_code)]` |
| `crates/nv-daemon/src/memory.rs` | Delete `get_context_summary()`; migrate 3 unit tests to `get_context_summary_for()` |
| `crates/nv-daemon/src/shutdown.rs` | Wire or delete `drain_with_timeout()` |
| `crates/nv-daemon/src/messages.rs` | Fix `assert_eq!(version, 1)` → `assert_eq!(version, 4)` |
| `crates/nv-daemon/src/nexus/client.rs` | Remove 2 `#[allow(dead_code)]` attributes |
| `crates/nv-daemon/src/query/mod.rs` | Remove stale `#[allow(dead_code)]` attributes on live submodules |
| New: `crates/nv-daemon/src/conversation.rs` (or equivalent shared module) | Add `pub(crate) fn truncate_history()` |

## Risks
| Risk | Mitigation |
|------|-----------|
| AgentLoop methods reference free functions that are also used by worker.rs | Free functions after line 1258 in agent.rs are retained; only the struct and impl block are deleted |
| Wiring flush_error_batch_if_expired() changes observable behavior | It only sends an error notification that was previously silently dropped — additive, not breaking |
| Migrating get_context_summary() tests to get_context_summary_for() reduces coverage | Both functions read from the same file store; coverage is equivalent with a fixed trigger string |
| drain_with_timeout() wiring requires knowing which channel to drain | Engineer should inspect the shutdown select! arm in main.rs/daemon.rs for the appropriate receiver |
