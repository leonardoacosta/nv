# Proposal: Dashboard Command Center Density Pass

## Change ID
`dashboard-density-pass`

## Summary

Strip the dashboard home page down to its load-bearing elements: remove the duplicate "Message Nova" input, collapse the right-column filler cards into a compact stat strip, let the activity feed take full width, and give feed entries visual weight differentiation with category filtering. Every pixel earns its space -- Vercel dashboard density, Diary page as the internal benchmark.

## Context
- Extends: `apps/dashboard/app/page.tsx` (current two-column command center layout)
- Related: `redesign-dashboard-home` (built the current layout), `global-density-pass` (established density tokens and conventions), `improve-messages-ux`
- Components: `apps/dashboard/components/layout/StatCard.tsx` (has `inline` variant), `apps/dashboard/components/ActivityFeed.tsx`, `apps/dashboard/components/ServerHealth.tsx`
- APIs: `/api/activity-feed`, `/api/obligations`, `/api/messages`, `/api/server-health`, `/api/fleet-status`, `/api/briefing`, `/api/sessions`

## Motivation

The current dashboard has the right bones from `redesign-dashboard-home` but wastes space in three ways:

1. **Duplicate entry point** -- "Message Nova" in the right column quick actions is a dressed-up link to `/chat`. Users already know where chat is (sidebar nav). The input field competes with the Chat page without adding value since it does not inline-send -- it just navigates.
2. **Right column is filler** -- "View Briefing" and "Fleet Health" are link-cards with minimal data. They consume 40% of the viewport width to display information that fits in a single stat chip. The two-column split (60/40) starves the activity feed of horizontal space.
3. **Flat activity feed** -- Every event renders at the same visual weight: same height, same text color, same icon style. An error alert looks identical to a routine diary entry. There is no way to filter the feed to a specific category. The feed is useful but does not scale -- at 50 events, scanning becomes tedious without differentiation.

The Diary page is denser: inline stat strip, full-width content, compact rows with visual hierarchy. The dashboard home should match.

## Requirements

### Req-1: Remove "Message Nova" Quick Action

Remove the "Message Nova" link row from the QuickActions component. The Chat page is the canonical entry point for messaging Nova. Sidebar navigation already surfaces it. Removing this eliminates a duplicate interaction path and frees the space for the stat strip.

### Req-2: Compact Stat Strip

Replace the right-column QuickActions + Fleet Health cards with a horizontal stat strip spanning full width, rendered between the PriorityBanner and the activity feed. The strip contains 5 stat cells in a single row, separated by subtle borders:

1. **Unread Messages** -- total unread count as the primary number. Below or beside it, a compact channel breakdown (e.g. "TG: 3 / DC: 1 / TM: 2"). Source: `/api/messages` response, counting messages where `sender !== "nova"` and filtering to recent unread. If the API does not expose a read/unread flag, use messages from the last 4 hours as a proxy.
2. **Pending Obligations** -- count of open/in-progress obligations. Split by owner: "Nova: N / Leo: N". Source: existing obligations fetch already in the page.
3. **Fleet Health** -- "N/M up" where N = healthy services and M = total services. A green/amber/red dot indicates aggregate status. Source: `/api/fleet-status` (already returns `healthy_count` and `total_count`).
4. **Active Sessions** -- count of currently running sessions (status "running" or "active"). Source: `/api/sessions`.
5. **Next Briefing** -- countdown to next scheduled briefing (e.g. "in 3h 12m") or "Available" if one is ready and unread, or "No schedule" if none configured. Source: `/api/briefing` response (use `generated_at` to derive timing or add a `next_at` field if available).

Each stat cell uses the existing `StatCard` inline variant or a new lighter-weight `StatChip` component: icon + label + value in a compact horizontal layout. The strip uses `flex` with `border-r border-ds-gray-400` separators and equal spacing. On mobile, the strip wraps to 2-3 rows using `flex-wrap`.

### Req-3: Full-Width Activity Feed

Remove the two-column `grid grid-cols-1 lg:grid-cols-5` layout. After the stat strip, the activity feed takes the full content width. The RecentConversations section below remains full-width. This reclaims the 40% that the right column occupied.

### Req-4: Activity Feed Visual Weight

Differentiate feed events by severity/importance:

- **Error/alert events** (obligation failures, health degradation, execution errors): left accent bar or background tint using `bg-red-500/5` or similar, icon in error color (`text-red-500`), summary text in `text-ds-gray-1000` (full contrast).
- **Action-required events** (new obligations detected, pending approvals): amber accent, icon in `text-amber-500`.
- **Routine events** (diary entries, outbound messages, completed obligations): no accent, icon and summary in muted tones (`text-ds-gray-700` / `text-ds-gray-900`).

The classification logic maps `event.type` + keywords in `event.summary` (e.g. "failed", "error", "critical") to a severity tier. This should be a pure function `getEventSeverity(event: ActivityFeedEvent): "error" | "warning" | "routine"` for testability.

### Req-5: Compact Feed Rows

Redesign each feed row to be a single horizontal line:

```
[timestamp] [icon] [actor] [action verb] [target] [relative time]
```

- Timestamp: monospace, fixed width (`w-12`), `text-label-13-mono`
- Icon: 13px, color based on severity (Req-4)
- Actor + action + target: single `text-sm` span, truncated with `truncate` class
- Relative time: right-aligned, muted, `text-xs text-ds-gray-700`

Row height target: `py-1` (tighter than current `py-1.5`). Each row is clickable/expandable -- on click, an inline detail panel slides open below the row showing full event text, metadata, and a link to the source page (e.g. link to `/obligations` for obligation events). Use `details/summary` or controlled state, not a modal.

### Req-6: Category Filter Pills

Above the activity feed (below the stat strip), render a row of filter pill tabs:

- **All** (default, shows everything)
- **Messages** (type === "message")
- **Sessions** (type === "session" -- requires adding session events to the feed)
- **Obligations** (type === "obligation")
- **System** (type === "diary" or type === "system")

Pills use the existing design language: `text-label-12`, `px-2.5 py-1`, `rounded-full`, `border border-ds-gray-400`. Active pill gets `bg-ds-gray-alpha-200 border-ds-gray-1000/40 text-ds-gray-1000`. Inactive pills are muted. Each pill shows its event count in parentheses: "Messages (12)".

Filtering is client-side on the already-fetched events array. The active filter is stored in component state (not URL params -- this is a transient UI filter).

### Req-7: Quick-Add Obligation Repositioned

Move the "Create Obligation" inline input from the now-removed QuickActions panel to a prominent position: either (a) directly above the activity feed as a slim input bar spanning full width, or (b) as a persistent footer bar at the bottom of the feed section. The input retains its current behavior (POST to `/api/obligations`, clear on success, inline error display). It should be visually distinct but not dominate -- a single-line input with a send button, no card wrapper.

### Req-8: Activity Feed API Enhancement

Extend `GET /api/activity-feed` to include session events alongside messages, obligations, and diary entries. Query the `sessions` table for sessions started or ended in the last 24 hours, mapping each to an `ActivityFeedEvent` with type `"session"` and a summary like "Session started: project-name (agent)" or "Session completed: project-name (2h 15m)". This ensures the "Sessions" filter pill (Req-6) has data.

Also add a `severity` field to `ActivityFeedEvent`: `"error" | "warning" | "info"`. The API assigns severity based on event content (failed obligations = error, new obligations = warning, everything else = info). This moves classification to the server where it has full context, rather than client-side keyword matching.

## Scope
- **IN**: Dashboard home page layout restructure, stat strip component, activity feed visual weight + compact rows + expandable detail + filter pills, obligation input repositioning, activity feed API enhancement (session events + severity field)
- **OUT**: Other dashboard pages, new API endpoints beyond the activity-feed enhancement, sidebar changes, WebSocket protocol changes, Chat page changes

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/page.tsx` | Major restructure: remove two-column grid, remove QuickActions component, add stat strip, add filter pills, rework ActivityFeedSection for visual weight + compact rows + expandable detail, reposition obligation input |
| `apps/dashboard/app/api/activity-feed/route.ts` | Add session event source, add severity field to response |
| `apps/dashboard/types/api.ts` | Update `ActivityFeedEvent` to include `severity` field, add `"session"` to type union |
| `apps/dashboard/components/StatStrip.tsx` | New component: horizontal stat strip with 5 stat cells |

## Risks
| Risk | Mitigation |
|------|-----------|
| Adding sessions to activity feed increases query load | Sessions table is small (dozens of rows per day); same 24h window + limit pattern as existing sources |
| `severity` field adds API opinion about event importance | Severity is advisory -- client can override or ignore; default "info" is safe |
| Expandable feed rows increase DOM complexity | Use lazy rendering -- detail panel only mounts on click, not for all 50 rows |
| Stat strip requires 2 additional API calls (fleet-status, sessions) | Both are lightweight; fleet-status is static registry, sessions is a simple count query. Fetch in the existing `Promise.allSettled` batch |
| Channel breakdown in unread messages stat depends on data the messages API may not expose cleanly | Fall back to total count only if channel grouping is not practical; the stat is useful even without breakdown |
| Five stat cells may overflow on narrow viewports | `flex-wrap` with `gap-2` ensures graceful wrapping; min-width per cell prevents text truncation |
