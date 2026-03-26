# Implementation Tasks
<!-- beads:epic:nv-2jtb -->

## UI Batch

- [ ] [1.1] [P-1] Refactor `apps/dashboard/components/ObligationItem.tsx` — replace the always-visible Cancel/Confirm Done/Reopen buttons with a `getActionsForStatus(status)` helper that returns only the valid icon buttons (`Check`, `X`, `RotateCcw` from Lucide) with tooltips based on obligation status: open/in_progress shows Done+Cancel, proposed_done shows Confirm+Reopen, done shows Reopen, dismissed shows nothing [owner:ui-engineer] [beads:nv-e9ls]
- [ ] [1.2] [P-1] Add deadline proximity indicator to `ObligationItem.tsx` — compute proximity from `obligation.deadline` and `approaching_deadline_hours` config (default 24h); apply amber border/glow when within threshold, red solid indicator when overdue, no indicator otherwise; use Tailwind ring/border utilities with conditional classes [owner:ui-engineer] [beads:nv-klig]
- [ ] [1.3] [P-1] Add inline expand/collapse to `ObligationItem.tsx` — hide obligation details by default behind a collapsible container with CSS `grid-template-rows` transition (150-200ms ease); track expanded IDs in parent state via `Set<string>`; expand first item by default on mount [owner:ui-engineer] [beads:nv-anql]
- [ ] [1.4] [P-2] Refactor `apps/dashboard/app/obligations/page.tsx` — replace the 5 stat cards with a single compact summary bar component showing status counts as colored inline badges (e.g., "3 Open | 2 In Progress | ..."); reduce vertical stat area from ~120px to ~40px [owner:ui-engineer] [beads:nv-j1ju]
- [ ] [1.5] [P-2] Extract `ObligationSummaryBar` component — receives obligation array, computes counts by status, renders as a horizontal flex bar with colored count badges; exported from `apps/dashboard/components/ObligationSummaryBar.tsx` [owner:ui-engineer] [beads:nv-8s5g]
- [ ] [1.6] [P-2] Add `approaching_deadline_hours` to the dashboard API response contract if not already present — ensure the obligations page receives this config value from the daemon; fall back to 24 if absent [owner:ui-engineer] [beads:nv-ozf3]

## Verify

- [ ] [2.1] `cd apps/dashboard && pnpm typecheck` passes — zero TypeScript errors [owner:ui-engineer] [beads:nv-m36h]
- [ ] [2.2] [user] Manual test: verify open obligation shows only Done + Cancel icon buttons; completed shows only Reopen [owner:ui-engineer] [beads:nv-gzoc]
- [ ] [2.3] [user] Manual test: create obligation with deadline 2 hours from now; verify amber indicator appears; set deadline to past; verify red indicator [owner:ui-engineer] [beads:nv-qui1]
- [ ] [2.4] [user] Manual test: verify obligation details are collapsed by default except first item; click to expand/collapse with smooth animation [owner:ui-engineer] [beads:nv-svu6]
- [ ] [2.5] [user] Manual test: verify summary bar shows correct counts and replaces the 5 stat cards [owner:ui-engineer] [beads:nv-8cxr]
