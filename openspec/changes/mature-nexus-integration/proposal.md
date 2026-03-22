# Proposal: Mature Nexus Integration

## Change ID
`mature-nexus-integration`

## Summary

Expand Nexus from read-only session queries to a full remote control surface: project-scoped
queries (`bd ready` per project), list open proposals per project, StartSession RPC to launch
Claude Code sessions from Telegram with confirmation, SendCommand RPC for remote `/apply`,
`/feature`, `/ci:gh`, and StopSession RPC for killing runaway sessions.

## Context
- Extends: `crates/nv-daemon/src/nexus/client.rs` (new RPCs), `crates/nv-daemon/src/nexus/tools.rs` (project-scoped queries), `crates/nv-daemon/src/tools.rs` (new tool definitions), `crates/nv-daemon/src/orchestrator.rs` (command dispatch)
- Related: PRD section 7.2, existing `NexusClient` with `query_sessions()` and `query_session()`, `nexus.proto` gRPC service, `scoped-bash-toolkit` (spec 4)
- Depends on: `add-scoped-bash-toolkit` (spec 4) — bd ready/proposals use scoped bash

## Motivation

Nexus currently only reports session status — Leo can ask "what sessions are running?" but cannot
start, control, or stop them from Telegram. This makes Nexus a status board, not a remote control.
Adding project-scoped queries and session lifecycle RPCs turns Telegram into a full Claude Code
control plane.

## Requirements

### Req-1: Project-Scoped Queries

Two new tools for querying project-level data via scoped bash:

- `nexus_project_ready(project_code)` — runs `bd ready` in the project directory, returns formatted task list
- `nexus_project_proposals(project_code)` — lists `openspec/changes/` directories in the project, returns proposal names and statuses

Both use the scoped bash toolkit (allowlisted read-only commands). No confirmation needed.

### Req-2: StartSession RPC

New gRPC RPC `StartSession(project, cwd, command)` dispatched to the Nexus agent managing
that project. Launches a new Claude Code session.

- Tool: `start_session(project, command)` — e.g., `start_session("oo", "/apply fix-chat-bugs")`
- Requires PendingAction confirmation via inline keyboard before execution
- Confirmation message: "Start CC session on OO: `/apply fix-chat-bugs`? [Approve] [Cancel]"
- On approve: call NexusClient.start_session(), report session ID back to Telegram

### Req-3: SendCommand RPC

New gRPC RPC `SendCommand(session_id, command)` to send a command to an existing session.

- Tool: `send_command(session_id, text)` — sends text input to a running session
- Use case: remote `/apply`, `/feature`, `/ci:gh` without SSH
- No confirmation needed for commands to already-running sessions (user already approved StartSession)

### Req-4: StopSession RPC

New gRPC RPC `StopSession(session_id)` to gracefully terminate a session.

- Tool: `stop_session(session_id)` — sends SIGTERM to the session process
- Requires PendingAction confirmation: "Stop session {id} on {project}? [Approve] [Cancel]"
- Use case: killing runaway sessions that are burning tokens

### Req-5: Proto Updates

Add to `nexus.proto`:

```protobuf
rpc StartSession(StartSessionRequest) returns (StartSessionResponse);
rpc SendCommand(SendCommandRequest) returns (SendCommandResponse);
rpc StopSession(StopSessionRequest) returns (StopSessionResponse);
```

Note: Proto changes require Nexus-side implementation too. Nova-side implements the client calls;
Nexus-side implementation is out of scope for this spec (tracked separately).

## Scope
- **IN**: Project-scoped queries (bd ready, proposals list), StartSession/SendCommand/StopSession client RPCs, tool definitions, PendingAction confirmation flow, proto message definitions
- **OUT**: Nexus server-side RPC implementation, session log streaming, multi-agent session orchestration, automatic session restart

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/nexus/client.rs` | Add start_session(), send_command(), stop_session() methods |
| `crates/nv-daemon/src/nexus/tools.rs` | Add project-scoped query formatters |
| `crates/nv-daemon/src/tools.rs` | Register 5 new tools (nexus_project_ready, nexus_project_proposals, start_session, send_command, stop_session) |
| `crates/nv-daemon/src/worker.rs` | Handle new tool calls in tool execution loop |
| `proto/nexus.proto` | Add 3 new RPCs + request/response messages |

## Risks
| Risk | Mitigation |
|------|-----------|
| Nexus agent doesn't implement new RPCs yet | Client returns clear error: "Agent does not support StartSession" — graceful degradation |
| StartSession burns tokens on accident | Requires explicit PendingAction confirmation before execution |
| StopSession kills mid-commit session | Confirmation message includes session status and elapsed time to inform the decision |
| Proto backward compatibility | New RPCs are additive — existing RPCs unchanged |
