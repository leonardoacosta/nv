# Proposal: Redesign Dashboard Home

## Change ID
`redesign-dashboard-home`

## Summary

Replace the empty-state stat cards dashboard home with a dense command center layout: priority banner, unified activity feed, quick actions panel, and recent conversations.

## Context
- Extends: `apps/dashboard/app/page.tsx`, `apps/dashboard/components/layout/PageShell.tsx`, `apps/dashboard/types/api.ts`
- Related: `polish-dashboard` (added greeting banner + stat cards, now being replaced), `improve-obligation-ux` (obligation summary bar pattern reusable here)

## Motivation

The current dashboard home is dominated by 6 oversized stat cards that show zero values (Obligations 0, Active 0, Health "ok", Projects 0, CPU "--", Memory "--"), a CC Session card in "Stopped" state, an empty Active Sessions list, and an empty Obligations sidebar. The greeting banner ("Good evening, Leo") burns prime viewport space. For a personal assistant command center, the home page should surface actionable information immediately: what happened recently, what needs attention, and how to interact with Nova.

1. **Stat cards are noise** -- 6 large tiles displaying zeros or dashes provide no value and push real content below the fold
2. **No activity visibility** -- the activity feed only shows daemon WebSocket events (session loaded), not actual messages, obligation changes, or diary entries from the database
3. **No quick interaction** -- to message Nova or create an obligation, the user must navigate away from the home page
4. **Wasted vertical space** -- the greeting banner + stat cards + CC Session widget consume ~500px before any useful content appears

## Requirements

### Req-1: Remove Stat Cards and Greeting Banner

Remove the 6 stat cards (Obligations, Active, Health, Projects, CPU, Memory) and the "Good morning/evening, Leo" greeting banner with date subtitle. Replace the `PageShell` title with a minimal "Command Center" header that preserves the auto-refresh toggle and "Updated Xs ago" timestamp.

### Req-2: Priority Banner

Add a single-line alert banner at the top of the content area (below the page header). The banner appears when any pending obligations exist (status "open" or "in_progress") or when a briefing is available. Display format: amber background for pending obligations ("N obligations need attention"), blue background for briefing available ("Briefing available -- last generated HH:MM"). When both conditions are true, show obligations (higher priority). When neither condition is true, the banner is hidden (zero height, no reserved space).

### Req-3: Unified Activity Feed (Left 60%)

Replace the current WebSocket-only activity feed with a database-backed unified timeline. Create a new API route `GET /api/activity-feed` that queries the last 24 hours from three tables:

- `messages` table: each row becomes an event with type "message", showing direction (inbound/outbound), channel, sender, and a truncated content preview (first 80 chars)
- `obligations` table: each row created or updated in the last 24h becomes an event with type "obligation", showing the detected action and current status
- `diary` table: each row becomes an event with type "diary", showing the slug and channel

The endpoint merges all three sources, sorts by timestamp descending, and returns the most recent 50 events. Response shape: `{ events: ActivityFeedEvent[] }` where each event has `id`, `type`, `timestamp`, `icon_hint` (for the client to pick a Lucide icon), and `summary` (one-line text).

The feed renders as dense table rows (not cards): monospace timestamp on the left, event type icon, one-line summary. Rows are compact (py-1.5, text-sm). Use subtle bottom borders between rows, not card surfaces. Preserve the WebSocket subscription to prepend real-time events to the top of the feed.

### Req-4: Quick Actions Panel (Right 40%)

Replace the Obligations sidebar and Session Breakdown sidebar with a Quick Actions panel containing:

1. **Message Nova** -- a link/button that navigates to `/chat`. Styled as a prominent action row.
2. **Create Obligation** -- an inline single-line text input with a submit button. On submit, POST to `/api/obligations` with `{ detected_action: <input value>, owner: "nova", status: "open", priority: 2, source_channel: "dashboard" }`. On success, clear the input and show a brief "Created" confirmation. On error, show the error inline.
3. **View Briefing** -- a link to `/briefing`. If a briefing exists (from the same fetch used by the priority banner), show a one-line preview of the briefing content (first 100 chars). Otherwise show "No briefing available" in muted text.
4. **Fleet Health** -- a single-line status from `/api/server-health` showing "DB: healthy" (or the actual status). No chart, no CPU/memory breakdown -- just the status word with a green/amber/red dot.

Each action row uses a consistent layout: icon on the left, label + description, and optional trailing indicator. Separated by subtle borders, not wrapped in cards.

### Req-5: Recent Conversations

Below the activity feed (spanning full width), show the last 5 messages grouped by conversation. Query `GET /api/messages?limit=10` and group consecutive messages by the same sender. Each group shows: sender name, channel badge, timestamp of first message, and content preview of the first message (truncated to 120 chars). Each group links to `/messages`. Render as a compact list with border separators.

### Req-6: Layout and Styling

- Remove all gutters -- content fills the main content area edge to edge (remove `max-w-6xl mx-auto` constraint from PageShell usage or override it)
- The two-column layout (60/40) uses `grid grid-cols-1 lg:grid-cols-5` with feed spanning `lg:col-span-3` and quick actions spanning `lg:col-span-2`
- Recent conversations section sits below both columns at full width
- Geist design language: monospace for timestamps and numeric data (`font-mono`), subtle borders (`border-ds-gray-400`) instead of card surfaces, tight spacing (`space-y-1` between rows, `py-1.5` row padding)
- Keep the auto-refresh toggle and "Updated Xs ago" from the existing implementation
- Keep the daemon disconnected overlay behavior (opacity reduction when WebSocket drops)

## Scope
- **IN**: Dashboard home page complete rewrite, new `GET /api/activity-feed` route, new `ActivityFeedEvent` type, obligation creation POST from dashboard, quick actions panel
- **OUT**: Other dashboard pages (obligations, messages, briefing, etc.), briefing cron, contact graph, daemon WebSocket protocol changes, PageShell component modifications (override layout in page only)

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/page.tsx` | Rewrite: remove stat cards, greeting, session widgets; add priority banner, activity feed, quick actions, recent conversations |
| `apps/dashboard/app/api/activity-feed/route.ts` | New: unified activity feed endpoint querying messages + obligations + diary |
| `apps/dashboard/types/api.ts` | Add: `ActivityFeedEvent`, `ActivityFeedGetResponse` types |
| `apps/dashboard/app/api/obligations/route.ts` | Add: POST handler for creating obligations from dashboard |

## Risks
| Risk | Mitigation |
|------|-----------|
| Activity feed query across 3 tables may be slow | Each table query uses indexed `created_at` with 24h filter and individual limits; merge in application code, not a SQL union |
| Obligation POST without full validation | Reuse the same validation the daemon uses; minimal required fields only (detected_action, owner, status, priority, source_channel) |
| Removing stat cards loses server health visibility | Fleet health one-liner in quick actions panel preserves the essential signal; detailed health remains on dedicated pages |
| Edge-to-edge layout may conflict with PageShell max-width | Override the constraint at the page level by wrapping content outside the default container, or pass a `fullWidth` prop |
