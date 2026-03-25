# Implementation Tasks

<!-- beads:epic:TBD -->

## Batch 1: CcSessionManager Core

- [ ] [1.1] [P-1] Create `crates/nv-daemon/src/cc_sessions.rs` — define `CcSessionState` enum (`Active`, `Idle`, `Stopping`, `Error(String)`), `CcSessionHandle` struct (id, project, cwd, command, started_at, state, restart_count, last_health_at, process handle), `CcSessionSummary` struct (id, project, state, started_at, duration_display, restart_count, command) [owner:api-engineer]
- [ ] [1.2] [P-1] Implement `CcSessionManager` struct — `sessions: Arc<Mutex<HashMap<Uuid, CcSessionHandle>>>`, `pub fn new() -> Self`, thread-safe via `Arc<Mutex<>>` [owner:api-engineer]
- [ ] [1.3] [P-1] Implement `CcSessionManager::start_session(project, cwd, command, agent_name) -> Result<(Uuid, String)>` — dedup guard checks for Active/Idle session on same project, spawns `claude` subprocess via `std::process::Command` with `cwd`, inserts handle with state Active, returns (session_id, display_string) [owner:api-engineer]
- [ ] [1.4] [P-1] Implement `CcSessionManager::stop_session(session_id: Uuid) -> Result<String>` — set state Stopping, SIGTERM, wait 5s, SIGKILL if still running, remove from registry, return result string [owner:api-engineer]
- [ ] [1.5] [P-1] Implement `CcSessionManager::list_sessions() -> Vec<CcSessionSummary>` — snapshot current registry, build CcSessionSummary for each handle including human-readable duration_display ("2h 14m"), sort by started_at descending [owner:api-engineer]
- [ ] [1.6] [P-2] Implement `CcSessionManager::get_status(session_id: Uuid) -> Option<CcSessionSummary>` — look up by ID and return summary, or None if not found [owner:api-engineer]
- [ ] [1.7] [P-2] Implement `CcSessionManager::find_by_project(project: &str) -> Option<Uuid>` — returns ID of first Active or Idle session matching the project name [owner:api-engineer]
- [ ] [1.8] [P-2] Add `pub mod cc_sessions;` to `crates/nv-daemon/src/lib.rs` [owner:api-engineer]

## Batch 2: Health Monitor and Auto-Restart

- [ ] [2.1] [P-1] Implement `CcSessionManager::spawn_health_monitor(self: Arc<Self>, telegram: Arc<TelegramClient>, chat_id: i64)` — tokio task running every 30s, iterates sessions, calls `child.try_wait()` on each process handle to detect unexpected exits [owner:api-engineer]
- [ ] [2.2] [P-1] Implement auto-restart logic in health monitor — on unexpected exit: increment restart_count; if restart_count <= 3 re-spawn with same (project, cwd, command), log warning; if restart_count > 3 set state Error, send Telegram message "Session <project> has crashed 3 times and will not auto-restart. Use /start <project> to retry manually." [owner:api-engineer]
- [ ] [2.3] [P-2] Update `state.rs` `SessionErrorMeta` — change comments and field source documentation from "Nexus session" to "CC session"; no struct field changes needed as fields are compatible (session_id, project, cwd, command, error_message, agent_name, timestamp) [owner:api-engineer]
- [ ] [2.4] [P-2] Wire `CcSessionManager::store_error_on_crash()` — on crash detection, call `state.store_session_error(event_id, SessionErrorMeta { ... })` with data from the CcSessionHandle [owner:api-engineer]

## Batch 3: Replace Nexus Callbacks

- [ ] [3.1] [P-1] In `callbacks.rs`, replace `execute_nexus_start_session(payload, nexus_client, project_registry)` with `execute_cc_start_session(payload, cc_sessions, project_registry)` — same payload fields (project, command, agent), same fallback cwd logic, calls `CcSessionManager::start_session` [owner:api-engineer]
- [ ] [3.2] [P-1] In `callbacks.rs`, replace `execute_nexus_stop_session(payload, nexus_client)` with `execute_cc_stop_session(payload, cc_sessions)` — reads session_id from payload, calls `CcSessionManager::stop_session` [owner:api-engineer]
- [ ] [3.3] [P-1] Update `handle_approve` signature in `callbacks.rs` — remove `nexus_client: Option<&nexus::client::NexusClient>` parameter, add `cc_sessions: &Arc<CcSessionManager>` parameter [owner:api-engineer]
- [ ] [3.4] [P-1] Update `detect_action_type` in `callbacks.rs` — rename action type variants for Nexus to CC equivalents: `NexusStartSession` → `CcStartSession`, `NexusStopSession` → `CcStopSession` in match arms; update `_action_type` string matching accordingly [owner:api-engineer]
- [ ] [3.5] [P-2] Update all callers of `handle_approve` in `orchestrator.rs` to pass `cc_sessions` instead of `nexus_client` [owner:api-engineer]

## Batch 4: Orchestrator Updates

- [ ] [4.1] [P-1] In `worker.rs`, remove `nexus_client: Option<Arc<NexusClient>>` from `SharedDeps`; add `cc_sessions: Arc<CcSessionManager>` [owner:api-engineer]
- [ ] [4.2] [P-1] In `orchestrator.rs`, remove `TriggerClass::NexusEvent` match arm and all Nexus event forwarding logic (the NexusEvent variant was used to route gRPC stream events; with Nexus removed, this trigger type is deleted) [owner:api-engineer]
- [ ] [4.3] [P-1] Remove `TriggerClass::NexusEvent` variant from the `TriggerClass` enum in `orchestrator.rs` [owner:api-engineer]
- [ ] [4.4] [P-1] In `orchestrator.rs`, add `/sessions` command handler to `handle_command` — calls `cc_sessions.list_sessions()`, formats as table (project, state dot, duration, restart_count), sends via Telegram [owner:api-engineer]
- [ ] [4.5] [P-1] In `orchestrator.rs`, add `/start <project> [command]` command handler — resolves cwd (project_registry or `$HOME/dev/<project>`), calls `cc_sessions.start_session(project, cwd, command, "nova")`, sends result or "Already active" [owner:api-engineer]
- [ ] [4.6] [P-1] In `orchestrator.rs`, add `/stop <project>` command handler — calls `cc_sessions.find_by_project(project)` then `cc_sessions.stop_session(id)`, sends "Session <project> stopped" or "No active session for <project>" [owner:api-engineer]
- [ ] [4.7] [P-2] Add `/start` and `/stop` usage responses for missing arguments — `/start` with no args returns `"Usage: /start <project> [command]"`; `/stop` with no args returns the same output as `/sessions` [owner:api-engineer]
- [ ] [4.8] [P-2] Register `/sessions`, `/start`, `/stop` in the BotFather command registry documentation comment in `orchestrator.rs` alongside existing commands [owner:api-engineer]
- [ ] [4.9] [P-2] Spawn `cc_sessions.spawn_health_monitor(Arc::clone(&cc_sessions), telegram_client, chat_id)` in the daemon main loop after all services are initialized [owner:api-engineer]

## Batch 5: Dashboard Project Sessions

- [ ] [5.1] [P-2] Create `apps/dashboard/src/app/api/session/projects/route.ts` — GET handler: fetches active CC project sessions from daemon via `DAEMON_WS_URL` or `CO_API_URL` (local env), returns `CcSessionSummary[]` JSON; if daemon unreachable, returns empty array with `{ sessions: [], error: "daemon unreachable" }` [owner:api-engineer]
- [ ] [5.2] [P-2] Create `apps/dashboard/src/components/ProjectSessionsTable.tsx` — table component: one row per session with state badge (Active=green, Idle=yellow, Error=red), project name, duration, restart count, Stop button that calls `POST /api/session/projects/stop` [owner:ui-engineer]
- [ ] [5.3] [P-2] Create `apps/dashboard/src/app/api/session/projects/stop/route.ts` — POST handler: `{ session_id: string }`, forwards stop request to daemon HTTP endpoint, returns result [owner:api-engineer]
- [ ] [5.4] [P-3] Add "Project Sessions" section to `apps/dashboard/src/app/session/page.tsx` — below the main CC session container status, renders `<ProjectSessionsTable>` with 15s polling; hides section if endpoint returns error [owner:ui-engineer]

## Batch 6: Cleanup

- [ ] [6.1] [P-2] Remove `use crate::nexus;` imports from `callbacks.rs` and `orchestrator.rs` (nexus module removed by `remove-nexus-crate`; this batch ensures no residual references remain) [owner:api-engineer]
- [ ] [6.2] [P-2] Remove `NexusStartSession` and `NexusStopSession` variants from `nv_core::types::ActionType` enum — replace with `CcStartSession` and `CcStopSession` variants; update all match exhaustiveness sites [owner:api-engineer]
- [ ] [6.3] [P-2] Update `nv_core` `Trigger` enum — remove `Trigger::NexusEvent` variant; update all match sites in orchestrator and tests [owner:api-engineer]
- [ ] [6.4] [P-3] Remove `has_active_session_for_project` dedup logic from nexus client call sites — this guard is reimplemented inside `CcSessionManager::start_session` in task 1.3 [owner:api-engineer]

## Verify

- [ ] [7.1] cargo build passes [owner:api-engineer]
- [ ] [7.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [7.3] cargo test — existing tests pass; ActionType match tests in `callbacks.rs` updated for CcStartSession/CcStopSession variants [owner:api-engineer]
- [ ] [7.4] Unit test: `CcSessionManager::start_session` dedup guard — start session for "oo", call start again for "oo", verify second call returns "Already active" without spawning a second process [owner:api-engineer]
- [ ] [7.5] Unit test: `CcSessionManager::list_sessions` — insert two handles with known started_at, verify sort order and duration_display format [owner:api-engineer]
- [ ] [7.6] Unit test: `CcSessionManager::find_by_project` — Active session found, Stopping session not returned [owner:api-engineer]
- [ ] [7.7] Unit test: health monitor restart cap — mock a session that exits 4 times, verify state transitions to Error after 3rd restart and Telegram notification is queued on 4th exit [owner:api-engineer]
- [ ] [7.8] pnpm build (dashboard) passes [owner:ui-engineer]
- [ ] [7.9] [user] Manual test: send `/start oo` via Telegram, verify session appears in `/sessions` list with Active state [owner:user]
- [ ] [7.10] [user] Manual test: send `/stop oo`, verify session removed from `/sessions` list [owner:user]
- [ ] [7.11] [user] Manual test: dashboard session page shows "Project Sessions" section with active session row, Stop button stops session [owner:user]
- [ ] [7.12] [user] Manual test: kill CC subprocess manually, verify daemon auto-restarts it within 60s and logs warning [owner:user]
