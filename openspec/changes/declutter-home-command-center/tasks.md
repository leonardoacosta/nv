# Implementation Tasks

<!-- beads:epic:nv-pgus -->

## API Batch

- [ ] [1.1] [P-1] Add `ActionItem` and `CategorySummary` types to `apps/dashboard/types/api.ts` -- `ActionItem: { id: string; severity: "error" | "warning"; category: "obligation" | "message" | "automation"; summary: string; link: string }`, `CategorySummary: { type: "message" | "obligation" | "session" | "system"; count: number; summaryText: string; latestTimestamp: string; items: ActivityFeedEvent[] }` [owner:api-engineer] [beads:nv-zpxt]

## UI Batch

- [ ] [2.1] [P-1] Create `ActionItems` component in `page.tsx` -- derives action items from existing obligation, message, and automation queries: pending obligations (status "open"/"in_progress") as warning items linking to /obligations, unread messages (inbound, last 4h, sender !== "nova") as warning items linking to /messages, failed/overdue automations as error items linking to /automations; renders severity dot + category label + summary + ArrowRight link per row; collapses to "All clear" muted line when empty; shows max 10 with "N more" expand toggle; uses `divide-y divide-ds-gray-400` row separators [owner:ui-engineer] [beads:nv-gj7f]
- [ ] [2.2] [P-1] Create `NovaStatus` component in `page.tsx` -- horizontal row with three data cells separated by `border-r border-ds-gray-400`: (1) connected channels from `system.fleetStatus` showing channel name + green/red dot per channel status, (2) watcher state from `automation.getAll` showing enabled/disabled + interval, (3) last briefing from `briefing.latest` showing relative time or "None"; uses `font-mono` for data values, `text-label-12` for labels; bordered container `border border-ds-gray-400 rounded-lg` [owner:ui-engineer] [beads:nv-d0vb]
- [ ] [2.3] [P-1] Create `GroupedActivitySummaries` component in `page.tsx` -- groups `ActivityFeedEvent[]` by type, computes per-group summary text (e.g., "8 messages -- 3 inbound, 5 outbound"), orders groups by most recent event timestamp; renders each group as: summary header row (icon + category name + count badge + summary text + "View all" link) followed by 3 most recent events in compact row format; groups with 0 events hidden [owner:ui-engineer] [beads:nv-uv5f]
- [ ] [2.4] [P-1] Add progressive disclosure to `GroupedActivitySummaries` -- each group starts collapsed (header + 3 items); clicking header or "Show more" expands to show all events (up to 50); only one group expanded at a time (expanding collapses previously expanded); expanded view reuses existing dense row format with severity coloring and expandable detail panels from current `ActivityFeedSection` [owner:ui-engineer] [beads:nv-5nb6]
- [ ] [2.5] [P-2] Add WebSocket badge counters -- modify `useDaemonEvents` handler to increment per-category `newEventCounts` state instead of prepending raw WS events to feed; render subtle "N new" badge on category summary headers when count > 0; clicking badge calls `queryClient.invalidateQueries` for the activity feed query and resets that category's counter [owner:ui-engineer] [beads:nv-asos]
- [ ] [2.6] [P-2] Create collapsible `QuickAdd` row -- replace inline ObligationBar with a collapsed row showing "+" icon button; clicking expands to reveal the existing obligation input + submit UI; auto-collapses after successful creation with brief "Created" confirmation; placed below activity summaries [owner:ui-engineer] [beads:nv-r2na]
- [ ] [2.7] [P-1] Refactor `DashboardPage` main layout -- remove PriorityBanner, StatStrip, CcSessionsWidget, CategoryPills, and standalone RecentConversations section; compose new layout top-to-bottom: page header (keep existing refresh controls), ActionItems, NovaStatus, GroupedActivitySummaries, QuickAdd; use `border-b border-ds-gray-400` between sections instead of `gap-4`; keep `useDaemonStatus` disconnect overlay and auto-refresh toggle [owner:ui-engineer] [beads:nv-vazb]
- [ ] [2.8] [P-2] Clean up dead code in `page.tsx` -- remove unused `PriorityBanner`, `CategoryPills`, `getCategoryCount`, `PILL_LABELS`, `FeedCategory` type, old `ActivityFeedSection` component (replaced by GroupedActivitySummaries), `RecentConversations` component (absorbed into messages group), `CcSessionsWidget`, `StatStrip` import and `statCells` computation; verify no other pages import these from page.tsx (they are all local to the file) [owner:ui-engineer] [beads:nv-z0ec]

## Verify

- [ ] [3.1] `cd apps/dashboard && pnpm typecheck` passes -- zero TypeScript errors [owner:ui-engineer] [beads:nv-5ek1]
- [ ] [3.2] `cd apps/dashboard && pnpm build` passes -- production build succeeds [owner:ui-engineer] [beads:nv-dirp]
- [ ] [3.3] [user] Manual test: Action Items panel shows pending obligations and unread messages when they exist, shows "All clear" when empty [owner:ui-engineer]
- [ ] [3.4] [user] Manual test: Nova's Status shows connected channels with dots, watcher state, and last briefing time [owner:ui-engineer]
- [ ] [3.5] [user] Manual test: Activity summaries show grouped categories with correct counts and summary text; expanding a group shows all events; expanding another collapses the first [owner:ui-engineer]
- [ ] [3.6] [user] Manual test: WebSocket events increment badge counters on category headers instead of prepending raw events [owner:ui-engineer]
- [ ] [3.7] [user] Manual test: Quick Add "+" expands obligation input, successful creation collapses it [owner:ui-engineer]
- [ ] [3.8] [user] Manual test: Page layout is single-column with section borders, no StatStrip or CcSessionsWidget visible, Geist dark theme maintained [owner:ui-engineer]
