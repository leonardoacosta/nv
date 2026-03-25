# Proposal: Fix Nexus Session Deduplication

## Change ID
`fix-nexus-session-dedup`

## Summary

Before launching any session via `start_session`, query Nexus for existing active sessions on
that project and skip the launch if one already exists. Prevents duplicate session storms when
batch-approvals arrive faster than Nexus registers previous launches.

## Context
- Extends: `crates/nv-daemon/src/callbacks.rs` (`execute_nexus_start_session`)
- Extends: `crates/nv-daemon/src/nexus/client.rs` (new `has_active_session_for_project` helper)
- May extend: `crates/nv-daemon/src/nexus/tools.rs` (if query helper is placed there)
- Related: proto `SessionFilter.project` field already exists and is wired into `GetSessions` RPC

## Motivation

When Nova batch-approves multiple actions (e.g., 3 approvals x 4 projects), each approved
`NexusStartSession` action is dispatched independently in `execute_nexus_start_session`. If
the approvals fire in quick succession, Nexus has not yet registered the first launch before
the second callback queries for existing sessions — or no check happens at all. Result: 8 real
sessions launched plus 2 crash loops observed in production.

The proto `SessionFilter` message already supports filtering by `project` (field 2). The
`NexusClient::query_sessions` path already iterates all agents and merges results. The missing
piece is a pre-launch guard that calls `GetSessions` with a project filter, checks whether any
returned session is `active` or `idle`, and aborts the launch if so.

## Requirements

### Req-1: `has_active_session_for_project` on `NexusClient`

Add an async method to `NexusClient` in `nexus/client.rs`:

```rust
pub async fn has_active_session_for_project(&self, project: &str) -> bool
```

- Calls `GetSessions` with `SessionFilter { project: Some(project.to_string()), status: None, session_type: None }` on each connected agent.
- Returns `true` if any session has status `active` or `idle` (treat `idle` as still-alive; stale
  and errored do not block).
- On RPC failure, logs a warning and continues to the next agent — a failed query must not
  silently block a legitimate launch. If all agents fail, returns `false` (fail-open: prefer a
  duplicate over a missed launch).
- Does not mutate connection state beyond updating `last_seen` on success.

### Req-2: Pre-launch guard in `execute_nexus_start_session`

In `callbacks.rs`, before calling `client.start_session(...)`, call
`has_active_session_for_project`. If it returns `true`, return early with an informational
`Ok(String)` rather than an error — the action is "done" in the sense that the session is
already running.

Return message format: `"Session already active for {project} — launch skipped"`

This surfaces in Telegram via the existing "Done: ..." edit path so Leo knows the dedup fired.

### Req-3: Structured log at dedup site

When a launch is skipped, emit a `tracing::info!` with fields:
- `project`
- `"dedup"` = `true`

This makes it greppable in systemd journal without requiring a separate metrics layer.

## Scope
- **IN**: pre-launch query, active/idle status check, early-return with informational message
- **OUT**: distributed locking, in-process cooldown timers, changes to the Nexus agent itself,
  changes to the approval/confirmation UI

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/nexus/client.rs` | Add `has_active_session_for_project` async method |
| `crates/nv-daemon/src/callbacks.rs` | Add pre-launch guard in `execute_nexus_start_session` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Nexus hasn't registered the just-launched session by the time the next callback queries | This is the race we're solving — `GetSessions` reflects Nexus agent state, which is authoritative. The first callback updates agent state before returning; subsequent callbacks query the same agent state. |
| Query adds latency to every batch-approved session launch | `GetSessions` with a project filter is a cheap gRPC call (<50ms on LAN). Acceptable. |
| Idle sessions block legitimate re-launches | `idle` means the session process is alive and attached. If Leo wants to force a relaunch, he can `stop_session` first. This is correct behavior. |
| All agents unreachable at launch time | Fail-open: `has_active_session_for_project` returns `false`, launch proceeds. Matches current behavior. |
