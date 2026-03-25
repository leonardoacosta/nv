# Implementation Tasks

<!-- beads:epic:TBD -->

## DB Batch

_(no database changes)_

## API Batch

- [ ] [2.1] [P-1] Delete `crates/nv-daemon/src/nexus/` directory (all 8 files: client.rs, connection.rs, mod.rs, notify.rs, progress.rs, stream.rs, tools.rs, watchdog.rs) [owner:api-engineer]
- [ ] [2.2] [P-1] Delete `proto/nexus.proto`; remove `proto/` directory if now empty [owner:api-engineer]
- [ ] [2.3] [P-1] Replace `build.rs` with empty `fn main() {}` (removes tonic_build::compile_protos call); delete file entirely if workspace build script not needed [owner:api-engineer]
- [ ] [2.4] [P-1] Remove `tonic`, `prost`, `prost-types` from `[dependencies]` in `crates/nv-daemon/Cargo.toml` [owner:api-engineer]
- [ ] [2.5] [P-1] Remove `tonic-build` from `[build-dependencies]` in `crates/nv-daemon/Cargo.toml` [owner:api-engineer]
- [ ] [2.6] [P-1] Remove `mod nexus;` from `crates/nv-daemon/src/lib.rs` [owner:api-engineer]
- [ ] [2.7] [P-1] Remove `mod nexus;` from `crates/nv-daemon/src/main.rs`; remove nexus init block (NexusClient::new, stream::spawn_event_streams, watchdog::run_watchdog), remove `health_poll_nexus` variable and its use in the health poller spawn [owner:api-engineer]
- [ ] [2.8] [P-1] Remove `nexus_client` field from `DashboardState` in `dashboard.rs`; remove `use crate::nexus::client::NexusClient` and `use crate::nexus::progress::parse_session_progress` imports; remove nexus-dependent handler bodies for session query and solve-with-nexus endpoints [owner:api-engineer]
- [ ] [2.9] [P-1] Remove `nexus_client` field and `NexusClient` import from `http.rs`; update router state construction to remove the field; update all `AppState` builder call sites [owner:api-engineer]
- [ ] [2.10] [P-1] Remove `Option<Arc<crate::nexus::client::NexusClient>>` parameter from `HealthPoller` struct and `run_poll_cycle` in `health_poller.rs`; remove nexus investigation block (lines ~169–183); update all callers [owner:api-engineer]
- [ ] [2.11] [P-1] Remove `nexus_client` field from `OrchestratorDeps` in `orchestrator.rs`; remove `use crate::nexus` import; remove `handle_nexus_events`, `handle_nexus_view_error`, `handle_nexus_create_bug` methods; remove all call sites [owner:api-engineer]
- [ ] [2.12] [P-1] Remove `nexus_client` field from worker deps struct in `worker.rs`; remove `use crate::nexus` import; remove nexus_client pass-through to tool execution; remove `nexus_count` preamble stripping logic [owner:api-engineer]
- [ ] [2.13] [P-1] Remove `use crate::nexus` import, `nexus_tool_definitions()` function, `tools.extend(nexus_tool_definitions())` call, all nexus execution arms (`query_nexus`, `query_nexus_health`, `query_nexus_projects`, `query_nexus_agents`, `nexus_project_ready`, `nexus_project_proposals`), and `Option<&nexus::client::NexusClient>` parameter from `execute_tools` in `tools/mod.rs`; update all callers of `execute_tools` to drop the nexus_client argument [owner:api-engineer]
- [ ] [2.14] [P-1] Remove nexus test fixtures in `tools/mod.rs` (NexusClient::new test helpers, lines ~3277–3325); update affected unit tests to pass `None` or remove the parameter [owner:api-engineer]
- [ ] [2.15] [P-2] Remove `use crate::nexus` import, `Option<&nexus::client::NexusClient>` parameter, and `gather_nexus` join branch from `aggregation.rs`; remove the nexus result variable and any rendering that depends on it [owner:api-engineer]
- [ ] [2.16] [P-2] Remove `use crate::nexus` import, `execute_nexus_start_session`, and `execute_nexus_stop_session` functions from `callbacks.rs`; remove nexus arms in the action dispatch match; update callers to remove the nexus_client argument [owner:api-engineer]
- [ ] [2.17] [P-2] Remove `use crate::nexus` import, `gather_nexus()` async function, `nexus_sessions` field from the gather context struct, and the nexus join arm in `digest/gather.rs` [owner:api-engineer]
- [ ] [2.18] [P-2] Remove `nexus_sessions` rendering block from `digest/synthesize.rs` (lines ~125–128 and all related match/format arms) [owner:api-engineer]
- [ ] [2.19] [P-2] Remove `query_nexus` from the tool-name list in `agent.rs` trigger batch formatting; remove the `format_trigger_batch_nexus` unit test [owner:api-engineer]
- [ ] [2.20] [P-2] Remove `resp.channels.get("nexus_homelab")` lookup and associated status rendering from `health.rs` [owner:api-engineer]

## Verify

- [ ] [3.1] `cargo build --package nv-daemon` passes with zero errors [owner:api-engineer]
- [ ] [3.2] `cargo clippy --package nv-daemon -- -D warnings` passes [owner:api-engineer]
- [ ] [3.3] `cargo test --package nv-daemon` passes — all remaining tests green, zero failures [owner:api-engineer]
- [ ] [3.4] `grep -r "nexus" crates/nv-daemon/src/ --include="*.rs" -l` returns no files [owner:api-engineer]
- [ ] [3.5] `grep -r "tonic\|prost" crates/nv-daemon/Cargo.toml` returns no matches [owner:api-engineer]
- [ ] [3.6] `ls proto/nexus.proto` returns no such file [owner:api-engineer]
