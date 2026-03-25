# Proposal: Update Session Lifecycle

## Change ID
`update-session-lifecycle`

## Summary

Replace Nexus-backed session management in the daemon with CC team agent coordination. Session
start, stop, monitor, and list operations are reimplemented using the CC SDK's native agent API.
Telegram commands (`/sessions`, `/start <project>`, `/stop <project>`) and the dashboard session
list page reflect CC agent status rather than Nexus gRPC data.

## Context
- Phase: Wave 4c — depends on `remove-nexus-crate`
- Extends: `crates/nv-daemon/src/callbacks.rs` (start/stop callback executors), `crates/nv-daemon/src/orchestrator.rs` (session tracking, NexusEvent routing), `crates/nv-daemon/src/state.rs` (SessionErrorMeta), `crates/nv-daemon/src/nexus/tools.rs` (session query tools)
- Related: `cc-session-management` spec (dashboard CC session container), `add-anthropic-api-client` spec (direct API path), `add-telegram-commands` spec (BotCommand routing already wired)
- Replaces: `NexusClient::start_session`, `NexusClient::stop_session`, `NexusClient::query_sessions`, Nexus gRPC session event stream for lifecycle events

## Motivation

Nexus was the session management layer: it launched tmux-based CC sessions over gRPC and streamed
lifecycle events back to the daemon. With Nexus removed (`remove-nexus-crate`), all session
operations that referenced `NexusClient` become dead code — `execute_nexus_start_session`,
`execute_nexus_stop_session`, `TriggerClass::NexusEvent`, and `SessionErrorMeta` (which stores
Nexus session crash context).

The replacement uses CC's built-in team agent coordination:

1. **No gRPC dependency** — CC agents are spawned via the CC SDK's subprocess/API path, not
   through a separate server
2. **Native lifecycle hooks** — the CC agent SDK exposes agent status, health, and output
   without requiring a sidecar process
3. **Crash recovery** — an agent's exit can be detected via its process handle; restart is a
   re-spawn with the same project context
4. **Consistent model** — the daemon already manages a CC session container (Wave 2b
   `cc-session-management`); this spec extends the same pattern to project-scoped agents

## Requirements

### Req-1: CC Agent Session Manager

Add `CcSessionManager` to `crates/nv-daemon/src/cc_sessions.rs`. This module owns the in-process
registry of active CC project agents.

```
CcSessionManager {
    sessions: HashMap<SessionId, CcSessionHandle>
}

CcSessionHandle {
    id: SessionId,             // uuid
    project: String,           // e.g. "oo"
    cwd: PathBuf,
    command: Option<String>,   // e.g. "/apply fix-auth"
    started_at: DateTime<Utc>,
    state: CcSessionState,     // Active | Idle | Stopping | Error
    restart_count: u32,
    last_health_at: DateTime<Utc>,
}

CcSessionState: Active | Idle | Stopping | Error(String)
```

`CcSessionManager` is thread-safe via `Arc<Mutex<>>` and stored in `SharedDeps`. It replaces
`NexusClient` for session lifecycle purposes.

### Req-2: Session Start

`CcSessionManager::start_session(project, cwd, command, agent_name)`:

1. Dedup guard: return early if an `Active` or `Idle` session for `project` already exists.
2. Spawn CC agent as a subprocess: `claude --project <cwd> <command>` (or equivalent CC SDK
   invocation). Capture the process handle for health tracking.
3. Insert `CcSessionHandle` into the registry with state `Active`.
4. Return `SessionId` (UUID) and a display string (e.g. `"oo — /apply fix-auth"`).

Agent name is stored in the handle for display but does not affect spawning. The `cwd` fallback
path (`$HOME/dev/<project>`) is preserved from the Nexus implementation.

### Req-3: Session Stop

`CcSessionManager::stop_session(session_id)`:

1. Look up session by ID.
2. Set state to `Stopping`.
3. Send SIGTERM to the CC subprocess. Wait up to 5s for graceful exit.
4. If still running after 5s, send SIGKILL.
5. Remove session from the registry.
6. Return a result string (e.g. `"Session oo stopped"`).

### Req-4: Session Health Monitor

A background task in `CcSessionManager` runs every 30 seconds:

- For each `Active` or `Idle` session: check if the subprocess is still alive (non-blocking
  `try_wait` on the process handle).
- If the process has exited unexpectedly (state was not `Stopping`): trigger auto-restart
  (see Req-5).
- Update `last_health_at` on each check.

Health state is readable via `CcSessionManager::get_status(session_id)` for Telegram and
dashboard use.

### Req-5: Auto-Restart on Agent Crash

When the health monitor detects an unexpected exit:

1. Increment `restart_count`.
2. If `restart_count <= 3`: re-spawn the agent with the same `project`, `cwd`, and `command`.
   Log the restart as a warning with project and restart count.
3. If `restart_count > 3`: set state to `Error("too many restarts")`. Send a Telegram
   notification: `"Session <project> has crashed 3 times and will not auto-restart. Use
   /start <project> to retry manually."`
4. `SessionErrorMeta` in `state.rs` is updated to use `CcSessionHandle` fields instead of
   Nexus fields (`session_id`, `project`, `cwd`, `command`, `error_message`, `agent_name`).
   The struct shape is preserved; only the source of truth changes.

### Req-6: Session List

`CcSessionManager::list_sessions()` returns `Vec<CcSessionSummary>`:

```
CcSessionSummary {
    id: SessionId,
    project: String,
    state: CcSessionState,
    started_at: DateTime<Utc>,
    duration_display: String,   // "2h 14m"
    restart_count: u32,
    command: Option<String>,
}
```

This replaces `NexusClient::query_sessions()` as the data source for session listing. The
`SessionSummary` type previously used for Nexus output is superseded; callers are updated to
use `CcSessionSummary` directly.

### Req-7: Telegram Commands

Wire three new bot commands into the existing `handle_command` dispatcher in
`crates/nv-daemon/src/orchestrator.rs`:

| Command | Handler | Output |
|---------|---------|--------|
| `/sessions` | `CcSessionManager::list_sessions()` | Table: project, state, duration, restart count |
| `/start <project>` | `CcSessionManager::start_session(project, ...)` | "Session started: <id>" or "Already active" |
| `/stop <project>` | Find active session by project, call `stop_session(id)` | "Session <project> stopped" or "No active session" |

`/start` without a project argument returns usage: `"Usage: /start <project> [command]"`.
`/stop` without a project argument returns the same session list as `/sessions`.
`/start <project> <command>` passes the remainder of args as the CC command string.

All three commands bypass the worker pool (same pattern as existing BotCommands).

### Req-8: Dashboard Session List Integration

The session list endpoint used by the dashboard (`apps/dashboard/src/app/api/session/status/route.ts`)
is extended to include project sessions from `CcSessionManager`. The daemon exposes a new
lightweight HTTP endpoint (or the existing `DashboardClient` push mechanism is extended) so
the dashboard can show:

- The primary CC session container status (existing, from `cc-session-management`)
- Each active project session: project name, state, duration, restart count

The dashboard session page gains a "Project Sessions" section beneath the main container status.
This section polls `/api/session/projects` (new route) every 15s and renders a table row per
active project session with state badge, duration, and a Stop button.

## Scope
- **IN**: `CcSessionManager` module, session start/stop/monitor/list, auto-restart, crash
  notification, three Telegram commands, dashboard project session list endpoint and UI section
- **OUT**: CC team agent `isolation: "worktree"` (that is an orchestrator-level concern, not
  daemon lifecycle), per-session log streaming to Telegram, session pause/resume, session
  migration between hosts

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/cc_sessions.rs` | New: `CcSessionManager`, `CcSessionHandle`, `CcSessionState`, `CcSessionSummary` |
| `crates/nv-daemon/src/callbacks.rs` | Replace `execute_nexus_start_session` / `execute_nexus_stop_session` with `CcSessionManager` calls; remove `nexus_client` parameter from `handle_approve` |
| `crates/nv-daemon/src/orchestrator.rs` | Remove `TriggerClass::NexusEvent` routing; add `/sessions`, `/start`, `/stop` to `handle_command`; add `CcSessionManager` to `SharedDeps` |
| `crates/nv-daemon/src/state.rs` | Update `SessionErrorMeta` field sourcing from Nexus to `CcSessionHandle`; remove Nexus-specific comments |
| `crates/nv-daemon/src/worker.rs` | Remove `nexus_client` from `SharedDeps`; add `cc_sessions: Arc<CcSessionManager>` |
| `apps/dashboard/src/app/api/session/projects/route.ts` | New: returns `CcSessionSummary[]` from daemon via HTTP or in-process if co-located |
| `apps/dashboard/src/app/session/page.tsx` | Add "Project Sessions" section using new projects endpoint |
| `apps/dashboard/src/components/ProjectSessionsTable.tsx` | New: table component with state badge, duration, Stop button per row |

## Risks
| Risk | Mitigation |
|------|-----------|
| CC subprocess spawning differs per platform (macOS vs Linux) | Use `std::process::Command` with `claude` binary on PATH; same approach as existing `claude.rs` worker |
| Process handle lost if daemon restarts | On startup, `CcSessionManager` starts empty — orphaned CC subprocesses are not re-adopted. This is acceptable for Wave 4c; session persistence across restarts is out of scope |
| `/start` without existing project registry entry | Fallback to `$HOME/dev/<project>` (same logic as Nexus `execute_nexus_start_session`) |
| Dashboard project endpoint unavailable if daemon not co-located | Dashboard gracefully hides the "Project Sessions" section when the endpoint returns 503 |
| Auto-restart loop consuming credits | Restart cap at 3 per session; Error state requires manual `/start` to reset |
