# Proposal: Add Vercel Tools

## Change ID
`add-vercel-tools`

## Summary

Vercel deployment status via REST API (api.vercel.com). Two tools exposing recent deployments
per project and deployment-specific build logs — read-only, authenticated via Vercel API token,
formatted for Telegram delivery.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions), `crates/nv-daemon/src/agent.rs` (tool dispatch)
- Related: PRD Phase 2 "Data Sources — API Key" (Tier 2), `add-tool-audit-log` spec (audit logging dependency)
- Auth: Bearer token via `VERCEL_TOKEN` env var

## Motivation

All T3 Turbo projects (oo, tc, tl, mv, ss) deploy via Vercel. Deploy failures are currently
invisible until someone checks the dashboard. Wiring Vercel into Nova enables:

1. **Instant deploy status** — "What's the latest deploy on OO?" returns state + URL
2. **Build log access** — failed deploys surface the error without opening Vercel dashboard
3. **Aggregation layer input** — `project_health(code)` needs deploy status (green/red)
4. **Proactive digest** — "OO: deployed 22m ago | TC: deploy failed (type error)"

## Requirements

### Req-1: vercel_deployments Tool

```
vercel_deployments(project: String) -> Vec<DeploymentSummary>
```

REST call: `GET https://api.vercel.com/v6/deployments?projectId={project}&limit=10`

Returns recent deployments with: id, state (READY/ERROR/BUILDING/QUEUED/CANCELED),
URL, git branch, git commit message (truncated), created timestamp, and ready timestamp.
Format for Telegram with state emoji (READY, ERROR, BUILDING).

Project can be name or ID — try name first via `GET /v9/projects/{name}` to resolve ID.

### Req-2: vercel_logs Tool

```
vercel_logs(deploy_id: String) -> BuildLog
```

REST call: `GET https://api.vercel.com/v2/deployments/{deploy_id}/events`

Returns build log events filtered to errors and warnings. Full build log can be large;
truncate to last 50 lines if >50 events. Highlight error lines for Telegram output.

### Req-3: HTTP Client

Use `reqwest` (already in workspace deps) with:
- `Authorization: Bearer {VERCEL_TOKEN}` header on all requests
- 15s request timeout
- JSON response parsing into typed structs
- Error mapping: 401 -> "Vercel token expired", 404 -> "Project not found", 429 -> "Rate limited"

### Req-4: Tool Registration

Both tools registered in `tools.rs` with:
- Tool name and description for Claude's tool-use schema
- Input validation (project name non-empty, deploy_id non-empty)
- Error handling for missing VERCEL_TOKEN env var
- Audit logging via tool_usage table

## Scope
- **IN**: Two read-only tools (deployments, logs), REST API client, error handling, audit logging, Telegram formatting
- **OUT**: Deploy triggers, project creation, domain management, environment variable management, team management

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools.rs` | Add vercel_deployments, vercel_logs tool definitions + dispatch handlers |
| `crates/nv-daemon/src/agent.rs` | Register new tools in available_tools list |
| `crates/nv-daemon/src/vercel.rs` | New: Vercel module with HTTP client, typed structs, formatters |
| `crates/nv-daemon/src/main.rs` | Add `mod vercel;` declaration |

## Risks
| Risk | Mitigation |
|------|-----------|
| VERCEL_TOKEN not set | Return clear error: "VERCEL_TOKEN env var not set" |
| Rate limiting (100 req/hr on free plan) | Single-user scale unlikely to hit; add retry-after header check |
| Build logs very large | Truncate to last 50 events, filter to errors/warnings |
| Project name vs ID ambiguity | Resolve name to ID via projects endpoint, cache mapping in memory |
