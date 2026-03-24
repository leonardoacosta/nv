# Implementation Tasks

<!-- beads:epic:TBD -->

## Dependencies

- `nv-tools-extract-wave-c`

## API Batch: SharedDeps Trait

- [ ] [1.1] [P-1] Define `SharedDeps` trait in `crates/nv-tools/src/shared.rs` with async methods for memory tools (read_memory, search_memory, write_memory) [owner:api-engineer]
- [ ] [1.2] [P-1] Add async methods for nexus tools (query_nexus, query_session, start_session, stop_session, send_command, query_nexus_health/projects/agents) [owner:api-engineer]
- [ ] [1.3] [P-1] Add async methods for schedule tools (list/add/modify/remove_schedule) [owner:api-engineer]
- [ ] [1.4] [P-1] Add async methods for reminder tools (set/list/cancel_reminder) [owner:api-engineer]
- [ ] [1.5] [P-1] Add async methods for message tools (get_recent_messages, search_messages) [owner:api-engineer]
- [ ] [1.6] [P-2] Add async methods for channel tools (list_channels, send_to_channel) [owner:api-engineer]
- [ ] [1.7] [P-2] Add async methods for aggregation tools (project_health, homelab_status, financial_summary) [owner:api-engineer]
- [ ] [1.8] [P-2] Add async methods for bash tools (git_status/log/branch/diff_stat, ls_project, cat_config, bd_ready/stats) [owner:api-engineer]
- [ ] [1.9] [P-2] Add async methods for jira tools (search/get/create/transition/assign/comment) [owner:api-engineer]
- [ ] [1.10] [P-2] Add async methods for teams tools (channels/messages/send/presence) [owner:api-engineer]
- [ ] [1.11] [P-2] Add async methods for bootstrap tools (complete_bootstrap, update_soul) [owner:api-engineer]
- [ ] [1.12] [P-2] Add async method for check_services tool [owner:api-engineer]

## API Batch: Daemon Implementation

- [ ] [2.1] [P-1] Create `DaemonSharedDeps` struct in `crates/nv-daemon/src/tools/mod.rs` holding Arc references to Memory, NexusClient, ScheduleStore, ReminderStore, MessageStore, etc. [owner:api-engineer]
- [ ] [2.2] [P-1] Implement `SharedDeps` trait on `DaemonSharedDeps` -- delegate each method to existing handler code [owner:api-engineer]
- [ ] [2.3] [P-2] Wire `DaemonSharedDeps` construction in `main.rs` using existing Arc resources [owner:api-engineer]

## API Batch: MCP Registry Wiring

- [ ] [3.1] [P-1] Update `ToolRegistry` to accept `Option<Box<dyn SharedDeps>>` for daemon-coupled dispatch [owner:api-engineer]
- [ ] [3.2] [P-2] Register all 28 daemon-coupled tool definitions in registry [owner:api-engineer]
- [ ] [3.3] [P-2] Wire `tools/call` dispatch to route daemon-coupled tools through SharedDeps [owner:api-engineer]

## Verify

- [ ] [4.1] `cargo test -p nv-daemon --lib` -- all 1,032 tests pass [owner:api-engineer]
- [ ] [4.2] `cargo build -p nv-tools` -- compiles with full registry [owner:api-engineer]
- [ ] [4.3] MCP `tools/list` returns 60+ tools (all stateless + all daemon-coupled) [owner:api-engineer]
- [ ] [4.4] `cargo clippy --workspace -- -D warnings` passes [owner:api-engineer]
