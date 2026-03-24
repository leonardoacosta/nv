---
name: audit:tools
description: Audit external service tool integrations — 20 tools across GitHub, Jira, Vercel, Sentry, etc.
type: command
execution: foreground
---

# Audit: Tools

Audit the 20 external service integrations exposed as agent tools.

## Scope

| Tool | File | Service | Key Methods |
|------|------|---------|-------------|
| GitHub | `tools/github.rs` (50.8K) | GitHub REST (via `gh` CLI) | `get_pr_summary`, `get_run_summary`, `get_issues` |
| Neon | `tools/neon.rs` (34.3K) | Neon Serverless DB | `list_projects`, `list_branches` |
| Check | `tools/check.rs` (33.1K) | Multi-service probes | Batched `Checkable` implementations |
| Sentry | `tools/sentry.rs` (29.5K) | Sentry API | `get_projects`, `get_issues`, `get_transaction` |
| Vercel | `tools/vercel.rs` (26.9K) | Vercel REST API | `get_deployments`, `get_logs` |
| Stripe | `tools/stripe.rs` (25.2K) | Stripe API | Invoice/customer queries |
| Web | `tools/web.rs` (24.6K) | HTTP GET | Fetch + parse metadata |
| Doppler | `tools/doppler.rs` (20.7K) | Doppler API | Secret fetching |
| CloudFlare | `tools/cloudflare.rs` (19.4K) | CloudFlare API | DNS, Workers, Analytics |
| Upstash | `tools/upstash.rs` (19.3K) | Upstash Redis/QStash | Queue/cache ops |
| Teams | `tools/teams.rs` (19.1K) | MS Graph Teams | Direct collaboration |
| Plaid | `tools/plaid.rs` (17.0K) | Plaid Fintech | Financial data |
| PostHog | `tools/posthog.rs` (16.7K) | PostHog Analytics | Feature flags, events |
| HA | `tools/ha.rs` (16.8K) | Home Assistant | Entity state queries |
| ADO | `tools/ado.rs` (16.8K) | Azure DevOps | Work items, pipelines |
| Resend | `tools/resend.rs` (16.2K) | Resend Email | Email delivery |
| Calendar | `tools/calendar.rs` (21.3K) | Google Calendar | Event queries |
| Docker | `tools/docker.rs` (11.6K) | Docker Daemon | Container/image queries |
| Schedule | `tools/schedule.rs` (14.9K) | Local cron store | `add_schedule`, `list_schedules` |
| Jira | `tools/jira/` | Jira REST API | `get_issue`, `create_issue`, `transition_issue` |
| Mod | `tools/mod.rs` (170K) | Tool registry | Tool definitions, dispatch |

## Architecture

- `Checkable` trait: async `check_read()` + optional `check_write()` for health probes
- `ServiceRegistry<T>`: Multi-instance client resolution by project code with fallback chain
- `CheckResult` enum: Healthy, Degraded, Unhealthy, Missing
- Tool timeouts: 30s read, 60s write

## Audit Checklist

### Per-Tool
- [ ] Error handling (network failures, auth expiry, rate limits)
- [ ] Timeout enforcement (30s read, 60s write)
- [ ] `Checkable` implementation correctness
- [ ] ServiceRegistry fallback chain (project → "default" → first)
- [ ] Secret handling (no hardcoded tokens, env var sourcing)

### Tool Registry (mod.rs — 170K)
- [ ] Tool definitions match actual implementations
- [ ] Tool dispatch routing covers all registered tools
- [ ] Read vs Write tool classification accuracy
- [ ] Tool result formatting for Claude consumption
- [ ] Missing tools (registered but unimplemented, or vice versa)

### Jira (Modular)
- [ ] Multi-instance support (`JiraRegistry`)
- [ ] Webhook handler (`tools/jira/webhooks.rs`) — event types covered
- [ ] Issue lifecycle: create → assign → transition → comment
- [ ] Field mapping correctness

### Cross-Cutting
- [ ] Consistent error type usage across tools
- [ ] HTTP client reuse (shared reqwest::Client or per-tool?)
- [ ] Retry logic consistency
- [ ] Response size limits (prevent token explosion from large tool results)

## Memory

Persist findings to: `.claude/audit/memory/tools-memory.md`

## Findings

Log to: `~/.claude/scripts/state/nv-audit-findings.jsonl`
