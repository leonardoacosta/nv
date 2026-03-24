# Context: Fix Dashboard API/Frontend Contract Mismatches

## Source: Audit 2026-03-23 (dashboard domain, 64/C health)

## Problem
The React SPA dashboard has 8 API/frontend contract mismatches causing most pages to silently display wrong data or fail saves.

## Findings

### P1 — PUT /api/memory field name mismatch
- `dashboard/src/pages/MemoryPage.tsx:59` sends `{ path, content }`
- Backend `PutMemoryRequest` at `crates/nv-daemon/src/dashboard.rs:566` expects `{ topic, content }`
- Every memory save fails with 400 "topic must not be empty"

### P1 — GET /api/memory response shape mismatch
- Backend returns `{ topics: string[] }` (list of topic names)
- `MemoryPage.tsx:27` checks for `raw.files`, falls through to `Object.entries(raw)` producing a single synthetic entry
- Memory page shows one file called "topics" instead of real file list

### P1 — PUT /api/config wrapper object missing
- `dashboard/src/pages/SettingsPage.tsx:213` sends `JSON.stringify(config)` (raw config)
- Backend `PutConfigRequest` at `dashboard.rs:638` expects `{ fields: {...} }`
- Settings save button returns 400

### P1 — GET /api/obligations + /api/projects counts always 0
- `DashboardPage.tsx:84,90` casts responses as `ApiObligation[]` and `ApiProject[]`
- Backend returns wrapped `{ obligations: [...] }` and `{ projects: [...] }`
- `.length` on wrapper object is `undefined`, counts display as 0

### P1 — GET /api/sessions always empty
- `DashboardPage.tsx:93` casts response as flat `Session[]`
- Backend returns `{ sessions: [...], uptime_secs, ... }`
- Sessions list always empty, active_sessions always 0

### P1 — IntegrationsPage save format mismatch
- `IntegrationsPage.tsx:53` sends `{ integration_id, config }` to PUT /api/config
- Server expects `{ fields: {...} }` with flat top-level keys
- Integration config saves are a no-op

### P2 — NexusPage HealthMetrics type mismatch
- `ServerHealth` expects `{ cpu_percent, memory_used_mb, memory_total_mb, uptime_seconds, status }`
- Actual `/api/server-health` returns `{ daemon: {...}, latest: { cpu_pct, mem_used_mb, ... }, status, history }`
- All metrics display as 0 or undefined

### P3 — UsagePage reads wrong endpoint
- `UsagePage.tsx:103` fetches `/api/sessions` and looks for `tool_stats` on each session
- Sessions never include `tool_stats`; `/stats` endpoint has real `tool_usage` data
- Tool usage table always empty

## Files to Modify
- `dashboard/src/pages/MemoryPage.tsx`
- `dashboard/src/pages/SettingsPage.tsx`
- `dashboard/src/pages/DashboardPage.tsx`
- `dashboard/src/pages/IntegrationsPage.tsx`
- `dashboard/src/pages/NexusPage.tsx`
- `dashboard/src/pages/UsagePage.tsx`
- `dashboard/src/types/` (type definitions)
