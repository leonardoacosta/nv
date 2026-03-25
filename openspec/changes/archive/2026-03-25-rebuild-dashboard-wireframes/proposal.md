# Proposal: Rebuild Dashboard Wireframes

## Change ID
`rebuild-dashboard-wireframes`

## Summary

Rebuild all five primary dashboard pages in the extracted Next.js app to match the approved
wireframes. The current Vite/React SPA has drifted from the wireframe designs, mixes concerns
across pages, and lacks the new pages added to the roadmap (message history, approval queue).
After `extract-nextjs-dashboard` lands, this spec rebuilds every page from scratch using the
correct layout primitives, shadcn/ui components, Tailwind tokens, WebSocket feed, and
mobile-responsive grid.

## Context
- Depends on: `extract-nextjs-dashboard` (Next.js app must exist before this runs)
- Extends: `apps/dashboard/` (Next.js app created by extract-nextjs-dashboard)
- Replaces: `dashboard/src/pages/DashboardPage.tsx`, `NexusPage.tsx`, `SettingsPage.tsx`,
  `ObligationsPage.tsx`, `MemoryPage.tsx`, `UsagePage.tsx`, `IntegrationsPage.tsx`, `ProjectsPage.tsx`
- Related beads: nv-4zs (dashboard-wireframe-drift), nv-tft (notifications), nv-0n8 (mobile-responsive),
  nv-967 (charts-trends), nv-e34 (activity-feed), nv-8y9 (approval-queue), nv-9p9 (conversation-threads),
  nv-jea (message-history), nv-x3m (authentication), nv-42e (websocket-feed)

## Motivation

The current embedded SPA was built iteratively and has accumulated drift:

1. **Wireframe misalignment** — page layouts, grid structure, and component hierarchy diverge from
   the approved wireframes. The main dashboard mixes health metrics into the summary grid instead of
   separating them into a dedicated health panel. The sessions view (NexusPage) is correct in spirit
   but uses inconsistent card sizes.
2. **Missing pages** — the wireframes specify a Message History page (searchable log), Approval Queue
   page (pending actions awaiting Leo's approval), and a dedicated Activity Feed. None exist.
3. **No WebSocket feed** — all pages poll on a fixed interval or require manual refresh. The daemon
   exposes a WebSocket feed; the dashboard should consume it for real-time updates without polling.
4. **Mobile layout gaps** — the sidebar collapses to an icon rail but the content grid does not
   reflow correctly on narrow viewports. Leo checks the dashboard from his phone.
5. **No shared layout primitives** — each page re-implements its own header, loading skeleton,
   and error banner instead of using shared layout components.

## Requirements

### Req-1: Shared Layout System

Create a set of layout primitives used by all pages:

- `PageShell` — page header (title, subtitle, action slot), max-width container, spacing
- `PageSkeleton` — standardized loading state (animated pulse, matches loaded content shape)
- `ErrorBanner` — inline error with retry button, cosmic-rose styling
- `EmptyState` — centered icon + message + optional CTA, consistent across all empty pages
- `StatCard` — metric tile (icon, label, value, optional trend indicator)
- `SectionHeader` — uppercase label + count badge used before grouped lists

These replace the duplicated inline implementations across DashboardPage, NexusPage, ObligationsPage, etc.

### Req-2: Main Dashboard Page (`/`)

Layout: two-column on desktop (content left 2/3, sidebar right 1/3), single column on mobile.

Left column:
- System health overview: status badge (healthy/degraded/critical), CPU %, memory %, uptime
- Active sessions: top 5 by status (active first), each with slug, project, progress bar, duration
- Recent activity feed: last 10 daemon events (message received, tool executed, session started/ended)

Right column (sidebar panel):
- Obligations summary: counts by owner (Nova / Leo) and status (open / in_progress)
- Quick stats: messages today, tools today, cost today

Header action: auto-refresh toggle (on by default, 10s interval via WebSocket or polling fallback).

### Req-3: Sessions Page (`/sessions`)

Replaces the current NexusPage. Shows the full worker session list.

- Three collapsible sections: Active, Idle, Completed (last 20)
- Each session card: slug/ID, project code, agent name, status indicator, progress bar (pct),
  phase label, started_at (relative time), duration display, branch if present
- Filter bar: by project (dropdown), by status (tabs), text search on slug/agent
- Detail drawer (slide-in): full session metadata, spec name, log tail if available
- Real-time: WebSocket updates promote idle → active, update progress bars

### Req-4: Message History Page (`/messages`)

New page. Searchable log of all messages Nova has processed.

- Chronological list, newest first, paginated (50 per page)
- Each row: channel icon (Telegram/Discord/Teams/iMessage), sender display name, message preview
  (truncated to 120 chars), timestamp (relative + absolute on hover), response latency badge
- Search bar: full-text search on message content and sender, debounced 300ms
- Filter chips: by channel, by date range (today / last 7d / all)
- Clicking a row expands inline to show full message, Nova's response, and tool calls used
- Backed by `GET /api/messages` — new API endpoint (see Impact)

### Req-5: Approval Queue Page (`/approvals`)

New page. Shows pending `PendingAction` items that need Leo's approval before Nova executes them.

- Split view: queue (left) and detail panel (right), collapses to single column on mobile
- Queue list: each item shows action type icon, title, project, created_at, urgency indicator
- Detail panel: full action description, proposed changes, context that triggered it, approve / dismiss
  buttons with confirmation (approve sends `POST /api/approvals/:id/approve`, dismiss sends
  `PATCH /api/obligations/:id` status=dismissed)
- Empty state: "No pending actions" with a note that Nova will ask for approval when needed
- Badges in sidebar nav: show count of pending approvals (pulled from `GET /api/obligations?owner=leo&status=open`)

### Req-6: Settings Page (`/settings`)

Rebuild the existing settings page with improved layout:

- Replace the auto-inferred field list with explicitly grouped sections matching config schema:
  - Daemon (log level, max workers, tool timeouts)
  - Channels (enabled flags per channel, rate limits)
  - Integrations (API endpoint URLs only — no secrets in the form)
  - Memory (retention days, max topics)
- Unsaved changes sticky footer (already exists — keep)
- Read-only display for secret fields (show masked value, no edit input)
- Restart notice banner when fields that require daemon restart are changed

### Req-7: Mobile-Responsive Layout

All pages must work at 375px (iPhone SE) and 768px (iPad portrait):

- Sidebar: icon-only rail at ≤768px, hidden at ≤640px with hamburger menu toggle
- Main dashboard: stack columns vertically at ≤768px
- Sessions page: hide detail drawer at ≤768px; tap session card to navigate to dedicated session
  detail route instead
- Approval queue: stack queue and detail panel vertically at ≤768px
- All touch targets minimum 44×44px (Tailwind `min-h-11 min-w-11`)

### Req-8: WebSocket Feed

Connect to `ws://<host>/ws/events` (endpoint provided by `extract-nextjs-dashboard` spec):

- Single shared WebSocket connection managed in a React context (`DaemonEventContext`)
- Reconnect with exponential backoff (1s, 2s, 4s, max 30s) on close/error
- Event types consumed: `session.updated`, `session.started`, `session.ended`, `health.snapshot`,
  `message.received`, `approval.created`
- Each event type dispatches to the relevant page's local state via context subscription
- Connection status indicator in sidebar footer (green dot = connected, amber = reconnecting, red = disconnected)
- Fallback: if WebSocket unavailable, pages fall back to 10s polling (existing behavior)

## Scope
- **IN**: All five rebuilt pages, shared layout primitives, WebSocket context, new `/messages` and
  `/approvals` API endpoints, mobile layout, sidebar nav badge for approvals
- **OUT**: Authentication/login page (nv-x3m, separate spec), charts/trends deep analytics (nv-967,
  separate spec), conversation threading UI (nv-9p9, separate spec), notifications panel (nv-tft,
  separate spec)

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/src/components/layout/` | New: PageShell, PageSkeleton, ErrorBanner, EmptyState, StatCard, SectionHeader |
| `apps/dashboard/src/components/providers/DaemonEventContext.tsx` | New: WebSocket context + reconnect logic |
| `apps/dashboard/src/app/(dashboard)/page.tsx` | Rebuilt: main dashboard with two-column layout |
| `apps/dashboard/src/app/(dashboard)/sessions/page.tsx` | New file: sessions page (replaces NexusPage) |
| `apps/dashboard/src/app/(dashboard)/messages/page.tsx` | New: message history page |
| `apps/dashboard/src/app/(dashboard)/approvals/page.tsx` | New: approval queue page |
| `apps/dashboard/src/app/(dashboard)/settings/page.tsx` | Rebuilt: settings with explicit schema grouping |
| `apps/dashboard/src/app/layout.tsx` | Wrap root with DaemonEventContext provider |
| `crates/nv-daemon/src/dashboard.rs` | Add `GET /api/messages` (paginated), `POST /api/approvals/:id/approve`, WebSocket `/ws/events` endpoint |
| `apps/dashboard/src/components/sidebar/` | Rebuild: mobile-responsive nav, approvals badge, WS status dot |

## API Additions

### GET /api/messages
```
Query: ?limit=50&offset=0&channel=telegram&search=<text>
Response: { messages: [{ id, channel, sender, content, response, tools_used, latency_ms, created_at }], total }
```
Backed by SQLite `messages` table already populated by the message store.

### POST /api/approvals/:id/approve
```
Body: {}
Response: { id, status: "approved" }
```
Triggers the pending action execution path; same approval flow currently done via Telegram.

### WebSocket /ws/events
```
Server-sent JSON frames: { type: "session.updated" | "session.started" | ... , payload: {...} }
Client-to-server: { type: "ping" }
Server-to-client: { type: "pong" }
```
Daemon broadcasts daemon events on this socket; dashboard subscribes.

## Risks
| Risk | Mitigation |
|------|-----------|
| extract-nextjs-dashboard not landed when this runs | Hard dependency — spec blocked until predecessor merges |
| WebSocket endpoint not available in extract spec | Fallback polling ensures pages work; WS is progressive enhancement |
| `/api/messages` query performance on large SQLite stores | Add index on `messages.created_at` in migration; paginate with LIMIT/OFFSET |
| Approval queue triggering unintended actions | Approval endpoint requires explicit confirmation body; no accidental approve on page load |
| Mobile layout regressions on existing pages | Add viewport smoke tests in Verify batch |
