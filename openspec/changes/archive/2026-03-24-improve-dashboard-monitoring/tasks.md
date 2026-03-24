# Implementation Tasks

<!-- beads:epic:TBD -->

## Health Cards Component

- [x] [1.1] [P-1] Update ServerHealth.tsx to accept optional history: ServerHealthSnapshot[] prop from @/types/api [owner:ui-engineer]
- [x] [1.2] [P-2] Add disk usage metric bar (disk_used_gb / disk_total_gb) below existing CPU and memory bars [owner:ui-engineer]
- [x] [1.3] [P-2] Add load average display (1m and 5m) in the info row section [owner:ui-engineer]
- [x] [1.4] [P-2] Refactor HealthMetrics interface to align with ServerHealthSnapshot from @/types/api -- extend or replace to eliminate parallel type definitions [owner:ui-engineer]

## Mini Charts

- [x] [2.1] [P-2] Create dashboard/src/components/MiniChart.tsx -- SVG sparkline component accepting data: number[], width (default 120), height (default 32), warnThreshold, critThreshold [owner:ui-engineer]
- [x] [2.2] [P-2] Implement SVG path rendering -- polyline from data points, scaled to component dimensions, use cosmic theme colors [owner:ui-engineer]
- [x] [2.3] [P-3] Add threshold color bands -- subtle background regions for warn (orange) and critical (red) zones [owner:ui-engineer]
- [x] [2.4] [P-2] Integrate MiniChart into ServerHealth.tsx -- render below CPU, memory, and disk bars when history data is available [owner:ui-engineer]
- [x] [2.5] [P-3] Add data downsampling utility -- reduce 1440-point history to ~120 points for rendering performance [owner:ui-engineer]

## Metrics Display (NexusPage)

- [x] [3.1] [P-1] Update NexusPage.tsx fetchHealth() to pass history array from ServerHealthGetResponse to ServerHealth component [owner:ui-engineer]
- [x] [3.2] [P-2] Map disk_used_gb and disk_total_gb from API response into ServerHealth metrics [owner:ui-engineer]
- [x] [3.3] [P-2] Map load_avg_1m and load_avg_5m from API response into ServerHealth display [owner:ui-engineer]
- [x] [3.4] [P-2] Remove any inline type definitions that duplicate types from @/types/api.ts [owner:ui-engineer]

## Dashboard Health Status

- [x] [4.1] [P-1] Add /api/server-health fetch to DashboardPage.tsx fetchData() using Promise.allSettled alongside existing API calls [owner:ui-engineer]
- [x] [4.2] [P-1] Add health status StatCard showing overall status (healthy/degraded/critical) with color-coded icon [owner:ui-engineer]
- [x] [4.3] [P-2] Add CPU percentage StatCard showing current cpu_percent from latest snapshot [owner:ui-engineer]
- [x] [4.4] [P-2] Add memory usage StatCard showing current memory_used_mb / memory_total_mb [owner:ui-engineer]
- [x] [4.5] [P-2] Import ServerHealthGetResponse and BackendHealthStatus from @/types/api.ts -- no inline API types [owner:ui-engineer]

## Type Alignment

- [x] [5.1] [P-1] Verify dashboard/src/types/api.ts has complete ServerHealthSnapshot (including disk_used_gb, disk_total_gb fields if missing) [owner:ui-engineer]
- [x] [5.2] [P-2] Update all pages to import API response types exclusively from @/types/api.ts -- audit NexusPage, DashboardPage, ServerHealth for inline duplicates [owner:ui-engineer]

## Verify

- [x] [6.1] pnpm build -- dashboard compiles cleanly with no TypeScript errors [owner:ui-engineer]
- [x] [6.2] pnpm lint -- no new lint warnings across changed files [owner:ui-engineer]
- [x] [6.3] [user] Visual verification: NexusPage shows CPU, memory, disk bars with mini charts when history data available [owner:ui-engineer]
- [x] [6.4] [user] Visual verification: DashboardPage shows health status, CPU, and memory cards in summary grid [owner:ui-engineer]
