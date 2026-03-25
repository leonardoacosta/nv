# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Add `has_active_session_for_project(&self, project: &str) -> bool` async method to `NexusClient` in `nexus/client.rs` — calls `GetSessions` with `SessionFilter { project: Some(...), .. }` on each connected agent [owner:api-engineer]
- [x] [2.2] [P-1] In `has_active_session_for_project`, return `true` if any session has status `active` or `idle`; treat `stale` and `errored` as non-blocking [owner:api-engineer]
- [x] [2.3] [P-1] On RPC failure in `has_active_session_for_project`, log a warning and continue to next agent — if all agents fail, return `false` (fail-open) [owner:api-engineer]
- [x] [2.4] [P-1] In `callbacks.rs` `execute_nexus_start_session`, call `client.has_active_session_for_project(project).await` before `client.start_session(...)` [owner:api-engineer]
- [x] [2.5] [P-1] If `has_active_session_for_project` returns `true`, return `Ok("Session already active for {project} — launch skipped".to_string())` without calling `start_session` [owner:api-engineer]
- [x] [2.6] [P-2] Emit `tracing::info!(project, dedup = true, "session launch skipped — already active")` at the dedup return site [owner:api-engineer]

## Verify

- [x] [3.1] `cargo build` passes [owner:api-engineer]
- [x] [3.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [3.3] Unit test: `has_active_session_for_project` returns `false` when no agents are connected [owner:api-engineer]
- [x] [3.4] Unit test: `has_active_session_for_project` returns `false` when all agents are disconnected (fail-open path) [owner:api-engineer]
- [x] [3.5] Unit test: status mapping — `active` and `idle` are blocking; `stale`, `errored`, `unknown` are not [owner:api-engineer]
- [x] [3.6] Existing tests pass [owner:api-engineer]
