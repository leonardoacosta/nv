# Proposal: Add Sentry Tools

## Change ID
`add-sentry-tools`

## Summary

Sentry error tracking via REST API (sentry.io). Two tools exposing unresolved issues per
project and detailed issue information — read-only, authenticated via Bearer token,
formatted for Telegram delivery.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions), `crates/nv-daemon/src/agent.rs` (tool dispatch)
- Related: PRD Phase 2 "Data Sources — API Key" (Tier 2), `add-tool-audit-log` spec (audit logging dependency)
- Auth: Bearer token via `SENTRY_AUTH_TOKEN` env var (organization-scoped)

## Motivation

Sentry captures runtime errors across all deployed projects. Currently errors go unnoticed
until someone opens the Sentry dashboard. Wiring Sentry into Nova enables:

1. **Error awareness** — "Any new Sentry errors on OO?" surfaces unresolved issues
2. **Aggregation layer input** — `project_health(code)` needs error count (0 errors = green)
3. **Proactive digest** — "OO: 0 errors | TC: 2 new errors (TypeError in auth.ts)"
4. **Issue drill-down** — get stack trace and event count for a specific issue

## Requirements

### Req-1: sentry_issues Tool

```
sentry_issues(project: String) -> Vec<SentryIssueSummary>
```

REST call: `GET https://sentry.io/api/0/projects/{org}/{project}/issues/?query=is:unresolved&sort=date&limit=10`

Returns unresolved issues with: id, title (error message), culprit (file/function),
count (event occurrences), first seen, last seen, level (error/warning/info).
Format for Telegram with level indicator and event count.

Organization slug resolved from `SENTRY_ORG` env var (default: infer from token scope).

### Req-2: sentry_issue Tool

```
sentry_issue(id: String) -> SentryIssueDetail
```

REST call: `GET https://sentry.io/api/0/issues/{id}/`

Returns detailed issue with: title, culprit, count, first/last seen, status,
and latest event's stack trace (top 5 frames). Stack trace formatted as condensed
code-location list for Telegram readability.

Optionally fetch latest event: `GET https://sentry.io/api/0/issues/{id}/events/latest/`
for full stack trace with source context.

### Req-3: HTTP Client

Use `reqwest` with:
- `Authorization: Bearer {SENTRY_AUTH_TOKEN}` header on all requests
- 15s request timeout
- JSON response parsing into typed structs
- Error mapping: 401 -> "Sentry token expired", 403 -> "Token lacks project access", 404 -> "Project not found"

### Req-4: Tool Registration

Both tools registered in `tools.rs` with:
- Tool name and description for Claude's tool-use schema
- Input validation (project slug non-empty, issue ID numeric)
- Error handling for missing SENTRY_AUTH_TOKEN env var
- Audit logging via tool_usage table

## Scope
- **IN**: Two read-only tools (issues list, issue detail), REST API client, stack trace formatting, error handling, audit logging
- **OUT**: Issue resolution/assignment, alert rule management, release tracking, performance monitoring, replay sessions

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools.rs` | Add sentry_issues, sentry_issue tool definitions + dispatch handlers |
| `crates/nv-daemon/src/agent.rs` | Register new tools in available_tools list |
| `crates/nv-daemon/src/sentry.rs` | New: Sentry module with HTTP client, typed structs, stack trace formatter |
| `crates/nv-daemon/src/main.rs` | Add `mod sentry;` declaration |

## Risks
| Risk | Mitigation |
|------|-----------|
| SENTRY_AUTH_TOKEN not set | Return clear error: "SENTRY_AUTH_TOKEN env var not set" |
| Organization slug unknown | Require SENTRY_ORG env var, fail with clear message if missing |
| Stack traces very large | Truncate to top 5 frames, omit vendor/node_modules frames |
| Rate limiting (API quota varies) | Single-user scale; add retry-after header check if needed |
