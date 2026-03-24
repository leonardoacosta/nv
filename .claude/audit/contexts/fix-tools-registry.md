# Context: Fix Tools Registry Issues

## Source: Audit 2026-03-23 (tools domain, 78/C+ health)

## Problem
Duplicate tool definitions sent to Anthropic API, env var hint mismatches, missing health check coverage, no dispatch timeout.

## Findings

### P1 — 3 duplicate tool definitions sent to Anthropic API
- `crates/nv-daemon/src/tools/mod.rs:430-456` — query_nexus_health, query_nexus_projects, query_nexus_agents hardcoded in initial `tools` vec
- `crates/nv-daemon/src/tools/mod.rs:563` — same 3 tools registered again via nexus_tool_definitions() at lines 1125-1151
- Test at line 3262 asserts tools.len() == 98, silently including 6 duplicate entries
- Anthropic API receives 3 duplicate tool definitions on every session start
- Fix: Remove lines 430-456 from hardcoded vec, update test count

### P1 — Env var name mismatches in check_services
- `crates/nv-daemon/src/tools/mod.rs:2218` — Vercel hint says `VERCEL_API_TOKEN` but actual var is `VERCEL_TOKEN`
- `crates/nv-daemon/src/tools/mod.rs:2225` — Doppler hint says `DOPPLER_TOKEN` but actual var is `DOPPLER_API_TOKEN`
- Operators following missing-credential hints set wrong variable

### P2 — Teams/Calendar/Jira not covered by check_services
- TeamsCheck implemented at `crates/nv-daemon/src/tools/teams.rs:478` but never added to `owned` vec
- Calendar has no Checkable impl
- Jira has no Checkable impl
- These services invisible in health reports

### P2 — No per-call dispatch timeout
- `crates/nv-daemon/src/tools/mod.rs:1478` — execute_tool_send has no outer tokio::time::timeout
- Individual tools rely on reqwest timeouts (10-15s) but TCP black-holes can hang indefinitely
- Architecture spec defines 30s read / 60s write but not enforced at dispatch level

### P2 — execute_tool() is dead code
- `crates/nv-daemon/src/tools/mod.rs:2267` — #[allow(dead_code)]
- Replicates full dispatch logic of execute_tool_send()
- O(N) maintenance burden across 100 tools
- Fix: Delete entirely

### P3 — check::timed() has no timeout
- `crates/nv-daemon/src/tools/check.rs:369`
- Just measures elapsed time, no deadline
- check_all via FuturesUnordered has no overall timeout
- Single stalled TCP probe blocks check_all indefinitely

### P3 — HA timeout 5s vs 15s standard
- `crates/nv-daemon/src/tools/ha.rs` — REQUEST_TIMEOUT 5s, others use 15s
- Local HA may legitimately take longer, causing false-Unhealthy

### P3 — Teams/Doppler/Cloudflare rebuild clients on every tool call
- No caching in ServiceRegistries for these 3 services

## Files to Modify
- `crates/nv-daemon/src/tools/mod.rs` (duplicates, env hints, dispatch timeout, dead code)
- `crates/nv-daemon/src/tools/check.rs` (timed timeout)
- `crates/nv-daemon/src/tools/ha.rs` (timeout constant)
- `crates/nv-daemon/src/tools/teams.rs` (add to check_services)
