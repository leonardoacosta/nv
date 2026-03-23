# Proposal: Sync Nexus Proto and Add Missing Tools

## Change ID
`sync-nexus-proto`

## Summary
Update nv's nexus.proto to match upstream (~/dev/nx), use the new EventFilter fields for
server-side event filtering, and expose 4 missing Nexus capabilities as Claude tools.

## Context
- Extends: `proto/nexus.proto`, `crates/nv-daemon/src/nexus/stream.rs`, `crates/nv-daemon/src/nexus/client.rs`, `crates/nv-daemon/src/tools/mod.rs`
- Related: StreamEvents already fully implemented in `nexus/stream.rs` with reconnection and watchdog respawn. This change is incremental.

## Motivation
Nexus shipped 3 proto enhancements (EventType enum, filter fields, agent_name on events,
is_snapshot flag). Nova's proto is stale — it subscribes unfiltered, receiving heartbeat noise
that gets discarded in Rust. The new fields enable server-side filtering and bootstrap snapshots.
Additionally, 4 Nexus RPCs have no Claude tool surface: GetHealth, ListProjects, agent-targeted
StartSession, and status_summary.

## Requirements

### Req-1: Proto Sync
Copy upstream nexus.proto, regenerate Rust types. New additions:
- `EventType` enum (5 values)
- `EventFilter.event_types` (repeated EventType)
- `EventFilter.initial_snapshot` (bool)
- `SessionEvent.agent_name` (string, field 7)
- `SessionStarted.is_snapshot` (bool)

### Req-2: Server-Side Event Filtering
Update `stream.rs` to pass `event_types: [STATUS_CHANGED, SESSION_STOPPED]` and
`initial_snapshot: true` in the EventFilter. Handle `is_snapshot` flag to distinguish
bootstrap replays from real session starts.

### Req-3: Agent-Targeted Start Session
Add optional `agent` parameter to `start_session` tool. When specified, skip round-robin
and only try the named agent.

### Req-4: New Tool Definitions
Expose these existing/trivial RPCs as Claude tools:
- `query_nexus_health` — calls GetHealth, returns machine stats per agent
- `query_nexus_projects` — calls ListProjects, returns available projects per agent
- `query_nexus_agents` — wraps existing `status_summary()`, returns connection status

## Scope
- **IN**: Proto sync, event filter usage, 4 tool additions, agent-targeted start
- **OUT**: New RPCs on the Nexus side (upstream is done), watchdog changes, notification changes

## Impact
| Area | Change |
|------|--------|
| `proto/nexus.proto` | Sync with upstream — additive fields only |
| `nexus/stream.rs` | Use new EventFilter fields (2 lines) |
| `nexus/client.rs` | Add `get_health()`, `list_projects()`, modify `start_session()` |
| `tools/mod.rs` | 3 new tool definitions + agent param on start_session |

## Risks
| Risk | Mitigation |
|------|-----------|
| Proto field numbers change | Proto changes are additive (new fields), no breaking renumbers |
| Generated code drift | Copy proto verbatim from upstream, don't hand-edit |
| Heartbeat filter breaks watchdog | Watchdog uses GetHealth RPC, not StreamEvents — independent |
