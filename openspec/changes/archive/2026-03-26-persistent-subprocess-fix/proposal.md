# Proposal: Persistent Subprocess Fix

## Change ID
`persistent-subprocess-fix`

## Summary

Diagnose and fix the CC stream-json subprocess hang that prevents persistent sessions from working
reliably. Write a minimal test harness, identify the root cause (likely hook loading triggered by
`--tools-json` or init drain timeout too short), apply targeted fixes, and enable persistent mode
(`fallback_only: false`). Add an integration test covering `FALLBACK_RESET_DURATION`.

## Context

- Extends: `crates/nv-daemon/src/claude.rs` (`PersistentSession`, `spawn_persistent`,
  `drain_init_events`, `SessionInner`)
- Related: `streaming-response-delivery` (Wave 1 — shares the same stream-json infrastructure;
  streaming delivery is the primary consumer of a working persistent session)
- Carry-forward: `response-latency-optimization` Req-3 (nova-v7 roadmap, Phase 5 Wave 11)

## Motivation

`PersistentSession` was fully implemented in an earlier wave but was forcibly disabled via
`fallback_only: true` due to a hang: the stream-json subprocess spawns but never emits any
response events after the user turn is written to stdin. The code comment reads:

> "Persistent mode disabled: the CC CLI stream-json subprocess never sends response data back
> (likely a CC 2.1.81 bug with stream-json + hooks). Cold-start mode works reliably (~8s).
> Re-enable once the root cause is identified."

Cold-start mode costs 8-14 seconds per turn (subprocess spawn + CC hook loading + model
generation). Persistent mode eliminates the subprocess spawn and hook loading overhead, targeting
~2s response latency. The `streaming-response-delivery` spec (already written) is also blocked
on a working persistent session — streaming edits only activate on the persistent path.

The hang has two plausible root causes:

1. **Hook loading under `--tools-json`**: When the daemon passes `--tools-json`, the CC CLI may
   trigger its hook-loading mechanism (`.claude/settings.json`) during process init, which reads
   MCP server configs and attempts to spawn MCP servers. In the daemon's sandbox environment
   (`HOME=~/.nv/claude-sandbox`) these servers don't exist or can't launch, causing the subprocess
   to stall waiting for MCP init to complete before emitting any `{"type":"system"}` events.

2. **Init drain timeout too short**: The `drain_init_events` function has a 10-second timeout.
   If CC emits many system events before ready (e.g. hook-loading logs, MCP probes), the drain
   may time out before the subprocess is ready, causing the first user turn to be sent into a
   subprocess that isn't yet listening — and the response is swallowed by the init event stream.

The fix strategy: remove `--tools-json` from persistent subprocess spawn args (the daemon already
sends tool definitions inline per-turn in the stream-json message), add `--no-mcp` to suppress
MCP loading entirely, and increase `drain_init_events` timeout from 10s to 20s. If the subprocess
becomes responsive after these changes, enable persistent mode.

## Requirements

### Req-1: Minimal Integration Test Harness

Add a `#[tokio::test]` test in `claude.rs` (or a separate `tests/persistent_session.rs`) that:

1. Spawns a real `claude` subprocess with the stream-json flags used by `spawn_persistent`
   (minus `--tools-json`, plus `--no-mcp`, with 20s drain timeout).
2. Sends a single minimal turn: `{"message":{"role":"user","content":"Say exactly: pong"}}`.
3. Reads events from stdout until a `result` event arrives or a 15-second timeout fires.
4. Asserts the response contains text with "pong" (case-insensitive) — proving the subprocess
   returns data.

This test is marked `#[ignore]` so it does not run in `cargo test` by default (requires a live
`claude` binary and valid OAuth session). Run explicitly with:
`cargo test -p nv-daemon -- persistent_subprocess_smoke --ignored`.

The test must fail before the fix (to document the root cause) and pass after.

### Req-2: Remove `--tools-json` from Persistent Spawn Args

In `spawn_persistent`, remove the `--tools-json` branch entirely. Tool definitions are already
sent inline as part of each stream-json turn via the `StreamJsonInput` message content (the
agent loop embeds tool context in the system prompt portion of the first user message). The
persistent subprocess does not need `--tools-json` at spawn time.

Update `SpawnConfig` to remove the `tools_json` field. Update `PersistentSession::new` and
`PersistentSession::send_turn` to stop serializing/comparing/updating `tools_json`. The tool
change detection block in `send_turn` can also be removed (no longer needed without per-spawn
tool registration).

### Req-3: Add `--no-mcp` Flag to Persistent Spawn

In `spawn_persistent`, add `"--no-mcp".into()` to `base_args` before spawning. This suppresses
MCP server loading in the persistent subprocess — the daemon provides tools via the stream-json
protocol, not via MCP. The cold-start path (`send_messages_cold_start_with_image`) already does
not use MCP servers; persistent mode should match.

### Req-4: Increase `drain_init_events` Timeout to 20s

In `drain_init_events`, change `Duration::from_secs(10)` to `Duration::from_secs(20)`. The
subprocess may emit multiple system events (hook load confirmations, version info) before
signaling readiness. 20 seconds provides adequate margin without blocking the first turn for too
long.

Update the warning log message to reflect the new timeout value.

### Req-5: Enable Persistent Mode

In `PersistentSession::new`, change `fallback_only: true` to `fallback_only: false`. Remove the
comment block explaining why it was disabled (the fix resolves the root cause). Update the comment
to state that persistent mode is active.

This is the gate: only land Req-5 after the smoke test from Req-1 passes against a live `claude`
binary on the development machine. The implementation task list reflects this ordering.

### Req-6: Integration Test for `FALLBACK_RESET_DURATION`

Add a unit test (no live `claude` required, uses `SessionInner` directly) that:

1. Constructs a `SessionInner` with `fallback_only: true` and `last_failure_at` set to
   `Instant::now() - FALLBACK_RESET_DURATION - Duration::from_secs(1)` (just past the window).
2. Calls `PersistentSession::ensure_alive` and asserts it attempts a spawn (i.e. returns `true`
   after the reset logic clears `fallback_only`).
3. Constructs a second `SessionInner` with `last_failure_at` set to `Instant::now()` (within the
   window).
4. Calls `ensure_alive` and asserts it returns `false` immediately (cooldown not elapsed).

This test can run in the normal `cargo test` suite (no live CLI needed) by mocking the spawn step
or by using `SpawnConfig` with a non-existent binary path (spawn failure is acceptable — the test
validates the reset logic, not the spawn outcome).

## Scope

- **IN**: smoke test harness, remove `--tools-json` from persistent spawn, add `--no-mcp`, increase
  drain timeout to 20s, enable persistent mode, `FALLBACK_RESET_DURATION` unit test
- **OUT**: Changes to cold-start path, streaming delivery wiring (covered by
  `streaming-response-delivery`), session resume (`--continue`), tool definition changes

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/claude.rs` | Remove `tools_json` from `SpawnConfig`; remove `--tools-json` from `spawn_persistent`; add `--no-mcp`; increase drain timeout to 20s; remove tool-change detection block from `send_turn`; set `fallback_only: false`; add two tests |

## Risks

| Risk | Mitigation |
|------|-----------|
| Root cause is not `--tools-json` or drain timeout — subprocess still hangs | Smoke test (Req-1) must pass before enabling (Req-5); if it fails, root cause investigation continues and Req-5 is not landed |
| Removing `--tools-json` breaks tool call routing in persistent mode | Tool definitions are already sent in the stream-json message content via `build_stream_input`; `--tools-json` was additive and is not the primary tool mechanism |
| `--no-mcp` flag does not exist in installed CC version | Check `claude --help` for `--no-mcp`; if absent, skip this flag silently (log a warning); fallback: pass empty MCP config via env var |
| 20s drain timeout delays startup when subprocess crashes early | `drain_init_events` returns `Err` on EOF immediately — the timeout only applies to the case where the subprocess is alive but slow; no regression on crash-during-init |
| Persistent session memory growth over long uptime | Existing RSS monitor and restart-on-crash logic unchanged; no new risk from this fix |
