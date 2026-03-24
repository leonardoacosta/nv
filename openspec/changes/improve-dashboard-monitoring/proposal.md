# Proposal: Improve Dashboard Monitoring

## Change ID
`improve-dashboard-monitoring`

## Depends On
`add-server-health-metrics`

## Summary

Enhance the Nova dashboard with real server health metrics: health cards on the Nexus page showing
live CPU, memory, disk, and uptime data from the `/api/server-health` endpoint; status indicators
on the main Dashboard page; and 7-day mini charts for key metrics. All pages use types from
`dashboard/src/types/api.ts`.

## Context
- Extends: `dashboard/src/pages/NexusPage.tsx`, `dashboard/src/pages/DashboardPage.tsx`, `dashboard/src/components/ServerHealth.tsx`
- Related: `add-server-health-metrics` (provides the API endpoint and data), `dashboard/src/types/api.ts` (canonical types)
- Depends on: `add-server-health-metrics` (API must exist before the dashboard can consume it)

## Motivation

The dashboard currently shows daemon health with basic CPU and memory bars in the
`ServerHealth` component on the Nexus page, but:

1. **No real metric data on DashboardPage** -- the main overview page shows session counts and
   obligations but has no server health indicator; operators must navigate to Nexus to check
   health status
2. **No historical charts** -- the `ServerHealth` component shows only the latest snapshot with
   no trend visibility; gradual degradation (memory leaks, disk filling) is invisible
3. **Type drift risk** -- some components define inline types instead of importing from
   `dashboard/src/types/api.ts`, creating maintenance burden when the API evolves

This spec adds health awareness across the dashboard so the operator sees status at a glance
on any page.

## Requirements

### Req-1: Health Cards Component

Enhance `dashboard/src/components/ServerHealth.tsx` to accept optional `history` data and render
mini charts alongside the existing metric bars:

- Accept `history: ServerHealthSnapshot[]` prop (from API `history` field)
- When history is available, render inline sparkline/mini charts below each metric bar
  showing 7-day trend (CPU %, memory %, disk %)
- Charts should be minimal (no axis labels, just the line/area) to fit the existing card layout
- Use the existing cosmic theme colors (green for healthy ranges, orange for degraded, red for critical)
- Retain the current loading/error/empty states

### Req-2: NexusPage Health Cards with Real Metrics

Update `dashboard/src/pages/NexusPage.tsx`:

- Continue fetching from `/api/server-health` (already implemented)
- Pass the `history` array from the API response to `ServerHealth` component
- Add disk usage metric display (currently only CPU and memory are shown)
- Add load average display (1m and 5m from `ServerHealthSnapshot`)
- Ensure all types are imported from `dashboard/src/types/api.ts` (remove any inline type
  definitions that duplicate API types)

### Req-3: DashboardPage Health Status Indicator

Update `dashboard/src/pages/DashboardPage.tsx`:

- Fetch `/api/server-health` alongside the existing `/api/obligations`, `/api/projects`,
  `/api/sessions` calls in `fetchData()`
- Add a health status `StatCard` to the summary grid showing overall status
  (healthy/degraded/critical) with appropriate color coding
- Add a CPU and memory summary card showing current values
- Use types from `dashboard/src/types/api.ts` (`ServerHealthGetResponse`, `BackendHealthStatus`)

### Req-4: Type Alignment

Ensure all dashboard pages import types exclusively from `dashboard/src/types/api.ts`:

- `NexusPage.tsx` -- use `ServerHealthGetResponse`, `ServerHealthSnapshot`, `BackendHealthStatus`
  from `@/types/api` (already partially done, verify completeness)
- `DashboardPage.tsx` -- add imports for `ServerHealthGetResponse` and `BackendHealthStatus`
- `ServerHealth.tsx` -- update `HealthMetrics` interface to align with or extend
  `ServerHealthSnapshot` from `@/types/api` rather than defining a parallel type
- No inline API response types; if a new shape is needed, add it to `api.ts` first

### Req-5: Mini Charts for Key Metrics

Add a lightweight mini chart component for rendering 7-day metric trends:

- New component `dashboard/src/components/MiniChart.tsx` or inline within `ServerHealth.tsx`
- Accepts an array of numeric values and renders as an SVG sparkline
- Supports configurable warn/critical thresholds for color bands
- No external charting library -- use raw SVG path for minimal bundle impact
- Chart dimensions: ~120px wide, ~32px tall (fits inside metric card rows)

## Scope
- **IN**: ServerHealth component enhancement, NexusPage metric expansion, DashboardPage health
  indicator, type alignment across pages, mini chart component
- **OUT**: Real-time WebSocket updates (polling is sufficient), alert configuration UI,
  historical data beyond 7 days, mobile responsive layout

## Impact
| Area | Change |
|------|--------|
| `dashboard/src/components/ServerHealth.tsx` | Add history prop, mini charts, disk/load display |
| `dashboard/src/components/MiniChart.tsx` (new) | SVG sparkline component for metric trends |
| `dashboard/src/pages/NexusPage.tsx` | Pass history to ServerHealth, add disk + load metrics |
| `dashboard/src/pages/DashboardPage.tsx` | Fetch server-health, add health status + CPU/memory cards |
| `dashboard/src/types/api.ts` | Verify completeness, no changes expected (types already defined) |

## Risks
| Risk | Mitigation |
|------|-----------|
| SVG sparkline rendering performance with 1440 data points | Downsample to ~120 points (one per hour for 5 days) before rendering |
| Type mismatch between ServerHealth HealthMetrics and api.ts ServerHealthSnapshot | Refactor HealthMetrics to extend/wrap ServerHealthSnapshot |
| Additional fetch on DashboardPage increases load time | Use Promise.allSettled (already in place), health fetch is fast (~5ms) |
| Mini chart visual clutter in small cards | Keep charts minimal (no labels, subtle line), hide on narrow viewports |
