# Proposal: Obligations Kanban Board

## Change ID
`obligations-kanban-board`

## Summary

Replace the flat-list Active tab on the Obligations page with a two-column Kanban board (Nova | Leo), status swim lanes within each column, draggable cards, inline create, and card-level activity/detail -- eliminating the dead-page empty state and the wasted activity sidebar.

## Context
- Extends: `apps/dashboard/app/obligations/page.tsx`, `apps/dashboard/components/ObligationItem.tsx`, `apps/dashboard/components/ObligationSummaryBar.tsx`, `apps/dashboard/components/ActivityFeed.tsx`
- Related: `improve-obligation-ux` (contextual actions, deadline indicators, expand/collapse -- all preserved), `redesign-dashboard-home` (activity feed pattern reusable)

## Motivation

The current Active tab renders two vertical sections (Nova, Leo) as flat card lists. When empty, the page shows "No obligations assigned to Nova" and "No obligations assigned to Leo" -- two dead-text blocks with no affordance to create work. The Activity Feed sidebar on the right shows "POLLING" with "No activity yet" until something happens, wasting 33% of the viewport. Approvals and History tabs are unaffected.

1. **Dead empty state** -- two text-only sections with zero visual structure or creation affordance; users must navigate elsewhere to create obligations
2. **No status grouping** -- all active statuses (open, in_progress, proposed_done) are mixed in a single flat list per owner, requiring mental scanning to find what needs attention
3. **No drag-based workflow** -- reassigning an obligation between Nova and Leo or changing status requires opening the card and clicking action buttons; there is no spatial metaphor for workflow progression
4. **Wasted sidebar** -- the ActivityFeed component occupies lg:w-1/3 of the layout but only shows obligation-level WebSocket events, which are sparse; the space would be better used for card detail when a card is selected
5. **No inline creation** -- creating an obligation requires navigating to a different flow; a "+" button at the top of each column (pre-assigning the owner) removes friction

## Requirements

### Req-1: Two-Column Kanban Layout

Replace the current Nova/Leo vertical sections with a two-column board layout. Each column is headed by the owner identity (Nova with `N` badge, Leo with `L` badge) and a count of active obligations in that column. Columns use `grid grid-cols-1 lg:grid-cols-2 gap-4` for the board area. On mobile (< lg), columns stack vertically. The Approvals and History tabs remain unchanged.

### Req-2: Status Swim Lanes

Within each column, group obligations into three collapsible swim lanes ordered top-to-bottom: **In Progress** (status `in_progress`), **Pending** (status `open`), **Proposed Done** (status `proposed_done`). Each lane has a subtle header with status label, count badge, and collapse toggle. Empty lanes render a single-line placeholder ("No items") at reduced opacity but remain visible so drop targets are always available. Obligations within each lane are sorted by priority (P0 first).

### Req-3: Draggable Cards

Obligations can be dragged between swim lanes within a column (status change) and between columns (owner reassignment). Use the HTML5 Drag and Drop API (no external library) with `draggable`, `onDragStart`, `onDragOver`, `onDrop` handlers. On drop:
- **Cross-lane (same column)**: PATCH `/api/obligations/{id}` with the new `status` matching the target lane
- **Cross-column**: PATCH `/api/obligations/{id}` with the new `owner` (and optionally new `status` if dropped into a different lane)

During drag, the target lane shows a dashed border highlight (`border-dashed border-ds-gray-700`). The dragged card gets `opacity-50`. On mobile, drag is disabled -- users use the existing action buttons instead.

### Req-4: Compact Card Design

Redesign the ObligationCard for Kanban density. The card shows:
- Priority bar (left edge, existing `PRIORITY_BAR` colors)
- Title (`detected_action`, single line, truncated)
- Due date (if present, with deadline proximity coloring from existing `DEADLINE_RING` logic)
- Status badge (existing `STATUS_BADGE` styling)
- Owner badge (only shown on the "Other" overflow, since column already implies owner)

Click on a card expands it inline (within the lane) to show the full detail view: execution history, source context, notes, and the contextual action buttons. Only one card can be expanded at a time across the entire board. Expanding a card collapses any previously expanded card.

### Req-5: Inline Create

Each column header includes a "+" icon button. Clicking it reveals an inline text input at the top of the Pending lane with the owner pre-assigned to the column's owner. On submit (Enter or blur with content), POST to `/api/obligations` with `{ detected_action: <input>, owner: <column_owner>, status: "open", priority: 2, source_channel: "dashboard" }`. On success, the new card appears in the Pending lane and the input clears. On error, show a brief inline error message. Pressing Escape cancels and hides the input.

### Req-6: Activity Feed Collapses into Card Detail

Remove the standalone ActivityFeed sidebar from the Active tab layout. When a card is expanded (Req-4), the detail view includes an "Activity" section that fetches obligation-specific events from `GET /api/obligations/{id}/activity` (existing endpoint) and renders them inline below the execution history. This eliminates the always-visible sidebar that shows "No activity yet" in the common case. The ActivityFeed component is retained for potential use elsewhere but no longer rendered on this page.

### Req-7: Quick Actions on Hover

When hovering over a non-expanded card, show a floating action bar on the right edge of the card with icon buttons: Done (Check), Dismiss (X), Reassign (ArrowLeftRight). These reuse the existing `getActionsForStatus` helper and `patchStatus`/`handleStart` logic. The Reassign action toggles the owner between "nova" and "leo" via PATCH. On touch devices, the actions appear on tap-and-hold or are accessed via the expanded card view.

### Req-8: Keyboard Shortcuts and Real-Time Updates

Preserve the existing keyboard navigation. Add arrow-key navigation across the board: Up/Down moves between cards within a lane, Left/Right moves focus between columns. `d` marks focused card done, `x` dismisses, `r` reassigns, `Enter` expands/collapses. Preserve the `useDaemonEvents` WebSocket subscription so cards update in real time when the daemon changes obligation status. New/updated obligations animate into their correct lane position.

## Scope
- **IN**: Kanban board layout, swim lanes, drag-and-drop, compact cards, inline create, activity-in-card, hover actions, keyboard navigation for Active tab only
- **OUT**: Approvals tab, History tab, obligation data model changes, new API endpoints (reuses existing PATCH and POST), daemon WebSocket protocol changes, mobile drag (disabled, action buttons remain)

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/obligations/page.tsx` | Rewrite Active tab: replace flat Nova/Leo sections + ActivityFeed sidebar with two-column Kanban board; extract board into `KanbanBoard` component |
| `apps/dashboard/components/obligations/KanbanBoard.tsx` | New: board container with two columns, swim lanes, drag-and-drop state management |
| `apps/dashboard/components/obligations/KanbanColumn.tsx` | New: single column (owner) with swim lanes, inline create input, column header |
| `apps/dashboard/components/obligations/KanbanLane.tsx` | New: status-grouped lane with collapse toggle, drop target styling, card list |
| `apps/dashboard/components/obligations/KanbanCard.tsx` | New: compact card with hover actions, inline expand, drag handle; reuses priority/status/deadline logic from existing ObligationCard |
| `apps/dashboard/components/obligations/InlineCreate.tsx` | New: inline text input for quick obligation creation |
| `apps/dashboard/components/ObligationItem.tsx` | Unchanged (used by History tab) |
| `apps/dashboard/components/ActivityFeed.tsx` | Unchanged (removed from Active tab layout, retained in codebase) |

## Risks
| Risk | Mitigation |
|------|-----------|
| HTML5 Drag and Drop has inconsistent touch support | Disable drag on mobile (< lg breakpoint); touch users use action buttons and expanded card view instead |
| Drag-and-drop status changes could conflict with real-time WebSocket updates | Optimistic UI update on drop with rollback on PATCH failure; WebSocket events reconcile authoritative state |
| Single-card expansion may feel limiting with many obligations | Keyboard shortcut `Enter` makes expand/collapse fast; consider multi-expand as a follow-up if usage shows demand |
| Inline create at top of Pending lane may push existing cards down unexpectedly | Animate the input reveal with height transition; auto-scroll lane to keep input visible |
| Removing activity sidebar loses global obligation event visibility | Card-level activity provides more targeted context; global activity feed remains on the dashboard home page |
