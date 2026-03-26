# Proposal: Polish Dashboard Home Page

## Change ID
`polish-dashboard`

## Summary

Add personalized greeting banner with time-of-day context, last-updated timestamps on auto-refresh, and improved stat card grouping on the Dashboard home page.

## Context
- Extends: `apps/dashboard/app/page.tsx` (dashboard home, 667 lines)
- Related: Dashboard auto-refresh logic, daemon WebSocket connection state

## Motivation

The dashboard is the primary entry point but feels impersonal and data-heavy. There is no greeting, no context for time-of-day, and 6 stat cards in one row create a wall of numbers. Auto-refresh has no visible indicator of data freshness. When the daemon is disconnected, stat cards show misleading "0" values instead of communicating the actual state.

1. **Personalization** --- a greeting with the user's name and time-of-day context makes the dashboard feel like a personal command center rather than a generic monitoring page.
2. **Data freshness** --- displaying "Updated 30s ago" next to auto-refresh gives confidence that numbers are current, reducing the urge to manually reload.
3. **Visual hierarchy** --- splitting 6 cards into two labeled groups (operational vs performance) reduces cognitive load and makes scanning faster.
4. **Honest offline state** --- dimming cards and showing "Offline" when disconnected prevents misinterpretation of stale or zero data.

## Requirements

### Req-1: Greeting Banner

Show "Good morning/afternoon/evening, Leo" with today's date and a one-line briefing summary fetched from `/api/briefing`. Replaces the plain "Dashboard" / "Nova activity overview" header.

- Morning: 05:00--11:59, Afternoon: 12:00--16:59, Evening: 17:00--04:59
- Briefing summary is fire-and-forget: show greeting immediately, append summary when it arrives
- If `/api/briefing` fails or is slow (>3s), show greeting without summary --- never block render

### Req-2: Last-Updated Timestamp

Display "Updated Xs ago" next to the auto-refresh toggle, updating in real-time via a 1s interval. Show exact ISO timestamp on hover (title attribute).

- Resets to "Updated just now" on each successful data fetch
- When auto-refresh is off, timestamp still ticks up from last fetch

### Req-3: Stat Card Grouping

Split the 6 stat cards into 2 visual rows with subtle group labels:

- **Operational** (top row): Obligations, Active Sessions, Health
- **Performance** (bottom row): Cold Starts, Five-Byte, Tokens

Each group has a muted label above it. Card order within groups matches the current left-to-right order.

### Req-4: Disconnected State Overlay

When the daemon WebSocket is disconnected:

- Dim all stat cards (opacity reduction or overlay)
- Show an "Offline" badge on each card instead of displaying potentially misleading "0" values
- Hide or grey out the auto-refresh toggle since refresh is meaningless while disconnected
- When connection restores, remove overlay and resume normal display

## Scope
- **IN**: Greeting banner, last-updated timestamp, stat card grouping, disconnected state overlay
- **OUT**: Dashboard layout redesign, widget system, drag-and-drop reordering, new stat cards, sidebar changes

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/page.tsx` | Modified: greeting banner replaces header, stat cards split into groups, offline overlay logic |
| `apps/dashboard/app/page.tsx` | Modified: last-updated timestamp next to auto-refresh toggle |

## Risks
| Risk | Mitigation |
|------|-----------|
| Greeting banner adds an API call on page load | Fire-and-forget fetch; greeting renders immediately without waiting for briefing summary; show greeting-only if API fails |
| Time-of-day greeting uses server time instead of client time | Compute greeting client-side using browser locale |
| 1s interval for "Updated X ago" causes unnecessary re-renders | Use a lightweight `useEffect` interval that only updates the timestamp string, not the entire component |
| Offline overlay hides data users might still want to see | Use opacity dimming (not removal) so values remain visible but clearly stale; tooltip explains "Last known values" |
