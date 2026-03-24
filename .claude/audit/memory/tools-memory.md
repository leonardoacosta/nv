# Tools Domain Audit Memory

**Audit date:** 2026-03-23
**Scope:** `crates/nv-daemon/src/tools/` (20 tool files + jira/ submodule, mod.rs 4294 lines)
**Findings logged to:** `~/.claude/scripts/state/nv-audit-findings.jsonl`

---

## Health Scores

| Axis | Score | Grade |
|------|-------|-------|
| Structure | 78 | C |
| Quality | 82 | B |
| Architecture | 75 | C |
| **Health** | **78** | **C** |

---

## Critical Findings (High Severity)

### Duplicate Tool Registrations — mod.rs lines 430-456 and 1125-1151

`query_nexus_health`, `query_nexus_projects`, and `query_nexus_agents` are registered **twice** in `register_tools()`:
- First time: hardcoded in the initial `tools` vec (~line 430)
- Second time: inside `nexus_tool_definitions()` called at line 563

The `register_tools_returns_expected_count` test asserts `len() == 98` and passes — meaning the duplicate is counted in the expected total of 98. This is a latent bug: the Anthropic API receives duplicate tool entries which may cause undefined behavior (API rejection or the model calling the tool incorrectly).

**Fix:** Remove the three hardcoded entries from the initial `tools` vec (lines 430-456). They are already emitted by `nexus_tool_definitions()`. Update the test count and comment accordingly.

---

## Medium Severity Findings

### 1. Env Var Mismatch in check_services (2 instances)

| Tool | check_services hint | Actual env var in from_env() |
|------|--------------------|-----------------------------|
| Vercel | `VERCEL_API_TOKEN` | `VERCEL_TOKEN` |
| Doppler | `DOPPLER_TOKEN` | `DOPPLER_API_TOKEN` |

`mod.rs` line 2218 and 2225. Misleads operators trying to debug `CheckResult::Missing` messages.

### 2. Teams / Calendar / Jira Absent from check_services

`TeamsCheck` is implemented in `tools/teams.rs:478` but never added to the `owned` vec in `check_services`. Calendar has no `Checkable` impl. Jira has no `Checkable` impl. These 3 services are invisible to health checks.

### 3. No Dispatch-Layer Timeout

`execute_tool_send` has no outer `tokio::time::timeout`. Individual tools rely on `reqwest::Client` timeout (10-15s), but `Neon` uses `tokio_postgres` with separate CONNECT (10s) and QUERY (30s) timeouts. A stalled TCP connection to a non-routing host could hold a dispatch slot indefinitely.

### 4. `execute_tool` Dead Code — Out-of-Sync Duplicate

`execute_tool()` (line 2267) is `#[allow(dead_code)]` and duplicates the full dispatch of `execute_tool_send`. It has a `message_store` parameter that the Send variant doesn't. The two functions will drift. The dead function should either be removed or consolidated into a shared internal helper.

---

## Low Severity Findings

| Finding | File | Line |
|---------|------|------|
| Test count comment says "3 schedule" but 4 are defined (modify_schedule present) | mod.rs | 3258 |
| check_services rebuilds clients from env on every call instead of reusing ServiceRegistries | mod.rs | 2199 |
| Neon SQL blocklist missing GRANT, REVOKE, VACUUM (READ ONLY tx mode compensates) | neon.rs | 41 |
| HA timeout 5s vs 15s standard; may cause false-Unhealthy for slow local HA | ha.rs | 22 |
| check::timed() has no timeout — check_all can hang indefinitely on black-hole hosts | check.rs | 369 |
| Teams rebuilds MsGraphAuth+Secrets on every tool call; no ServiceRegistry pattern | mod.rs | 2038 |
| Doppler and Cloudflare also reconstruct clients per call; inconsistent with Vercel/Stripe/Sentry | mod.rs | 2141 |

---

## Positive Patterns (What's Working Well)

- **Checkable trait coverage**: 15 of 20 services have `Checkable` impls (Teams, Neon, Jira missing from check_services but Teams has impl).
- **ServiceRegistry<T>**: Clean fallback chain (project → "default" → first). Used by Stripe, Vercel, Sentry, Resend, HA, Upstash, ADO, Cloudflare, Doppler.
- **Neon SQL safety**: Blocklist + READ ONLY transaction mode + 50-row limit + cell truncation. Well-defended.
- **Secret handling**: All tokens read from env vars at client construction. No hardcoded secrets found.
- **Error messages**: Actionable 401/403/429/404 mapping in most clients (Stripe, Sentry, Doppler, PostHog, CloudFlare).
- **Jira multi-instance**: JiraRegistry + project_map handles multi-tenant Jira correctly.
- **Confirmation gating**: All write tools return PendingAction, not Immediate. No accidental writes.
- **GitHub**: Subprocess timeout via `tokio::time::timeout(GH_TIMEOUT, ...)` — correct pattern.
- **check_all**: `FuturesUnordered` — concurrent probes, no sequential bottleneck.

---

## Architecture Notes

- **Two dispatch functions** exist: `execute_tool_send` (live/used) and `execute_tool` (dead). The live one handles all production traffic. The dead one is the legacy form preserved for `execute_tool` callers (none after refactor). Safe to delete.
- **ServiceRegistries struct** is the migration target. Vercel/Stripe/Sentry/Resend/HA/Upstash/ADO/Cloudflare/Doppler have slots. Teams, Calendar, PostHog, GitHub, Docker, Neon, Plaid do not — they construct inline. Teams/Doppler/Cloudflare are the highest-priority candidates for promotion.
- **Timeout hierarchy**: reqwest.timeout (10-15s) > tokio_postgres CONNECT (10s) + QUERY (30s). No global per-tool deadline. Consistent with the architecture spec (30s read, 60s write) in intent but not enforced at dispatch.
