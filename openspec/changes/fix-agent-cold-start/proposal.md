# Proposal: Fix Agent Cold-Start Bugs

## Change ID
`fix-agent-cold-start`

## Summary

Fix six bugs in the cold-start agent path: a multi-tool-call parser that silently drops all but
the first call, a worker queue dequeue race that momentarily exceeds max_concurrent, a queued
worker timeout that is silent to the user, quiet hours using the OS timezone instead of the user's
configured timezone, a hardcoded port 8400 in the /digest bot command, and a StageComplete event
that clears worker tracking before ToolCalled events can re-insert.

## Context
- Extends: `crates/nv-daemon/src/claude.rs` (parse_tool_calls)
- Extends: `crates/nv-daemon/src/worker.rs` (WorkerPool queue dequeue, queued-worker timeout)
- Extends: `crates/nv-daemon/src/orchestrator.rs` (is_quiet_hours, cmd_digest, StageComplete handling)
- Source: Audit 2026-03-23 (agent domain, ~78/C+ health)

## Motivation

Since persistent streaming is disabled (`fallback_only: true`), every Claude turn uses the
cold-start path. Six bugs in that path affect correctness or user experience:

1. **Multi-tool-call truncation** — `parse_tool_calls()` finds the first ` ```tool_call ` block
   and returns immediately. A response with two tool calls executes only the first; the rest are
   silently dropped with no log and no error. Because all traffic is cold-start, this affects
   every multi-tool turn.

2. **Worker queue dequeue race** — After a worker finishes it does `active.fetch_sub(1)`, then
   reads `active.load()`, then checks `< max_concurrent`, then `active.fetch_add(1)`. Two
   concurrent workers completing simultaneously can both pass the `< max_concurrent` gate before
   either increments, spawning two replacements and momentarily exceeding the concurrency limit.

3. **Silent queued-worker timeout** — The primary worker timeout branch (worker.rs lines 318-350)
   emits `WorkerEvent::Error` and sends a Telegram message. The queued-worker timeout branch
   (worker.rs lines 386-393) only logs a `warn!` and decrements `active`. Users whose requests
   were queued see their request disappear without any explanation.

4. **Quiet hours use OS timezone** — `is_quiet_hours()` calls `chrono::Local::now().time()` which
   uses the OS timezone of the host machine (UTC on the homelab server). `SharedDeps.timezone`
   holds the user's IANA timezone but is never consulted here. For a user in America/Chicago, quiet
   hours are 5-6 hours off.

5. **cmd_digest() hardcodes port 8400** — `cmd_digest()` builds its HTTP request with a literal
   `let port = 8400`. The actual HTTP server port comes from `config.daemon.health_port` (with
   the same 8400 default). If the config differs, the /digest bot command fails silently with a
   connection-refused error.

6. **StageComplete clears tracking prematurely** — `WorkerEvent::StageComplete` removes the worker
   from `worker_stage_started` (orchestrator.rs line 660). If `WorkerEvent::ToolCalled` arrives
   immediately after and re-inserts the worker, a `check_inactivity` poll in between sees no
   active stage and lets the Telegram typing indicator expire mid-tool-call.

## Requirements

### Req-1: Parse All Tool Calls (P1)

`parse_tool_calls()` in `crates/nv-daemon/src/claude.rs` (line 1183) MUST collect all
` ```tool_call ` blocks from the response, not just the first. Change the early-return after the
first match to a loop that advances the search offset past each consumed block. Return all
`ContentBlock::ToolUse` entries (plus any interleaved text blocks) in a single `Vec<ContentBlock>`.
If no tool call blocks are found, behaviour is unchanged (plain text response).

### Req-2: Fix Worker Queue Dequeue Race (P1)

In `WorkerPool::spawn()` in `crates/nv-daemon/src/worker.rs` (line 354), the decrement and the
subsequent dequeue must be atomic with respect to the concurrency gate. Restructure the dequeue
path so that `active.fetch_add(1)` for the next task happens inside the same lock scope as the
queue pop, before releasing the slot from the completed worker. This ensures no other thread can
observe `active < max_concurrent` and also attempt to pop between the sub and the add.

### Req-3: Queued-Worker Timeout Parity (P1)

The queued-worker timeout branch in `crates/nv-daemon/src/worker.rs` (line 386) MUST mirror the
primary worker timeout branch. On timeout it must:
- Emit `WorkerEvent::Error` via `deps.event_tx` with a descriptive message.
- Send a Telegram message to the originating chat (same `OutboundMessage` pattern used at lines
  325-348), using the `task_tg_chat_id` captured at task-spawn time.

### Req-4: Quiet Hours Respect User Timezone (P2)

`is_quiet_hours()` in `crates/nv-daemon/src/orchestrator.rs` (line 1775) MUST replace
`chrono::Local::now().time()` with the current time in `SharedDeps.timezone`. Use the existing
`tz_offset_seconds()` helper from `reminders.rs` (already `pub`) to convert UTC now to the user's
local `NaiveTime`. Pass `&deps.timezone` (or the resolved offset) to `is_quiet_hours()` — either
add a `tz_name: &str` parameter or compute the offset at the call sites in `Orchestrator`.

### Req-5: cmd_digest() Uses Configured Port (P2)

`cmd_digest()` in `crates/nv-daemon/src/orchestrator.rs` (line 1009) MUST NOT hardcode `8400`.
Add a `health_port: u16` field to `SharedDeps` in `worker.rs`, populated from
`config.daemon.health_port` (defaulting to 8400) in `main.rs`. `cmd_digest()` reads
`self.deps.health_port` to construct the URL. No config schema change required — the field already
exists in the config; this wires it through to the orchestrator.

### Req-6: StageComplete Defers Tracking Removal (P2)

`WorkerEvent::StageComplete` in `crates/nv-daemon/src/orchestrator.rs` (line 660) MUST NOT remove
the worker from `worker_stage_started` immediately. Instead, replace the remove with a short-lived
defer: schedule a removal that is cancelled if `WorkerEvent::ToolCalled` arrives first. The
simplest implementation is a `worker_stage_pending_removal: HashSet<Uuid>` — on `StageComplete`,
insert the worker ID; on `ToolCalled`, remove it from the pending set without removing the tracking
entry; on the next `check_inactivity` tick, remove entries that are still in the pending set.

## Scope
- **IN**: parse_tool_calls loop, dequeue atomicity, queued timeout parity, timezone-aware quiet
  hours, configured port in cmd_digest, deferred stage tracking removal
- **OUT**: Persistent streaming re-enablement, new tools, tool-call retry logic, health_port config
  schema changes

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/claude.rs` | parse_tool_calls collects all blocks in a loop |
| `crates/nv-daemon/src/worker.rs` | Dequeue race fix; queued timeout emits error + Telegram; add health_port to SharedDeps |
| `crates/nv-daemon/src/orchestrator.rs` | is_quiet_hours uses user tz; cmd_digest reads health_port; StageComplete defers removal |
| `crates/nv-daemon/src/main.rs` | Populate SharedDeps.health_port from config |

## Risks
| Risk | Mitigation |
|------|-----------|
| Multi-tool loop over-collects (non-tool JSON blocks tagged tool_call) | Rely on serde_json ToolCall parse failing for non-conforming blocks; only emit ToolUse on successful parse |
| Dequeue restructure deadlocks if queue lock is held during spawn | Use a scoped lock that drops before tokio::spawn; test with max_concurrent=1 |
| tz_offset_seconds uses a hand-rolled DST table (reminders.rs:32) | Acceptable for v1; same limitation already applies to reminder display |
