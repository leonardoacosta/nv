# Implementation Tasks
<!-- beads:epic:TBD -->

## API Batch

- [x] [1.1] [P-1] Update `apps/dashboard/types/api.ts` -- add `severity: "error" | "warning" | "info"` field to `ActivityFeedEvent` interface; add `"session"` to the `type` union (`type: "message" | "obligation" | "diary" | "session"`) [owner:api-engineer]
- [x] [1.2] [P-1] Update `apps/dashboard/app/api/activity-feed/route.ts` -- add sessions query: select from `sessions` table where `startedAt >= twentyFourHoursAgo`, map each to `ActivityFeedEvent` with type `"session"`, icon_hint `"Activity"`, and summary `"Session started: {project} ({agent_name})"` or `"Session completed: {project} ({duration})"` based on status; assign severity to all events: obligation events with "failed" or "error" in status get `"error"`, obligation events with status "open" or "detected" get `"warning"`, all others get `"info"` [owner:api-engineer]

## UI Batch 1 -- Stat Strip Component

- [ ] [2.1] [P-1] Create `apps/dashboard/components/StatStrip.tsx` -- a horizontal flex row of stat cells separated by `border-r border-ds-gray-400`; each cell renders: icon (14px), label (`text-label-12 text-ds-gray-700`), value (`text-heading-16 text-ds-gray-1000 font-mono tabular-nums`), and optional sublabel (`text-label-12 text-ds-gray-700`); the row uses `flex flex-wrap gap-y-2` for mobile wrapping; each cell has `flex-1 min-w-[140px] px-4 py-2`; last cell has no right border [owner:ui-engineer]
- [ ] [2.2] [P-2] Wire stat strip data in `apps/dashboard/app/page.tsx` -- add `/api/fleet-status` and `/api/sessions` to the existing `Promise.allSettled` fetch batch; derive 5 stat values: (1) unread messages = count of messages where sender !== "nova" from last 4 hours, with channel breakdown, (2) pending obligations = existing `pendingObligations.length` split by owner, (3) fleet health = `fleet.healthy_count` / `fleet.total_count` from fleet-status response, (4) active sessions = sessions with status "running" or "active", (5) next briefing = "Available" if briefing exists, else countdown or "No schedule" [owner:ui-engineer]

## UI Batch 2 -- Layout Restructure

- [ ] [3.1] [P-1] Update `apps/dashboard/app/page.tsx` -- remove the `grid grid-cols-1 lg:grid-cols-5 gap-6` two-column layout; remove the QuickActions component entirely (including the "Message Nova" link); restructure content flow to: PriorityBanner, StatStrip (full width), filter pills, activity feed (full width), obligation input, recent conversations (full width) [owner:ui-engineer]
- [ ] [3.2] [P-1] Reposition obligation quick-add input -- move the "Create Obligation" input from QuickActions to a slim full-width bar above the activity feed (below filter pills); render as a single `flex` row: input field (`flex-1`) + send button; keep existing POST logic, success/error feedback; style with `border border-ds-gray-400 rounded-lg px-3 py-1.5` to match density tokens [owner:ui-engineer]

## UI Batch 3 -- Activity Feed Enhancements

- [ ] [4.1] [P-1] Add `getEventSeverity` helper function in `apps/dashboard/app/page.tsx` -- pure function mapping `ActivityFeedEvent` to severity tier; uses the `severity` field from the API response (Req-4); returns `"error"` | `"warning"` | `"routine"`; this drives icon color, accent bar, and text contrast [owner:ui-engineer]
- [ ] [4.2] [P-1] Rework `ActivityFeedSection` rows in `apps/dashboard/app/page.tsx` -- change each row to a single horizontal line: `[timestamp w-12 text-label-13-mono] [icon 13px severity-colored] [summary text-sm truncate flex-1] [relative-time text-xs text-ds-gray-700 ml-auto]`; reduce row padding from `py-1.5` to `py-1`; apply severity styling: error rows get `bg-red-500/5` background tint + `border-l-2 border-red-500` left accent, warning rows get `bg-amber-500/5` + `border-l-2 border-amber-500`, routine rows get no accent and muted icon/text colors [owner:ui-engineer]
- [ ] [4.3] [P-2] Add expandable detail to feed rows -- wrap each row in a clickable container; on click, toggle an inline detail panel below the row; the detail panel shows: full event summary (unwrapped), event type badge, absolute timestamp, and a "View" link to the relevant page (`/obligations` for obligation events, `/messages` for message events, `/diary` for diary events, `/sessions` for session events); use controlled state (`expandedEventId`) not `<details>` element; only one row expanded at a time [owner:ui-engineer]
- [ ] [4.4] [P-1] Add category filter pills above the feed -- render a `flex gap-1.5` row of pill buttons: "All", "Messages", "Sessions", "Obligations", "System"; each pill shows count in parentheses; active pill: `bg-ds-gray-alpha-200 border-ds-gray-1000/40 text-ds-gray-1000`; inactive: `text-ds-gray-700 border-ds-gray-400 hover:text-ds-gray-1000`; pills use `text-label-12 px-2.5 py-1 rounded-full border`; filter stored in component state; "System" matches type "diary"; "All" shows unfiltered [owner:ui-engineer]

## UI Batch 4 -- Cleanup

- [ ] [5.1] [P-2] Remove dead code from `apps/dashboard/app/page.tsx` -- remove the `QuickActions` component definition, remove unused imports (`Send`, `Loader2`, `FileText`, `ArrowRight` if no longer needed), remove `briefingPreview` state and its fetch logic if no longer consumed outside the stat strip [owner:ui-engineer]
- [ ] [5.2] [P-3] Remove `apps/dashboard/components/ServerHealth.tsx` if no longer imported anywhere -- the stat strip replaces its dashboard usage; check for other imports first with grep [owner:ui-engineer]

## Verify

- [ ] [6.1] `cd apps/dashboard && pnpm typecheck` passes -- zero TypeScript errors [owner:ui-engineer]
- [ ] [6.2] `cd apps/dashboard && pnpm build` passes -- production build succeeds [owner:ui-engineer]
- [ ] [6.3] [user] Visual review: stat strip renders 5 stat cells in a horizontal row, wraps on mobile
- [ ] [6.4] [user] Visual review: activity feed is full-width with no right-column sidebar
- [ ] [6.5] [user] Visual review: error events have red accent, warning events have amber accent, routine events are muted
- [ ] [6.6] [user] Visual review: filter pills appear above feed, counts update, filtering works
- [ ] [6.7] [user] Visual review: clicking a feed row expands inline detail panel
- [ ] [6.8] [user] Visual review: quick-add obligation input is full-width above the feed
- [ ] [6.9] [user] Visual review: "Message Nova" link no longer appears anywhere on the dashboard
- [ ] [6.10] [user] Visual review: density matches or exceeds Diary page benchmark
