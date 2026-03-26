# Implementation Tasks

<!-- beads:epic:nv-0zot -->

## UI Batch

- [x] [3.1] [P-0] Create reusable ErrorBoundary component at components/layout/ErrorBoundary.tsx with fallback UI, retry action, and "Show details" toggle [owner:ui-engineer] [beads:nv-fcz9]
- [x] [3.2] [P-0] Make BriefingGetResponse.entry nullable in types/api.ts and add null guards for sources_status, suggested_actions, content in briefing/page.tsx [owner:ui-engineer] [beads:nv-54rg]
- [x] [3.3] [P-0] Wrap briefing content area (sections + sources + actions) in ErrorBoundary, keeping header and history rail outside the boundary [owner:ui-engineer] [beads:nv-k1ce]

## E2E Batch

- [x] [4.1] Verify briefing page renders empty state when daemon is unreachable without crashing [owner:e2e-engineer] [beads:nv-h9yz]
