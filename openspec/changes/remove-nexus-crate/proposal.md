# Proposal: Remove Nexus Crate

## Change ID
`remove-nexus-crate`

## Summary

Delete `crates/nv-daemon/src/nexus/` (8 files) and all references to Nexus throughout the daemon
codebase. Remove tonic/prost build dependencies. This is a post-migration cleanup spec that runs
after `replace-nexus-with-team-agents` has replaced all Nexus functionality with team-agent
equivalents.

## Context
- Depends on: `replace-nexus-with-team-agents`
- Depended on by: `remove-nexus-register-binary`, `update-session-lifecycle`
- Phase: Wave 4b
- Type: refactor — no new behaviour, no public API changes

## Motivation

The `nexus/` module is a gRPC client layer connecting the daemon to a separate Nexus process.
After `replace-nexus-with-team-agents` ships, every call site that previously used
`NexusClient` is routed through the new team-agent system instead. Leaving the dead module in
place:

1. Keeps tonic/prost/tonic-build in the dependency graph, inflating compile times and binary size
2. Leaves `build.rs` compiling `nexus.proto` on every clean build even though the generated code
   is unreachable
3. Clutters grep results and code review with ~21 files worth of stale references
4. Maintains a `mod nexus;` declaration in `lib.rs` and `main.rs` that signals live code to
   future contributors

Removing it now (immediately after the migration lands) keeps the diff reviewable and
contained to a single spec.

## Requirements

### Req-1: Delete the nexus module directory

Remove the entire `crates/nv-daemon/src/nexus/` directory:

```
crates/nv-daemon/src/nexus/client.rs
crates/nv-daemon/src/nexus/connection.rs
crates/nv-daemon/src/nexus/mod.rs
crates/nv-daemon/src/nexus/notify.rs
crates/nv-daemon/src/nexus/progress.rs
crates/nv-daemon/src/nexus/stream.rs
crates/nv-daemon/src/nexus/tools.rs
crates/nv-daemon/src/nexus/watchdog.rs
```

### Req-2: Remove `mod nexus` declarations

Remove `mod nexus;` from:
- `crates/nv-daemon/src/lib.rs` (line 29)
- `crates/nv-daemon/src/main.rs` (line 17)

### Req-3: Remove NexusClient from DashboardState and http.rs

`dashboard.rs` holds `Option<Arc<NexusClient>>` on `DashboardState` (line 54) and uses it in
two handler functions (lines 340–492). Remove the field, the two `use crate::nexus::*` imports,
and the handler logic that references `nexus`. The `solve-with-nexus` endpoint either becomes a
404 stub or is removed entirely (decision deferred to `replace-nexus-with-team-agents`; this
spec removes whichever dead code remains after that spec).

`http.rs` passes `nexus_client` when constructing the router state (lines 44, 76, 478–492).
Remove the field from the state struct and the constructor argument.

### Req-4: Remove NexusClient from health_poller.rs

`health_poller.rs` accepts `Option<Arc<NexusClient>>` and passes it into `run_poll_cycle`.
Remove the parameter, all call sites inside the file, and the associated Nexus investigation
logic (lines 169–183). Keep the rest of the poll cycle intact.

### Req-5: Remove nexus references from orchestrator.rs

`orchestrator.rs` references `nexus_client` in `OrchestratorDeps` and calls:
- `handle_nexus_events` (line 412)
- `handle_nexus_view_error` (line 759)
- `handle_nexus_create_bug` (line 761)
- `nexus_client` field accesses (lines 819, 1003, 1071, 1112)

Remove the `nexus_client` field from `OrchestratorDeps`, the three handler methods, and all
call sites. Remove `use crate::nexus;` import (line 21).

### Req-6: Remove nexus references from worker.rs

`worker.rs` carries `pub nexus_client: Option<nexus::client::NexusClient>` in the worker deps
struct (line 171) and passes it into tool execution (line 1324). Remove the field, the pass-
through, and the `use crate::nexus;` import (line 27). Remove the `nexus_count` preamble
stripping logic (lines 1934–1947) — after migration no nexus tool calls will appear in
transcripts.

### Req-7: Remove nexus references from tools/mod.rs

`tools/mod.rs` contains:
- `use crate::nexus;` import (line 178)
- `nexus_tool_definitions()` function (line 861–) which returns tool definitions for
  `query_nexus`, `query_nexus_health`, `query_nexus_projects`, `query_nexus_agents`,
  `nexus_project_ready`, `nexus_project_proposals`
- `tools.extend(nexus_tool_definitions())` registration call (line 380)
- Execution arms for all nexus tool names (lines 1616–1717)
- `Option<&nexus::client::NexusClient>` parameter on `execute_tools` (line 1525)
- Test fixtures that construct a `NexusClient` (lines 3277–3325)

Remove all of the above. Remove the `nexus_client` parameter from `execute_tools` and update
all call sites to match.

### Req-8: Remove nexus references from aggregation.rs, callbacks.rs, digest/gather.rs, digest/synthesize.rs

Each of these files imports `use crate::nexus` and passes `nexus_client` through one or more
function signatures. After the migration, no nexus client is available to pass. Remove:
- `aggregation.rs`: `use crate::nexus` (line 21), `Option<&nexus::client::NexusClient>`
  parameter (line 202), the `join!` branch that calls `format_query_sessions` (lines 256–288)
- `callbacks.rs`: `use crate::nexus` (line 16), `execute_nexus_start_session` and
  `execute_nexus_stop_session` functions, all call sites and parameter threads
- `digest/gather.rs`: `use crate::nexus` (line 8), `gather_nexus()` function (line 190),
  the `nexus_sessions` field in the gather context, and rendering in `synthesize.rs` (lines 125–128)

### Req-9: Remove agent.rs nexus tool name reference

`agent.rs` references `query_nexus` in a format string for trigger batch formatting (line 20
import list, line 247 function). Remove the nexus entry from the tool-name list and the
associated unit test assertion (lines 247–255).

### Req-10: Remove health.rs nexus_homelab channel lookup

`health.rs` reads `resp.channels.get("nexus_homelab")` (line 250). Remove the lookup and any
associated status rendering that depends on a Nexus channel entry.

### Req-11: Remove tonic/prost from Cargo.toml and build.rs

From `crates/nv-daemon/Cargo.toml` remove:
- `tonic = { workspace = true }`
- `prost = { workspace = true }`
- `prost-types = { workspace = true }`

From `[build-dependencies]` remove:
- `tonic-build = "0.12"`

Replace `build.rs` with an empty main function (or delete it if no other build script logic
exists). Verify `tonic-build` is not used elsewhere in the workspace before removing from the
workspace `Cargo.toml`; if used by another crate, leave the workspace definition but remove
only the nv-daemon dependency line.

### Req-12: Delete the proto file

Remove `proto/nexus.proto`. If no other proto files remain in `proto/`, remove the directory.
Verify no other crate in the workspace references this file via a `build.rs`.

### Req-13: All tests pass

After the above removals, `cargo test --package nv-daemon` must pass with zero failures. Any
unit tests that exclusively tested Nexus behaviour (e.g., the 4 `detect_action_type_*_nexus_*`
tests in `callbacks.rs`) are deleted as part of removing their parent functions.

## Scope
- **IN**: Deleting the nexus module, removing all call sites, removing tonic/prost deps, deleting the proto file
- **OUT**: Introducing replacement behaviour (handled by `replace-nexus-with-team-agents`), changes to workspace-level tonic usage in other crates, changes to `nv-core` config types that still reference `NexusAgent` (separate spec)

## Impact

| File | Change |
|------|--------|
| `crates/nv-daemon/src/nexus/` (8 files) | Delete entirely |
| `proto/nexus.proto` | Delete |
| `crates/nv-daemon/build.rs` | Remove or replace with empty fn main() |
| `crates/nv-daemon/Cargo.toml` | Remove tonic, prost, prost-types deps; remove tonic-build build-dep |
| `crates/nv-daemon/src/lib.rs` | Remove `mod nexus;` |
| `crates/nv-daemon/src/main.rs` | Remove `mod nexus;`, nexus init block, health_poll_nexus |
| `crates/nv-daemon/src/dashboard.rs` | Remove NexusClient field, imports, handler logic |
| `crates/nv-daemon/src/http.rs` | Remove NexusClient field and constructor arg |
| `crates/nv-daemon/src/health_poller.rs` | Remove nexus_client param and investigation logic |
| `crates/nv-daemon/src/orchestrator.rs` | Remove nexus_client field, 3 handler methods, call sites |
| `crates/nv-daemon/src/worker.rs` | Remove nexus_client field, preamble stripping logic |
| `crates/nv-daemon/src/tools/mod.rs` | Remove nexus tool defs, registrations, execution arms, test fixtures |
| `crates/nv-daemon/src/aggregation.rs` | Remove nexus param and join branch |
| `crates/nv-daemon/src/callbacks.rs` | Remove nexus session functions and call sites |
| `crates/nv-daemon/src/digest/gather.rs` | Remove gather_nexus and nexus_sessions field |
| `crates/nv-daemon/src/digest/synthesize.rs` | Remove nexus_sessions rendering |
| `crates/nv-daemon/src/agent.rs` | Remove nexus tool name from trigger batch list |
| `crates/nv-daemon/src/health.rs` | Remove nexus_homelab channel lookup |

## Risks

| Risk | Mitigation |
|------|-----------|
| `replace-nexus-with-team-agents` not fully complete when this spec runs | Hard dependency — this spec MUST NOT run until the predecessor spec is merged and CI is green |
| tonic/prost still referenced elsewhere in the workspace | Check `grep -r "tonic\|prost" crates/ --include="Cargo.toml"` before removing workspace definitions |
| Removing `nexus_client` param cascades through many function signatures | Work inside-out: delete module first, let compiler errors guide remaining removals |
| Deleting nexus-specific tests reduces coverage on adjacent behaviour | Review adjacent test coverage before deleting; add replacement tests if needed |
