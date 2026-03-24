# Proposal: Fix Tools Registry

## Change ID
`fix-tools-registry`

## Summary

Eight correctness and reliability issues in the tool registry, discovered by the 2026-03-23 audit
(tools domain, 78/C+ health). Two are P1 bugs actively degrading the API contract with Anthropic
(duplicate tool definitions, wrong env var hints). Three are P2 gaps that leave services invisible
to health checks and allow unbounded tool hangs. Three are P3 polish items.

## Context
- Extends: `crates/nv-daemon/src/tools/mod.rs` (duplicates, env hints, dispatch timeout, dead code)
- Extends: `crates/nv-daemon/src/tools/check.rs` (timed timeout)
- Extends: `crates/nv-daemon/src/tools/ha.rs` (timeout constant)
- Extends: `crates/nv-daemon/src/tools/teams.rs` (add to check_services)
- Source: `.claude/audit/contexts/fix-tools-registry.md`

## Motivation

### P1 — Duplicate tool definitions sent to Anthropic API

`query_nexus_health`, `query_nexus_projects`, and `query_nexus_agents` are hardcoded in the initial
`tools` vec at lines 430–456 of `mod.rs`, then registered a second time via `nexus_tool_definitions()`
at lines 1125–1151. Every session start sends 6 entries to the Anthropic API where 3 are expected.
The tool count test at line 3262 asserts `tools.len() == 98`, silently including the 3 duplicates
and masking the bug.

### P1 — Env var name mismatches in check_services

`check_services` emits operator-facing hints when a credential is missing. Line 2218 says
`VERCEL_API_TOKEN` but `VercelClient::from_env()` reads `VERCEL_TOKEN`. Line 2225 says
`DOPPLER_TOKEN` but `DopplerClient::from_env()` reads `DOPPLER_API_TOKEN`. Operators following
these hints set the wrong variable and the service remains unconfigured.

### P2 — Teams/Calendar/Jira not covered by check_services

`TeamsCheck` is implemented at `teams.rs:478` but never pushed into the `owned` vec inside
`check_services`. Calendar and Jira have no `Checkable` impl at all. These services are invisible
in health reports.

### P2 — No per-call dispatch timeout

`execute_tool_send` at line 1478 has no outer `tokio::time::timeout`. Individual tools rely on
reqwest timeouts (10–15s), but TCP black-holes can hang indefinitely at the OS level. The
architecture spec defines 30s read / 60s write budgets but they are not enforced at the dispatch
layer.

### P2 — execute_tool() is dead code duplicating dispatch

`execute_tool` at line 2267 carries `#[allow(dead_code)]` and replicates the full dispatch
logic of `execute_tool_send`. Tests call it directly, but production paths use
`execute_tool_send` exclusively. The duplication creates O(N) maintenance burden across ~100
tools. Fix: migrate tests to `execute_tool_send`, then delete `execute_tool`.

### P3 — check::timed() has no timeout

`check::timed()` at `check.rs:369` measures elapsed time but imposes no deadline. A single
stalled TCP probe inside `check_all` blocks the entire `FuturesUnordered` pipeline
indefinitely.

### P3 — HA timeout 5s vs 15s standard

`ha.rs` sets `REQUEST_TIMEOUT` to 5s at line 22. Every other service client uses 15s. Local
Home Assistant instances can legitimately take longer, causing false-Unhealthy results.

### P3 — Teams/Doppler/Cloudflare rebuild clients per call

These three service clients construct a new HTTP client on every tool invocation. The other
services cache their clients in `ServiceRegistries`. Rebuilding allocates a new TLS session
and connection pool on each call.

## Requirements

### Req-1: Remove duplicate nexus tool definitions

Delete the three hardcoded `ToolDefinition` structs for `query_nexus_health`,
`query_nexus_projects`, and `query_nexus_agents` from the initial `tools` vec (lines 430–456).
They are already registered via `nexus_tool_definitions()`. Update the `tools.len()` assertion
from 98 to 95.

### Req-2: Fix env var hints in check_services

In `push_env!` calls at lines 2218 and 2225, correct the hint strings:
- `"VERCEL_API_TOKEN"` → `"VERCEL_TOKEN"`
- `"DOPPLER_TOKEN"` → `"DOPPLER_API_TOKEN"`

### Req-3: Add Teams to check_services; add Calendar and Jira Checkable impls

Push `Box::new(TeamsCheck)` into the `owned` vec inside `check_services`. Add `Checkable`
implementations for Calendar (read probe: list next event) and Jira (read probe: `GET /rest/api/3/myself`).
Push both into `owned`.

### Req-4: Add dispatch-level timeout to execute_tool_send

Wrap the dispatch `match` body in `execute_tool_send` with `tokio::time::timeout`. Use
`TOOL_TIMEOUT_READ` (30s) for read tools and `TOOL_TIMEOUT_WRITE` (60s) for write tools.
On timeout, return a `ToolResult` error string `"Tool timed out after Ns"` so Claude can
continue without that data. Constants should be defined as `const` at module top level.

### Req-5: Delete execute_tool()

Migrate all test call-sites from `execute_tool(...)` to `execute_tool_send(...)`, then delete
the `execute_tool` function and its `#[allow(dead_code)]` attribute.

### Req-6: Add deadline to check::timed()

Change `timed()` to accept a `Duration` deadline parameter. Internally wrap the probe future
with `tokio::time::timeout`. On timeout, return `(elapsed_ms, Err(anyhow!("probe timed out
after {deadline:?}")))` so callers surface a `Timeout` status rather than hanging. Update all
call-sites in `check_read` / `check_write` implementations.

### Req-7: Raise HA timeout to 15s

Change `REQUEST_TIMEOUT` in `ha.rs` from `Duration::from_secs(5)` to `Duration::from_secs(15)`
to match every other service client.

### Req-8: Cache clients for Teams, Doppler, Cloudflare in ServiceRegistries

Add `TeamsClient`, `DopplerClient`, and `CloudflareClient` fields to `ServiceRegistries`.
Construct them once at session startup (alongside existing clients) and thread them through
`execute_tool_send`. Remove the per-call construction from tool dispatch arms.

## Scope
- **IN**: duplicate removal, env hint fixes, check_services coverage, dispatch timeout, dead code
  deletion, timed() deadline, HA timeout, client caching
- **OUT**: new tool implementations, new service integrations, Checkable impls beyond Teams/Calendar/Jira

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/mod.rs` | Remove lines 430–456, fix env hints, add dispatch timeout, delete `execute_tool`, add Teams/Calendar/Jira to check_services, add clients to ServiceRegistries |
| `crates/nv-daemon/src/tools/check.rs` | Add deadline param to `timed()`, update all call-sites |
| `crates/nv-daemon/src/tools/ha.rs` | Raise `REQUEST_TIMEOUT` to 15s |
| `crates/nv-daemon/src/tools/teams.rs` | Add `TeamsCheck` to `owned` vec; add client to ServiceRegistries |
| `crates/nv-daemon/src/tools/calendar.rs` | Add `Checkable` impl |
| `crates/nv-daemon/src/tools/jira.rs` | Add `Checkable` impl |

## Risks
| Risk | Mitigation |
|------|-----------|
| Removing duplicate tools shifts `len()` — other assertions may depend on count | Only one assertion at line 3262; update it to 95 |
| Migrating `execute_tool` test call-sites to `execute_tool_send` — signature differs (no `message_store`) | `execute_tool_send` does not take `message_store`; tests that passed `None` can drop the argument cleanly |
| Dispatch timeout fires on legitimately slow tools (Nexus over WAN) | 30s read budget is generous vs observed <5s; configurable per-category if needed |
| `timed()` signature change breaks existing Checkable impls | All call-sites are internal; update in same PR |
