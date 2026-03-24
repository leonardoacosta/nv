# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [api-engineer] Remove hardcoded `query_nexus_health`, `query_nexus_projects`, `query_nexus_agents` ToolDefinition structs from initial `tools` vec (lines 430–456) — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Update `tools.len()` assertion from 98 to 95 — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Fix env var hint in `push_env!` for Vercel: `"VERCEL_API_TOKEN"` → `"VERCEL_TOKEN"` (line 2218) — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Fix env var hint in `push_env!` for Doppler: `"DOPPLER_TOKEN"` → `"DOPPLER_API_TOKEN"` (line 2225) — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Push `Box::new(TeamsCheck)` into the `owned` vec inside `check_services` — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Add `Checkable` impl to `CalendarClient` with read probe (list next event) — `crates/nv-daemon/src/tools/calendar.rs`
- [x] [api-engineer] Push `Box::new(CalendarClient)` into the `owned` vec inside `check_services` — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Add `Checkable` impl to `JiraRegistry` with read probe (`GET /rest/api/3/myself`) — `crates/nv-daemon/src/tools/jira.rs`
- [x] [api-engineer] Push `Box::new(JiraCheck)` into the `owned` vec inside `check_services` — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Define `TOOL_TIMEOUT_READ: Duration = Duration::from_secs(30)` and `TOOL_TIMEOUT_WRITE: Duration = Duration::from_secs(60)` constants at module top level — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Wrap `execute_tool_send` dispatch arms with `tokio::time::timeout(TOOL_TIMEOUT_READ, ...)` for read tools; return `ToolResult` error string `"Tool timed out after 30s"` on expiry — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Apply `TOOL_TIMEOUT_WRITE` (60s) to write tool arms in `execute_tool_send` dispatch — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Migrate all `execute_tool(...)` test call-sites to `execute_tool_send(...)` (drop `message_store` argument) — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Delete `execute_tool` function and its `#[allow(dead_code, clippy::too_many_arguments)]` attribute (line 2266–end of function) — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Change `timed()` signature to accept a `Duration` deadline; wrap inner future with `tokio::time::timeout`; return `(elapsed_ms, Err(...))` on timeout — `crates/nv-daemon/src/tools/check.rs`
- [x] [api-engineer] Update all `timed()` call-sites in `check_read` / `check_write` implementations to pass a deadline (use 15s as default) — `crates/nv-daemon/src/tools/check.rs`
- [x] [api-engineer] Raise `REQUEST_TIMEOUT` from `Duration::from_secs(5)` to `Duration::from_secs(15)` — `crates/nv-daemon/src/tools/ha.rs`
- [x] [api-engineer] Add `teams_client: Option<TeamsClient>`, `doppler_client: Option<DopplerClient>`, `cloudflare_client: Option<CloudflareClient>` fields to `ServiceRegistries` — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Construct `TeamsClient`, `DopplerClient`, and `CloudflareClient` once at session startup and populate `ServiceRegistries` fields — `crates/nv-daemon/src/tools/mod.rs`
- [x] [api-engineer] Replace per-call client construction in Teams, Doppler, and Cloudflare dispatch arms with references from `service_registries` — `crates/nv-daemon/src/tools/mod.rs`

## Verify

- [x] [api-engineer] `cargo build` passes with no errors
- [x] [api-engineer] `cargo clippy -- -D warnings` passes
- [x] [api-engineer] Unit test: tool definitions list has no duplicate names (assert unique names == total count)
- [x] [api-engineer] Unit test: `check_services` output includes Teams, Calendar, Jira entries
- [x] [api-engineer] Unit test: `timed()` returns `Err` when probe exceeds deadline
- [x] [api-engineer] Unit test: `timed()` returns `Ok` and measured latency when probe completes within deadline
- [x] [api-engineer] Existing tests pass
