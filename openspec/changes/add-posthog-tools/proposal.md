# Proposal: Add PostHog Tools

## Change ID
`add-posthog-tools`

## Summary

PostHog product analytics via REST API (app.posthog.com or eu.posthog.com). Two tools exposing
event trends and feature flag status per project — read-only, authenticated via Personal API key,
formatted for Telegram delivery.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions), `crates/nv-daemon/src/agent.rs` (tool dispatch)
- Related: PRD Phase 2 "Data Sources — API Key" (Tier 2), `add-tool-audit-log` spec (audit logging dependency)
- Auth: Personal API key via `POSTHOG_API_KEY` env var, project ID via `POSTHOG_PROJECT_ID` env var

## Motivation

PostHog tracks user behavior, feature flags, and experiments across deployed projects. Currently
checking analytics requires opening the PostHog dashboard. Wiring PostHog into Nova enables:

1. **Quick analytics** — "How many signups on OO this week?" returns trend data
2. **Feature flag status** — "What flags are active on TC?" shows rollout percentages
3. **Aggregation layer input** — `project_health(code)` can include activity metrics
4. **Proactive digest** — "OO: 42 signups today (+15% WoW) | 3 flags active"

## Requirements

### Req-1: posthog_trends Tool

```
posthog_trends(project: String, event: String) -> TrendResult
```

REST call: `POST https://app.posthog.com/api/projects/{project_id}/insights/trend/`

Body:
```json
{
  "events": [{"id": "{event}", "type": "events"}],
  "date_from": "-7d",
  "interval": "day"
}
```

Returns daily counts for the last 7 days with: event name, daily values, total,
day-over-day trend direction. Format for Telegram as a condensed sparkline or
daily breakdown.

Project mapping: maintain a config map from project code (oo, tc) to PostHog project ID.

### Req-2: posthog_flags Tool

```
posthog_flags(project: String) -> Vec<FeatureFlag>
```

REST call: `GET https://app.posthog.com/api/projects/{project_id}/feature_flags/?limit=50`

Returns active feature flags with: key, name, active status, rollout percentage,
filters summary. Only include flags where `active == true`. Format for Telegram
as a condensed list with rollout percentage.

### Req-3: HTTP Client

Use `reqwest` with:
- `Authorization: Bearer {POSTHOG_API_KEY}` header on all requests (Personal API key)
- 15s request timeout
- JSON response parsing into typed structs
- Error mapping: 401 -> "PostHog API key invalid", 404 -> "Project not found"
- Base URL configurable (app.posthog.com vs eu.posthog.com) via `POSTHOG_HOST` env var

### Req-4: Tool Registration

Both tools registered in `tools.rs` with:
- Tool name and description for Claude's tool-use schema
- Input validation (project code resolves to known project ID, event name non-empty)
- Error handling for missing POSTHOG_API_KEY env var
- Audit logging via tool_usage table

## Scope
- **IN**: Two read-only tools (trends, flags), REST API client, project code mapping, error handling, audit logging
- **OUT**: Event ingestion, flag creation/modification, experiment management, cohort management, session recording playback

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools.rs` | Add posthog_trends, posthog_flags tool definitions + dispatch handlers |
| `crates/nv-daemon/src/agent.rs` | Register new tools in available_tools list |
| `crates/nv-daemon/src/posthog.rs` | New: PostHog module with HTTP client, typed structs, project ID mapping |
| `crates/nv-daemon/src/main.rs` | Add `mod posthog;` declaration |

## Risks
| Risk | Mitigation |
|------|-----------|
| POSTHOG_API_KEY not set | Return clear error: "POSTHOG_API_KEY env var not set" |
| Project code to ID mapping stale | Config file mapping, error message if code not found |
| Trends API response large | Request only 7-day window, single event per query |
| EU vs US hosting | POSTHOG_HOST env var, default to app.posthog.com |
