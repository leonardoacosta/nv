# Implement Nexus gRPC Integration

| Field | Value |
|-------|-------|
| Spec | `nexus-integration` |
| Priority | P2 |
| Type | feature |
| Effort | medium |
| Wave | 5 |

## Context

NV needs real-time awareness of Claude Code sessions running across the homelab and MacBook. Nexus exposes a gRPC API on port 7400 for session management and event streaming. This spec connects NV to Nexus via tonic (Rust gRPC client), providing two capabilities: tools for the agent loop to query session state on demand, and a background event stream that pushes significant session lifecycle events into the agent's trigger channel.

The config supports multiple Nexus agents (e.g., homelab:7400, macbook:7400) with partial connectivity handling — if one agent is unreachable, the other still works. Session data feeds into digests (spec-7), query responses (spec-8), and standalone Telegram notifications for session completions and errors.

The proto definition lives at `~/dev/nexus/proto/nexus.proto`. The inclusion strategy is a checked-in copy under `proto/nexus.proto` in the NV repo, built at compile time via `tonic-build` in a `build.rs`. This avoids a git submodule dependency while keeping the proto definition explicit and auditable.

## User Stories

- **Success Criteria #4 (PRD 16)**: See session status from Nexus in the digest
- **Consumer (PRD 6.2)**: Digest includes "2 sessions running" with project names and durations
- **Architecture (PRD 7.1)**: Nexus gRPC integration on homelab:7400 and macbook:7400
- **PRD 10**: GetSessions, GetSession, StreamEvents, optional StartSession/StopSession

## Proposed Changes

### Proto Inclusion and Build

- `proto/nexus.proto`: Copy of Nexus protobuf definition from `~/dev/nexus/proto/nexus.proto`. Includes service definition with `GetSessions`, `GetSession`, `StreamEvents`, `StartSession`, `StopSession` RPCs. Message types for `Session`, `SessionEvent`, `SessionStatus`, etc.
- `crates/nv-daemon/build.rs`: `tonic-build` configuration — compiles `proto/nexus.proto` at build time, generates Rust types and client stubs in the `OUT_DIR`. Configured to generate only the client (no server needed).
- `crates/nv-daemon/Cargo.toml`: Add `tonic`, `prost`, `prost-types` to dependencies. Add `tonic-build` and `prost-build` to build-dependencies.

### NexusClient

- `crates/nv-daemon/src/nexus/client.rs`: `NexusClient` struct managing connections to multiple configured Nexus agents. Fields: `agents: Vec<NexusAgentConnection>` where each holds a `name: String`, `endpoint: String`, `client: Option<NexusServiceClient<Channel>>`, `status: ConnectionStatus`. Methods:
  - `new(config: &[NexusAgentConfig])` — creates client with configured agents, no connection yet
  - `connect_all()` — attempts to connect to each agent in parallel via `tokio::join!`. Each connection has a 10s timeout. Failed connections logged as warnings, not errors (partial connectivity is normal)
  - `query_sessions()` — calls `GetSessions` on all connected agents, merges results into a unified `Vec<SessionSummary>` sorted by start time. Failed agents return empty with a warning
  - `query_session(id: &str)` — calls `GetSession` on the agent that owns the session (determined by agent prefix in session ID, or tries all)
  - `is_connected(&self) -> bool` — true if at least one agent is connected

### Connection Management

- `crates/nv-daemon/src/nexus/connection.rs`: Per-agent connection lifecycle:
  - Initial connection with 10s timeout via `tonic::transport::Channel::from_shared(endpoint).connect_timeout(Duration::from_secs(10))`
  - Reconnect on disconnect — when an RPC call fails with transport error, mark agent as disconnected, spawn reconnect task with exponential backoff (1s, 2s, 4s, 8s, max 60s)
  - Health tracking: `last_seen: Option<DateTime<Utc>>`, `consecutive_failures: u32`, `status: enum { Connected, Disconnected, Reconnecting }`
  - On reconnect success, re-establish the `StreamEvents` subscription for that agent

### Agent Tools

- `crates/nv-daemon/src/nexus/tools.rs`: Tools exposed to the agent loop for Claude tool use:
  - `query_sessions()` — returns formatted session list: `[{agent_name}] {session_id}: {project} — {status} ({duration})`. Used in digest gathering and query responses
  - `query_session(id)` — returns detailed session info: project, status, command history, errors, duration, agent name. Used when Claude needs to investigate a specific session
  - Both tools handle partial connectivity gracefully — include a note about which agents are unreachable

### Background Event Stream

- `crates/nv-daemon/src/nexus/stream.rs`: `StreamEvents` subscription task — for each connected agent, spawns a tokio task that:
  1. Calls `StreamEvents` RPC to get a server-streaming response
  2. Iterates over the stream, filtering for significant events:
     - Session completion (success) → `Trigger::NexusEvent(SessionCompleted { id, project, duration })`
     - Session error → `Trigger::NexusEvent(SessionError { id, project, error_message })`
     - Session started (optional, lower priority) → logged but not triggered unless config enables it
  3. Pushes filtered events to the shared `mpsc::Sender<Trigger>` channel
  4. On stream disconnect, logs warning and triggers reconnect via connection manager
  - Events include the agent name for attribution in notifications

### Telegram Notifications

- `crates/nv-daemon/src/nexus/notify.rs`: Event-to-notification mapping in the agent loop:
  - **Session completed**: Telegram message with session summary — project, duration, agent. No action buttons needed (informational).
  - **Session error**: Telegram alert with error details and inline keyboard:
    - "View Error" → sends full error message as follow-up
    - "Retry" → (future: calls StartSession to re-run)
    - "Create Bug" → triggers Jira issue creation flow with error context pre-filled
  - Notification formatting respects quiet hours if configured (future — not in v1, just the hook point)

### Daemon Integration

- `crates/nv-daemon/src/nexus/mod.rs`: Module declaration for client, connection, tools, stream, notify submodules.
- `crates/nv-daemon/src/main.rs`: Wire NexusClient — create client from config, call `connect_all()`, spawn stream tasks, pass client to agent loop for tool access.
- `crates/nv-daemon/src/agent_loop.rs`: Handle `Trigger::NexusEvent` variants — route to notification formatter, include session context in Claude calls.
- `crates/nv-core/src/types.rs`: Add `NexusEvent` variants to the `Trigger` enum if not already present. Add `SessionSummary`, `SessionEvent` types.

### Backfill Existing Specs

- `crates/nv-daemon/src/digest/gather.rs`: Replace Nexus stub (spec-7 task 3.4) with actual `nexus_client.query_sessions()` call. Sessions section in digest now shows real data.
- `crates/nv-daemon/src/query/gather.rs`: Replace Nexus stub (spec-8 task 2.5) with actual `nexus_client.query_sessions()` filtered by relevant project.

## Dependencies

- `agent-loop` (spec-4) — mpsc trigger channel and agent loop infrastructure
- Nexus running on at least one configured agent (homelab or macbook)

## Out of Scope

- StartSession / StopSession RPCs (read-only integration for v1 — session control deferred)
- Git submodule for proto (copy strategy chosen for simplicity)
- Proto schema evolution handling (pin to current version, update manually when Nexus proto changes)
- Multi-tenant session filtering (NV is single-user, all sessions are Leo's)
- Session log streaming (only events, not full session output)
