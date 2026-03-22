# Proposal: Add Aggregation Layer

## Change ID
`add-aggregation-layer`

## Summary

Three composite tools that call individual data source tools in parallel and return unified
summaries: `project_health(code)` aggregates Vercel + Sentry + Jira + Nexus + Neon + GitHub;
`homelab_status()` aggregates Docker + Tailscale + HA; `financial_summary()` aggregates
Plaid + Stripe. This is the layer that powers Nova's dashboard digest and high-level status commands.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions + dispatch), `crates/nv-daemon/src/agent.rs` (tool execution)
- Depends on: ALL individual data source specs (5-17) — Docker, Tailscale, GitHub, Vercel, Sentry, PostHog, Neon, Stripe, Resend, Upstash, HA, ADO, Plaid
- Related: `add-tool-audit-log` (audit logging), digest system (proactive-digest), Telegram commands (/status, /health)
- PRD ref: Phase 2, Section 6.2 — Aggregation Layer

## Motivation

Individual tools are useful for specific queries, but Leo's most common question is "How's
everything?" The aggregation layer provides single-tool answers to broad questions by calling
multiple data sources in parallel and synthesizing the results. `project_health` is what powers
the dashboard digest line:
```
OO: deploy 22m ago | 0 errors | 3 Jira (1 P1) | 1 session
```

Without aggregation, Claude would need to make 6+ sequential tool calls to answer "How's OO?"
The aggregation layer does this in parallel (~100ms) and returns a pre-formatted summary.

## Requirements

### Req-1: Aggregation Module

New file `crates/nv-daemon/src/aggregation.rs` with:
- `AggregationService` struct holding references to all individual data source clients
- Uses `tokio::join!` (or `futures::join_all`) to call sources in parallel
- Each sub-call has independent timeout (5s) — one failing source doesn't block others
- Failed sources show as "unavailable" in output, not errors

### Req-2: project_health Tool

`project_health(code)` — Comprehensive health check for a single project.

- Input: `code` (required) — project code (e.g., `"oo"`, `"tc"`, `"mv"`)
- Sources called in parallel:
  - `vercel_deployments(project)` — latest deploy status + age
  - `sentry_issues(project)` — unresolved error count + top error
  - `jira_search(project)` — open issue count by priority
  - `query_nexus(project)` — active CC sessions
  - `neon_query(project, "SELECT ...")` — DB size, connection count (if project uses Neon)
  - `gh_run_status(repo)` — latest CI run status
- Output: Single formatted block per dimension with status indicator:
  ```
  OO Health:
    Deploy: 22m ago (succeeded)
    Errors: 0 unresolved
    Issues: 3 open (1 P1, 2 P2)
    Sessions: 1 active
    DB: 45MB, 2 connections
    CI: passing
  ```
- Project-to-resource mapping: hardcoded map of project code to Vercel project name, Sentry slug, Jira key, GitHub repo, Neon project ID

### Req-3: homelab_status Tool

`homelab_status()` — Health check for homelab infrastructure.

- No input parameters
- Sources called in parallel:
  - `docker_status()` — container states (running, stopped, unhealthy)
  - `tailscale_status()` — connected nodes, online/offline status
  - `ha_states()` — entity summary (lights on, sensors, climate)
- Output: Single formatted block:
  ```
  Homelab:
    Docker: 12/14 running, 2 stopped (postgres, redis)
    Tailscale: 5/6 online (atlas offline)
    Home: 3 lights on, 22.5C living room, alarm armed
  ```

### Req-4: financial_summary Tool

`financial_summary()` — Financial overview combining Plaid and Stripe.

- No input parameters
- Sources called in parallel:
  - `plaid_balances()` — account balances (checking, savings, credit)
  - `stripe_invoices("open")` — outstanding invoices
- Output: Single formatted block:
  ```
  Finances:
    Checking: $X,XXX.XX
    Savings: $XX,XXX.XX
    Credit: -$X,XXX.XX owed
    Stripe: 2 open invoices ($XXX.XX total)
  ```
- **PII safety**: Plaid data already filtered by plaid.rs before reaching aggregation. Aggregation layer receives SafeRow only.

### Req-5: Parallel Execution with Timeout

Each sub-call wrapped in `tokio::time::timeout(Duration::from_secs(5), ...)`:
- If a source times out or errors: include `"[source]: unavailable"` in output
- If ALL sources fail: return "All sources unavailable. Check individual tools."
- Log which sources succeeded/failed to audit log

### Req-6: Project-Resource Mapping

Hardcoded map in `aggregation.rs`:
```rust
struct ProjectResources {
    vercel_project: Option<&'static str>,
    sentry_slug: Option<&'static str>,
    jira_key: Option<&'static str>,
    github_repo: Option<&'static str>,
    neon_project_id: Option<&'static str>,
}
```
Not every project has every resource. Missing resources are skipped, not errored.

### Req-7: Tool Registration

Register all 3 tools in `register_tools()`.
Wire dispatch in `execute_tool()` to call AggregationService methods.

### Req-8: Audit Logging

Log each composite tool invocation: tool name, sources attempted, sources succeeded, total duration_ms.

## Scope
- **IN**: AggregationService module, project_health tool, homelab_status tool, financial_summary tool, parallel execution, project-resource mapping, tool registration
- **OUT**: Custom aggregation queries, cross-project comparisons, trend analysis over time, alerting/thresholds

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/aggregation.rs` | New: AggregationService with project_health(), homelab_status(), financial_summary() |
| `crates/nv-daemon/src/tools.rs` | Add 3 tool definitions + dispatch cases |
| `crates/nv-daemon/src/main.rs` | Init AggregationService with refs to all data source clients |

## Risks
| Risk | Mitigation |
|------|-----------|
| One slow source blocks all | Independent 5s timeout per source. Partial results returned. |
| Too many concurrent requests | Max ~6 parallel per composite call. Well within system limits. |
| Project mapping goes stale | Hardcoded is fine for single-user. Update map when projects change. |
| Output too long for Telegram | Cap per-dimension output. Telegram has 4096 char message limit. |
| Circular dependency on individual tools | Aggregation calls client methods directly, not via execute_tool. No recursion risk. |
