# Implementation Tasks

<!-- beads:epic:nv-3vw -->

## Batch 1: Config Types

- [ ] [1.1] [P-1] Add `TeamAgentMachine` struct to `crates/nv-core/src/config.rs` — fields: `name: String`, `ssh_host: String` (empty = local), `cc_path: String` (path to CC binary on the target machine). Derive `Debug, Clone, Deserialize` [owner:api-engineer]
- [ ] [1.2] [P-1] Add `TeamAgentsConfig` struct to `crates/nv-core/src/config.rs` — fields: `enabled: bool`, `machines: Vec<TeamAgentMachine>`. Derive `Debug, Clone, Deserialize`. Add `team_agents: Option<TeamAgentsConfig>` field to `NvConfig` [owner:api-engineer]
- [ ] [1.3] [P-1] Add `use_team_agents: bool` field (default `false`) to `NexusConfig` in `crates/nv-core/src/config.rs` — the flag that switches dispatch from Nexus to team agents. Annotate with `#[serde(default)]` [owner:api-engineer]
- [ ] [1.4] [P-2] Unit tests for new config types — deserialize a TOML fixture with `team_agents` section, verify all fields present; verify missing `use_team_agents` defaults to false [owner:api-engineer]

## Batch 2: Session State Types

- [ ] [2.1] [P-1] Create `crates/nv-daemon/src/team_agent/mod.rs` — module declarations for `dispatcher`, `session`, `watcher` submodules; re-export `TeamAgentDispatcher` [owner:api-engineer]
- [ ] [2.2] [P-1] Create `crates/nv-daemon/src/team_agent/session.rs` — define `AgentStatus` enum (`Active`, `Idle`, `Errored`, `Stopped`). Define `AgentSession` struct: `id: String`, `project: String`, `cwd: String`, `command: String`, `machine: String`, `status: AgentStatus`, `started_at: DateTime<Utc>`, `process_id: Option<u32>`. Implement `to_session_summary()` and `to_session_detail()` methods that return the existing `nexus::client::SessionSummary` / `nexus::client::SessionDetail` types [owner:api-engineer]
- [ ] [2.3] [P-2] Unit tests for session type conversions — `AgentSession::to_session_summary()` maps fields correctly; `to_session_detail()` sets `session_type = "cc-native"` and `agent_name` from `machine` field [owner:api-engineer]

## Batch 3: TeamAgentDispatcher Core

- [ ] [3.1] [P-1] Create `crates/nv-daemon/src/team_agent/dispatcher.rs` — `TeamAgentDispatcher` struct with `machines: Vec<TeamAgentMachine>`, `sessions: Arc<Mutex<HashMap<String, AgentSession>>>`. Implement `new(config: &TeamAgentsConfig) -> Self` constructor [owner:api-engineer]
- [ ] [3.2] [P-1] Implement `TeamAgentDispatcher::start_agent(project, cwd, command, machine_name: Option<&str>) -> Result<(String, String)>` — generates `ta-{uuid}` session ID, resolves target machine (by name if provided, else first available), builds the subprocess command (direct or `ssh <host> <cc_path> <args>`), spawns via `tokio::process::Command::new`, inserts `AgentSession` into map with `status = Active`, returns `(session_id, "cc-native")` [owner:api-engineer]
- [ ] [3.3] [P-1] Implement `TeamAgentDispatcher::stop_agent(session_id: &str) -> Result<String>` — looks up session by ID, sends `SIGTERM` to the child process via `tokio::signal::unix` or `Child::kill()`, awaits up to 10s, falls back to `SIGKILL`, marks session `Stopped`, returns confirmation message [owner:api-engineer]
- [ ] [3.4] [P-1] Implement `TeamAgentDispatcher::list_agents() -> Result<Vec<SessionSummary>>` — returns all non-`Stopped` sessions from the map as `Vec<SessionSummary>` sorted by `started_at` descending. Uses `AgentSession::to_session_summary()` [owner:api-engineer]
- [ ] [3.5] [P-1] Implement `TeamAgentDispatcher::get_agent(id: &str) -> Result<Option<SessionDetail>>` — looks up session by ID, returns `Some(to_session_detail())` or `None` [owner:api-engineer]
- [ ] [3.6] [P-1] Implement `TeamAgentDispatcher::has_active_agent_for_project(project: &str) -> bool` — scans in-memory map for any session in `Active` or `Idle` status with matching project. Mirrors `NexusClient::has_active_session_for_project` dedup semantics exactly [owner:api-engineer]
- [ ] [3.7] [P-2] Implement `TeamAgentDispatcher::is_available() -> bool` — for each configured machine, checks CC binary is present: local = `std::path::Path::new(&machine.cc_path).exists()`, remote = `ssh <host> test -f <cc_path>` with 5s timeout. Returns `true` if at least one machine passes [owner:api-engineer]
- [ ] [3.8] [P-2] Implement `TeamAgentDispatcher::agent_details() -> Vec<(String, String, bool, Option<DateTime<Utc>>)>` — returns `(name, ssh_host_or_local, available, last_used)` for each machine. `last_used` is the `started_at` of the most recent session for that machine [owner:api-engineer]
- [ ] [3.9] [P-2] Implement `TeamAgentDispatcher::status_summary() -> Vec<(String, String)>` — returns `(machine_name, "available"|"unavailable")` for each configured machine, analogous to `NexusClient::status_summary()` [owner:api-engineer]

## Batch 4: Background Session Watcher

- [ ] [4.1] [P-1] Create `crates/nv-daemon/src/team_agent/watcher.rs` — `watch_session(session_id: String, sessions: Arc<Mutex<HashMap<String, AgentSession>>>, child: tokio::process::Child, trigger_tx: mpsc::UnboundedSender<Trigger>)` async fn. Awaits child exit, determines exit code, transitions session status to `Idle` (exit 0) or `Errored` (non-zero), emits `Trigger::NexusEvent(SessionEvent { agent_name, session_id, event_type: Completed|Failed, details })` [owner:api-engineer]
- [ ] [4.2] [P-1] Spawn watcher task in `start_agent` — after successful subprocess spawn, call `tokio::spawn(watch_session(...))` with a clone of the sessions `Arc` and the child handle. The child `Child` is moved into the watcher; the session map entry stores only `process_id` (from `child.id()`) [owner:api-engineer]
- [ ] [4.3] [P-2] Unit tests for watcher exit-code mapping — `exit 0` → `SessionEventType::Completed`; `exit 1` → `SessionEventType::Failed`; details field contains exit code for non-zero exits [owner:api-engineer]

## Batch 5: Wire into Daemon

- [ ] [5.1] [P-1] Add `Option<TeamAgentDispatcher>` field to `SharedDeps` in `crates/nv-daemon/src/worker.rs`. Initialise to `None` alongside existing `nexus_client` [owner:api-engineer]
- [ ] [5.2] [P-1] In `crates/nv-daemon/src/main.rs`: after the nexus client init block, check if `config.nexus.as_ref().map(|n| n.use_team_agents).unwrap_or(false)` and `config.team_agents` is `Some`. If both true, create and wire `TeamAgentDispatcher`; set `nexus_client = None` for the SharedDeps (team agents replaces nexus, not extends). Pass dispatcher to `SharedDeps::team_agent_dispatcher` [owner:api-engineer]
- [ ] [5.3] [P-1] In `crates/nv-daemon/src/orchestrator.rs`: thread `Option<&TeamAgentDispatcher>` ref through the call sites that currently pass `nexus_client` to tool execution. The dispatcher ref is taken from `SharedDeps` [owner:api-engineer]

## Batch 6: Tool Dispatch Routing

- [ ] [6.1] [P-1] In `crates/nv-daemon/src/tools/mod.rs` tool dispatch function: add a helper `fn resolve_nexus_backend<'a>(nexus: Option<&'a NexusClient>, dispatcher: Option<&'a TeamAgentDispatcher>) -> NexusBackend<'a>` where `NexusBackend` is a local enum `{ Nexus(&'a NexusClient), TeamAgent(&'a TeamAgentDispatcher), None }`. Use this enum in each nexus tool arm to route the call [owner:api-engineer]
- [ ] [6.2] [P-1] Route `query_sessions` tool call — if `NexusBackend::TeamAgent`, call `dispatcher.list_agents()` and format using existing `nexus::tools::format_query_sessions`-equivalent logic (the `SessionSummary` type is shared). If `NexusBackend::Nexus`, call existing path unchanged [owner:api-engineer]
- [ ] [6.3] [P-1] Route `query_session` tool call — `TeamAgent` path calls `dispatcher.get_agent(id)`. Format with existing `SessionDetail` type [owner:api-engineer]
- [ ] [6.4] [P-1] Route `start_nexus_session` tool call — `TeamAgent` path calls `dispatcher.start_agent(project, cwd, command, machine)`. Returns same confirmation string format as nexus path [owner:api-engineer]
- [ ] [6.5] [P-1] Route `stop_nexus_session` tool call — `TeamAgent` path calls `dispatcher.stop_agent(session_id)` [owner:api-engineer]
- [ ] [6.6] [P-2] Route `query_nexus_health` tool call — `TeamAgent` path formats a health summary from `dispatcher.agent_details()` and `dispatcher.is_available()`. No gRPC health RPC needed [owner:api-engineer]
- [ ] [6.7] [P-2] Route `query_nexus_agents` tool call — `TeamAgent` path uses `dispatcher.agent_details()` to list machines and their availability [owner:api-engineer]
- [ ] [6.8] [P-2] Route `query_nexus_projects` tool call — `TeamAgent` path derives the project list from `dispatcher.list_agents()` (unique project names across active sessions) [owner:api-engineer]

## Batch 7: Callback Handler Routing

- [ ] [7.1] [P-1] In `crates/nv-daemon/src/callbacks.rs`: add `team_agent_dispatcher: Option<&TeamAgentDispatcher>` parameter to `handle_approve`. Route `ActionType::NexusStartSession` — if dispatcher is `Some`, call `dispatcher.start_agent(...)` extracting the same `project`, `command`, `agent` fields from payload; else fall through to existing nexus path [owner:api-engineer]
- [ ] [7.2] [P-1] Route `ActionType::NexusStopSession` in `handle_approve` — if dispatcher is `Some`, call `dispatcher.stop_agent(session_id)`; else existing nexus path [owner:api-engineer]
- [ ] [7.3] [P-1] Thread `team_agent_dispatcher` through all `handle_approve` call sites in `orchestrator.rs` and `worker.rs` [owner:api-engineer]
- [ ] [7.4] [P-2] Preserve the dedup guard in `execute_nexus_start_session` equivalent — before calling `dispatcher.start_agent`, call `dispatcher.has_active_agent_for_project(project)` and return early with the same "session already active" message if true [owner:api-engineer]

## Batch 8: Verification

- [ ] [8.1] [P-1] `cargo build -p nv-daemon` passes with team agents feature [owner:api-engineer]
- [ ] [8.2] [P-1] `cargo clippy -p nv-daemon -- -D warnings` passes [owner:api-engineer]
- [ ] [8.3] [P-1] `cargo test -p nv-daemon` passes — all existing nexus tests still pass; new team_agent tests pass [owner:api-engineer]
- [ ] [8.4] [P-2] `cargo test -p nv-core` passes — config deserialization tests for `TeamAgentsConfig` [owner:api-engineer]
- [ ] [8.5] [P-2] Unit test: `start_agent` with no machines configured returns `Err` [owner:api-engineer]
- [ ] [8.6] [P-2] Unit test: `has_active_agent_for_project` returns false when map is empty; returns true when a matching Active session exists [owner:api-engineer]
- [ ] [8.7] [P-2] Unit test: `list_agents` excludes Stopped sessions; includes Active and Idle [owner:api-engineer]
- [ ] [8.8] [P-3] [user] Manual test with `use_team_agents = true`: send Telegram "start session for nv" message, approve action, verify CC subprocess launches locally, `query_nexus` returns it as active session [owner:api-engineer]
- [ ] [8.9] [P-3] [user] Manual test: stop session via Telegram, verify subprocess is terminated, session disappears from `query_nexus` output [owner:api-engineer]
- [ ] [8.10] [P-3] [user] Manual test with `use_team_agents = false` (default): verify existing Nexus path unchanged — no regression [owner:api-engineer]
