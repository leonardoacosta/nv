# Proposal: Fix Persistent Claude Subprocess

## Change ID
`fix-persistent-claude-subprocess`

## Summary

Fix three regressions in the persistent Claude CLI subprocess caused by CC v2.1.81 breaking
changes: update the stream-json input format, add hook event draining on startup, and switch
from sandbox HOME to real HOME for OAuth compatibility.

## Context
- Extends: `crates/nv-daemon/src/claude.rs` (StreamJsonInput, spawn_persistent, send_turn)
- Related: `openspec/changes/archive/2026-03-22-persistent-claude-session/` (original implementation)
- Trigger: CC v2.1.81 changed the expected stdin input format; persistent path has been broken since

## Motivation

The persistent Claude subprocess (implemented to eliminate 8-14s cold-start latency per turn) is
non-functional since CC updated to v2.1.81. Every turn silently fails and falls back to cold-start
mode, negating the entire performance benefit. Three distinct issues compound:

1. **Input format mismatch** — Nova sends `{"type":"user","content":"..."}` but CC 2.1.81 expects
   `{"message":{"role":"user","content":"..."}}`. The subprocess logs:
   `Error parsing streaming input line: TypeError: undefined is not an object (evaluating '$.message.role')`

2. **No hook draining** — With real HOME, CC fires ~11 SessionStart hooks on spawn (~2s). The
   daemon writes user input during hook processing, which may corrupt the stream. There is no
   startup drain phase.

3. **Sandbox HOME too bare** — The sandbox at `~/.nv/claude-sandbox/` lacks files CC 2.1.81 needs
   to initialize. Real HOME has valid OAuth credentials but triggers hook events.

The fix is straightforward: update the input struct, add a drain phase after spawn, and use real
HOME (hooks are harmless — just skip them in the reader).

## Requirements

### Req-1: Update StreamJsonInput Format

Change the `StreamJsonInput` struct serialization from the flat format
`{"type":"user","content":"..."}` to the nested format CC 2.1.81 expects:
`{"message":{"role":"user","content":"..."}}`. The struct and its Serialize impl must produce
the new format. The `msg_type` field becomes the `role` field inside a nested `message` object.

### Req-2: Hook Event Draining on Startup

After `spawn_persistent()` creates the subprocess, add a drain phase that reads stdout lines
until all `system` type events (hook invocations, init messages) are consumed. The drain must:
- Read lines in a loop, parsing each as JSON
- Skip any event where `type` is `"system"`
- Stop when a non-system event arrives (buffer it for the first turn), 10 seconds elapse (timeout,
  proceed anyway), or EOF (process died — return error so caller falls back to cold-start)
- Log each skipped system event at debug level

### Req-3: Use Real HOME Instead of Sandbox

Replace `config.sandbox_home` with `config.real_home` in the `Command::env("HOME", ...)` call
within `spawn_persistent()`. Real HOME has valid OAuth credentials, CC config, and all required
initialization files. The `--dangerously-skip-permissions` and `--tools` flags already restrict
what the subprocess can do. Remove the `sandbox_home` field from `SpawnConfig` since it is no
longer needed.

### Req-4: Ready Detection After Drain

After the drain phase completes, verify the subprocess is alive by calling `try_wait()` on the
child process. If it returns `Some(exit_status)`, the process died during init — return an error
so the caller falls back to cold-start. If it returns `None`, the process is alive and ready.

## Scope
- **IN**: StreamJsonInput format fix, spawn drain phase, HOME switch, ready detection, tests
- **OUT**: Changes to cold-start path, agent.rs, worker.rs, new CLI flags, streaming to Telegram

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/claude.rs` | StreamJsonInput struct + serialization, spawn_persistent drain phase, HOME env var, SpawnConfig cleanup |

Single-file change. No other files affected.

## Risks
| Risk | Mitigation |
|------|-----------|
| CC changes input format again in future versions | Defensive parsing with clear error messages; cold-start fallback remains |
| Hook drain timeout too short for slow machines | 10s is generous (hooks take ~2s on dev machine); configurable later if needed |
| Real HOME exposes user config to subprocess | Already mitigated by --dangerously-skip-permissions + --tools whitelist |
| Drain phase buffers a non-system event that is never consumed | Store the buffered event and return it as the first line in the next read |
