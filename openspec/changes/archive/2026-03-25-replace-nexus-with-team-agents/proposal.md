# Proposal: Replace Nexus with CC Team Agent Coordination

## Change ID
`replace-nexus-with-team-agents`

## Summary

Replace the custom Nexus gRPC-based remote session management with Claude Code's native team agent
coordination. Instead of launching CC sessions on remote machines via gRPC calls to a nexus-register
binary, NV dispatches CC team agents directly — eliminating the Nexus sidecar entirely.

## Context
- Phase: Wave 4a — Nexus Deprecation
- Related beads: nv-3vw (deprecate-nv-daemon — the wave epic)
- Depends on: nothing (independent of Wave 2/3 specs)
- Depended on by: `remove-nexus-crate` (cannot be implemented until this spec is complete)
- Extends: `crates/nv-daemon/src/nexus/` (full module replacement), `crates/nv-daemon/src/callbacks.rs`
- Replaces: gRPC-over-tonic calls to `NexusClient` with CC SDK subprocess dispatch

## Motivation

Nexus is a bespoke daemon (`nexus-register`) that runs on every remote machine and implements a
gRPC server for session management. This creates a maintenance burden: the binary must be deployed
and kept running on each remote host, the proto definition must be kept in sync, and the watchdog,
connection pool, and event stream add thousands of lines of code to the daemon.

Claude Code ships native team agent support (`isolation: "worktree"` + `Task` tool dispatch). CC
agents can be launched on any configured machine via the CC CLI — no extra daemon required. The
CC SDK itself handles the session lifecycle.

This spec replaces `NexusClient` and all gRPC machinery with a new `TeamAgentDispatcher` that
wraps CC subprocess dispatch. The Telegram UX is preserved: the user sees the same
approve/start/stop flow.

Key wins:
1. **No sidecar binary** — remote machines need only the CC CLI installed, not nexus-register
2. **Native CC lifecycle** — session start, stop, and status come from CC itself, not a custom protocol
3. **Fewer moving parts** — ~1,200 LOC of nexus module deleted in the follow-on `remove-nexus-crate` spec
4. **Feature flag** — gradual migration: existing Nexus config keeps working until removed

## Requirements

### Req-1: TeamAgentDispatcher

New module `crates/nv-daemon/src/team_agent/` with a `TeamAgentDispatcher` struct that provides
the same interface as `NexusClient` for the operations NV actually uses:

| NexusClient method | TeamAgentDispatcher equivalent |
|---|---|
| `start_session(project, cwd, args, agent)` | `start_agent(project, cwd, command, machine)` |
| `stop_session(session_id)` | `stop_agent(session_id)` |
| `query_sessions()` | `list_agents()` |
| `query_session(id)` | `get_agent(id)` |
| `has_active_session_for_project(project)` | `has_active_agent_for_project(project)` |
| `is_connected()` | `is_available()` — always true if CC CLI is present |

`TeamAgentDispatcher` spawns the CC CLI as a subprocess using `tokio::process::Command`:

```
claude --output-format stream-json --print --dangerously-skip-permissions \
  --cwd <cwd> \
  --model <model> \
  <command>
```

Session state is tracked in an in-memory `HashMap<String, AgentSession>` keyed by a generated
session ID (`ta-{uuid}`). Each `AgentSession` holds: `id`, `project`, `cwd`, `command`,
`machine` (for display), `status` (`active | idle | errored | stopped`), `started_at`, `task_handle`
(the tokio `JoinHandle`).

### Req-2: Session Lifecycle Semantics

**Start:** `start_agent` generates a `ta-{uuid}` session ID, spawns the CC subprocess (or logs
a warning and returns an error if the CC CLI is not reachable on the target machine), adds the
session to the in-memory map, returns `(session_id, "cc-native")`. The second return value
mirrors the `tmux_session` return of `NexusClient::start_session` — callers that display the
tmux session name will show `"cc-native"` instead.

**Stop:** `stop_agent` looks up the session by ID, sends `SIGTERM` to the subprocess, awaits
up to 10s for clean exit, falls back to `SIGKILL`, marks the session `stopped`.

**Status query:** `list_agents` returns all non-`stopped` sessions from the in-memory map as
`Vec<SessionSummary>`. `get_agent(id)` returns `Option<SessionDetail>`. Both use the same
`SessionSummary` / `SessionDetail` types already defined in `nexus::client` — no type changes
needed.

**Completion detection:** When the CC subprocess exits, a background watcher task transitions
the session status to `idle` (exit 0) or `errored` (non-zero exit) and emits a `Trigger::NexusEvent`
with the appropriate `SessionEventType` — exactly as the gRPC event stream does today. The
existing orchestrator notification path (`Trigger::NexusEvent`) is unchanged.

### Req-3: Machine Routing

NV currently routes sessions to specific Nexus agents by name (`agent: Option<&str>` in
`start_session`). The replacement maps `machine` to a configured set of hosts:

```toml
[team_agents]
enabled = true

[[team_agents.machines]]
name = "homelab"
ssh_host = "omarchy"       # or empty string for local
cc_path = "/home/leo/.local/bin/claude"

[[team_agents.machines]]
name = "macbook"
ssh_host = ""              # empty = local
cc_path = "/usr/local/bin/claude"
```

When `ssh_host` is non-empty, the subprocess is wrapped in an SSH command:
`ssh <ssh_host> <cc_path> <args>`. When empty, the CC binary is invoked directly.

This mirrors the Nexus agent config structure in `nv_core::config::NexusConfig` but uses SSH
rather than gRPC.

`TeamAgentConfig` and `TeamAgentsConfig` are new types in `crates/nv-core/src/config.rs`.

### Req-4: Feature Flag

Both `NexusClient` and `TeamAgentDispatcher` remain available simultaneously. Selection is
controlled by a config flag:

```toml
[nexus]
agents = [...]           # existing, unchanged
use_team_agents = false  # default: false (stays on Nexus)

[team_agents]
enabled = true           # if true AND nexus.use_team_agents = true, team agent path is used
```

The `SharedDeps` struct in `worker.rs` gains an `Option<TeamAgentDispatcher>` field alongside
the existing `Option<NexusClient>`. Tool dispatch in `tools/mod.rs` checks the flag and routes
to the appropriate implementation.

The `callbacks.rs` approve handler for `NexusStartSession` / `NexusStopSession` is updated to
check the flag and call the appropriate backend. The `ActionType` variants are reused — no
Telegram UX changes.

### Req-5: Session Dedup Guard

`has_active_agent_for_project` mirrors `NexusClient::has_active_session_for_project`: it scans
the in-memory session map for any `active` or `idle` session with a matching project. This
prevents duplicate launches on batch-approvals (existing behaviour in `callbacks.rs` line 130).

### Req-6: Health and Observability

`is_available()` performs a cheap availability check: verifies the CC binary is reachable at the
configured path (local) or via SSH (remote). Returns `true` on success.

`TeamAgentDispatcher` exposes an `agent_details()` method returning `Vec<(name, path_or_host,
available, last_used)>` — same shape as `NexusClient::agent_details()` so the dashboard and
health poller do not need per-backend changes for basic status display.

The existing `HealthState` channel registration (`nexus_{agent_name}`) is preserved — the team
agent backend registers channels as `ta_{machine_name}` so the health dashboard continues to
show per-machine status without code changes in the health layer.

### Req-7: Tool Registration Compatibility

The existing tool names exposed to Claude — `query_nexus`, `query_session`, `start_nexus_session`,
`stop_nexus_session`, `query_nexus_health`, `query_nexus_agents`, `query_nexus_projects` — are
preserved. The tool dispatch layer in `tools/mod.rs` routes to `TeamAgentDispatcher` or
`NexusClient` based on the feature flag. Claude's tool interface does not change.

## Scope

**IN:**
- `crates/nv-daemon/src/team_agent/` module — new dispatcher, config, session tracking
- `crates/nv-core/src/config.rs` — `TeamAgentConfig`, `TeamAgentsConfig` types
- `crates/nv-daemon/src/worker.rs` — add `Option<TeamAgentDispatcher>` to `SharedDeps`
- `crates/nv-daemon/src/callbacks.rs` — route `NexusStartSession`/`NexusStopSession` to dispatcher
- `crates/nv-daemon/src/tools/mod.rs` — tool dispatch routing by feature flag
- `crates/nv-daemon/src/main.rs` — initialize `TeamAgentDispatcher` if configured
- `crates/nv-daemon/src/orchestrator.rs` — thread `TeamAgentDispatcher` through to tool calls
- Unit tests for all new types and session lifecycle transitions

**OUT:**
- Removal of `crates/nv-daemon/src/nexus/` — that is the follow-on `remove-nexus-crate` spec
- Removal of `proto/nexus.proto` — deferred to `remove-nexus-crate`
- Dashboard UI changes for team agent sessions — deferred
- Streaming session output via CC stream-json — read-only session status only in v1
- Multi-concurrent sessions per project — one active session per project enforced by dedup guard
- Full CC SDK integration — subprocess CLI is sufficient; SDK migration is a separate concern

## Impact

| Area | Change |
|------|--------|
| `crates/nv-core/src/config.rs` | Add `TeamAgentConfig`, `TeamAgentsConfig` |
| `crates/nv-daemon/src/team_agent/mod.rs` | New module |
| `crates/nv-daemon/src/team_agent/dispatcher.rs` | `TeamAgentDispatcher` struct and session lifecycle |
| `crates/nv-daemon/src/team_agent/session.rs` | `AgentSession`, `AgentStatus`, in-memory session map |
| `crates/nv-daemon/src/team_agent/watcher.rs` | Background task: subprocess exit → session status + Trigger |
| `crates/nv-daemon/src/worker.rs` | Add `Option<TeamAgentDispatcher>` to `SharedDeps` |
| `crates/nv-daemon/src/callbacks.rs` | Route start/stop actions to dispatcher or nexus via flag |
| `crates/nv-daemon/src/tools/mod.rs` | Tool dispatch routing by `use_team_agents` flag |
| `crates/nv-daemon/src/main.rs` | Initialize and wire `TeamAgentDispatcher` |
| `crates/nv-daemon/src/orchestrator.rs` | Thread dispatcher ref through to tool calls |
| `crates/nv-core/src/config.rs` | Add `use_team_agents: bool` to `NexusConfig` |

## Risks

| Risk | Mitigation |
|------|-----------|
| CC subprocess SSH sessions outlive the daemon | Stop all active sessions on daemon shutdown signal; SIGTERM then SIGKILL after 10s |
| SSH key auth not set up on remote machine | `is_available()` detects this on startup; warning logged, tool returns error |
| In-memory session state lost on daemon restart | Sessions are transient; same behaviour as today (Nexus sessions are not persisted either) |
| Feature flag confusion (both backends configured) | Only one path executes per flag value; nexus client is `None` when team_agents is active |
| Session ID format change (`ta-` prefix) | Session IDs are opaque strings in the UX; no stored references that would break |
