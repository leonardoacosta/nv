# Context: Fix Agent Cold-Start Critical Bugs

## Source: Audit 2026-03-23 (agent domain, ~78/C+ health)

## Problem
The agent core loop has 3 critical bugs in the cold-start path (the only active path since persistent streaming is disabled).

## Findings

### P1 — parse_tool_calls() drops all but first tool call
- `crates/nv-daemon/src/claude.rs:1163`
- Scans for first ```tool_call block and returns immediately after parsing it
- If Claude emits two tool calls in one turn, only first executes, rest silently discarded
- Since persistent stream path is disabled (`fallback_only: true`), ALL traffic uses cold-start
- Every multi-tool response is affected

### P1 — Worker queue dequeue race condition
- `crates/nv-daemon/src/worker.rs:356`
- After worker completes: `active.fetch_sub(1)` → check `active < max_concurrent` → lock queue → pop
- Between fetch_sub and fetch_add, concurrent workers can both enter the branch
- Both spawn new workers, momentarily exceeding max_concurrent
- Fix: increment active BEFORE popping within same critical section

### P1 — Queued-worker timeout is silent
- `crates/nv-daemon/src/worker.rs:386`
- Queued task timeout logs warn! and decrements active, but does NOT:
  - Emit WorkerEvent::Error
  - Send Telegram timeout message to user
- Primary worker timeout branch (lines 325-350) does both
- Users whose requests were queued see their request disappear without explanation

### P2 — Quiet hours use system timezone, not user timezone
- `crates/nv-daemon/src/orchestrator.rs:1775`
- `is_quiet_hours()` calls `chrono::Local::now().time()` (OS timezone)
- `SharedDeps.timezone` stores user's IANA timezone but is never used here
- Daemon on UTC server + user in America/Chicago = quiet hours 5-6 hours off

### P2 — cmd_digest() hardcodes port 8400
- `crates/nv-daemon/src/orchestrator.rs:1009`
- `/digest` bot command triggers HTTP call to `http://127.0.0.1:8400/digest`
- Actual port is configurable in config — if different, bot command fails silently

### P2 — StageComplete clears tracking before ToolCalled events arrive
- `crates/nv-daemon/src/orchestrator.rs:660`
- Race between StageComplete removing worker from tracking and ToolCalled re-inserting
- If check_inactivity fires in between, Telegram typing indicator expires mid-tool

## Files to Modify
- `crates/nv-daemon/src/claude.rs` (parse_tool_calls)
- `crates/nv-daemon/src/worker.rs` (queue dequeue, timeout handling)
- `crates/nv-daemon/src/orchestrator.rs` (quiet hours, port, stage tracking)
