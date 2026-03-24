# Proposal: Fix Dashboard API/Frontend Contract Mismatches

## Change ID
`fix-dashboard-contracts`

## Summary

The React SPA dashboard has 8 API/frontend contract mismatches that cause most pages to silently
display wrong data or silently fail saves. Every memory save returns 400, the settings save button
returns 400, integration config saves are a no-op, the projects count always shows 0, the sessions
list is always empty, server health metrics are all 0 or undefined, and the tool usage table is
always empty. The root cause is that the dashboard pages were written against an assumed API shape
that does not match the actual Axum handlers in `crates/nv-daemon/src/dashboard.rs`.

All fixes are frontend-only. The Axum handlers are correct and have passing unit tests. No backend
changes are needed. The work is a targeted correction of field names, response unwrapping, type
casts, and endpoint routing in six dashboard page files plus the `ServerHealth` component type.

## Context
- Extends: `dashboard/src/pages/MemoryPage.tsx`
- Extends: `dashboard/src/pages/SettingsPage.tsx`
- Extends: `dashboard/src/pages/DashboardPage.tsx`
- Extends: `dashboard/src/pages/IntegrationsPage.tsx`
- Extends: `dashboard/src/pages/NexusPage.tsx`
- Extends: `dashboard/src/pages/UsagePage.tsx`
- Extends: `dashboard/src/components/ServerHealth.tsx`
- Related: Full audit 2026-03-23 (dashboard domain, health 64/C)

## Motivation

Every non-trivial user action in the dashboard is broken. A user who opens the dashboard sees
zeros across the stats panel, an empty sessions list, and health metrics that never update. A user
who tries to save a memory entry, update config, or save an integration credential gets a silent 400
error (the error is shown in the UI but there is no indication that the request format is wrong).
The backend is fully functional — the mismatches are entirely in the frontend fetch calls and type
casts. Fixing them requires no migration, no data change, and no backend deployment.

## Requirements

### Req-1: Fix MemoryPage PUT body — `path` to `topic`

`MemoryPage.tsx:59` sends `{ path, content }`. The backend `PutMemoryRequest` at
`dashboard.rs:566` expects `{ topic, content }`. The `handleSave` callback receives the file's
`path` string from `MemoryPreview`, which is the topic name. The fix is to rename the sent field
from `path` to `topic` in the `JSON.stringify` call.

### Req-2: Fix MemoryPage GET — handle `{ topics: string[] }` response shape

`MemoryPage.tsx:29` checks for `raw.files` then falls through to `Object.entries(raw)`. The
backend returns `{ topics: string[] }` — a list of topic name strings with no content. The page
must detect the `topics` array and map each string to a `MemoryFile` with `name` and `path` set to
the topic string and `content` left empty (content is fetched on demand or left blank). The
`MemoryApiResponse` interface already declares `topics?: string[]`; the branch that handles it is
missing.

### Req-3: Fix SettingsPage PUT body — wrap in `{ fields: {...} }`

`SettingsPage.tsx:213` sends `JSON.stringify(config)` — the raw config object. The backend
`PutConfigRequest` at `dashboard.rs:638` expects `{ fields: {...} }` where the value is the flat
key-value map. The fix is to wrap the config: `JSON.stringify({ fields: config })`.

### Req-4: Fix DashboardPage GET /api/projects — unwrap `{ projects: [...] }` before counting

`DashboardPage.tsx:89` casts the projects response as `ApiProject[]` and calls `.length`. The
backend `get_projects` at `dashboard.rs:329` returns `{ projects: [...] }`. Calling `.length` on
a plain object returns `undefined`, so the count is always 0. The fix is to extract
`response.projects` before taking the length.

### Req-5: Fix DashboardPage GET /api/sessions — unwrap `{ sessions: [...] }` before use

`DashboardPage.tsx:92` casts the sessions response as a flat `Session[]`. Both the Nexus path
(`dashboard.rs:370`) and the fallback path (`dashboard.rs:396`) always return
`{ sessions: [...], ... }`. The flat-array cast produces an object where `.filter` and `.reduce`
are called on the wrapper, silently returning empty results. The fix is to extract
`response.sessions` after parsing.

### Req-6: Fix IntegrationsPage PUT body — send `{ fields: {...} }` instead of `{ integration_id, config }`

`IntegrationsPage.tsx:53` sends `{ integration_id: id, config }` to `PUT /api/config`. The backend
ignores unknown wrapper keys and expects `{ fields: {...} }` with flat top-level scalar keys. The
integration config object must be flattened into the `fields` value:
`JSON.stringify({ fields: config })`. The `integration_id` field has no corresponding backend
concept and should be dropped.

### Req-7: Fix NexusPage ServerHealth type — map `latest` snapshot fields

`NexusPage.tsx:94` casts the `/api/server-health` response directly as `HealthMetrics`. The backend
returns `{ daemon, latest, status, history }` where `latest` is a `ServerHealthSnapshot` with
fields `cpu_percent`, `memory_used_mb`, `memory_total_mb`, `uptime_seconds` (same names as
`HealthMetrics`). The `ServerHealth` component's `HealthMetrics` interface at
`components/ServerHealth.tsx:3` uses `status: "ok" | "degraded" | "down"` but the backend
`HealthStatus` enum serializes to `"healthy"`, `"degraded"`, `"critical"`. The fix has two parts:
(a) extract `data.latest` for the metrics and `data.status` separately; (b) add a mapping function
from backend status strings to the component's expected union.

### Req-8: Fix UsagePage — fetch `/stats` instead of `/api/sessions` for tool usage

`UsagePage.tsx:94` fetches `/api/sessions` and looks for `tool_stats` on each session object.
Sessions never include `tool_stats`. The actual tool usage data lives at the `/stats` endpoint
(served by `http.rs:83`) under the `tool_usage` key, which contains the `ToolStatsReport` shape
with a `per_tool` array of `{ tool_name, count, success_count, avg_duration_ms }` entries. The fix
is to change the fetch target to `/stats` and map `response.tool_usage.per_tool` to the page's
`ToolUsage[]` type.

## Scope
- **IN**: Field name corrections, response unwrapping, status string mapping, endpoint redirect for
  tool usage, TypeScript type definitions for all corrected response shapes
- **OUT**: Backend changes, new API endpoints, dashboard layout changes, error handling redesign

## Impact

| File | Change |
|------|--------|
| `dashboard/src/pages/MemoryPage.tsx` | PUT field rename (`path` → `topic`); GET branch for `topics` array |
| `dashboard/src/pages/SettingsPage.tsx` | Wrap PUT body in `{ fields: config }` |
| `dashboard/src/pages/DashboardPage.tsx` | Unwrap `projects` and `sessions` arrays from response objects |
| `dashboard/src/pages/IntegrationsPage.tsx` | Replace PUT body with `{ fields: config }` |
| `dashboard/src/pages/NexusPage.tsx` | Extract `latest` and map `status` from server-health response |
| `dashboard/src/pages/UsagePage.tsx` | Fetch `/stats`, read `tool_usage.per_tool` |
| `dashboard/src/components/ServerHealth.tsx` | Expand `status` union to include backend values |
| `dashboard/src/types/api.ts` (new) | Canonical response type definitions for all API shapes |

## Risks

| Risk | Mitigation |
|------|-----------|
| MemoryPage GET: topics are names only, no content pre-loaded | Show topic name in list; content pane stays empty until a dedicated GET /api/memory/:topic is added — acceptable for now |
| `/stats` endpoint is on the base HTTP server, not the dashboard router — may have CORS or path prefix difference | Verify at runtime; if path differs, use relative `/stats` same as the existing dashboard API calls |
| IntegrationsPage config keys may not match the flat scalar keys the backend accepts | Backend validates against existing top-level config keys — non-matching keys are silently ignored, which is the current behavior anyway |
