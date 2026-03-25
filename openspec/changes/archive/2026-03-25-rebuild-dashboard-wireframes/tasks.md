# Implementation Tasks

<!-- beads:epic:nv-f8c -->

## DB Batch

- [x] [1.1] [P-1] Add `GET /api/messages` handler in `crates/nv-daemon/src/http.rs` — paginated query against SQLite `messages` table, params: limit, offset, channel, search [owner:api-engineer]
- [x] [1.2] [P-1] Add `POST /api/approvals/:id/approve` handler — mark obligation approved, trigger pending action execution path [owner:api-engineer]
- [x] [1.3] [P-2] Add WebSocket `/ws/events` endpoint to dashboard router — upgrade handler, broadcast session/health/message/approval events [owner:api-engineer]
- [x] [1.4] [P-2] Add SQLite index on `messages.created_at` (migration) so paginated message queries remain fast on large stores [owner:api-engineer]

## UI Batch — Shared Primitives

- [x] [2.1] [P-1] Create `apps/dashboard/src/components/layout/PageShell.tsx` — page header with title, subtitle, and action slot; max-width container [owner:ui-engineer]
- [x] [2.2] [P-1] Create `apps/dashboard/src/components/layout/PageSkeleton.tsx` — animated pulse skeleton that matches the loaded content shape for each page variant [owner:ui-engineer]
- [x] [2.3] [P-1] Create `apps/dashboard/src/components/layout/ErrorBanner.tsx` — inline error display with retry button, cosmic-rose styling [owner:ui-engineer]
- [x] [2.4] [P-1] Create `apps/dashboard/src/components/layout/EmptyState.tsx` — centered icon + message + optional CTA, consistent across all empty pages [owner:ui-engineer]
- [x] [2.5] [P-1] Create `apps/dashboard/src/components/layout/StatCard.tsx` — metric tile: icon, label, value, optional accent color and trend indicator [owner:ui-engineer]
- [x] [2.6] [P-2] Create `apps/dashboard/src/components/layout/SectionHeader.tsx` — uppercase section label with count badge and optional status dot [owner:ui-engineer]

## UI Batch — WebSocket Context

- [x] [3.1] [P-1] Create `apps/dashboard/src/components/providers/DaemonEventContext.tsx` — WebSocket connection manager: connect to `/ws/events`, reconnect with exponential backoff (1s/2s/4s, max 30s), expose event subscription hook [owner:ui-engineer]
- [x] [3.2] [P-1] Wrap `apps/dashboard/src/app/layout.tsx` root with `DaemonEventContext` provider [owner:ui-engineer]
- [x] [3.3] [P-2] Add WebSocket status indicator to sidebar footer — green/amber/red dot reflecting connected/reconnecting/disconnected state [owner:ui-engineer]

## UI Batch — Main Dashboard Page

- [x] [4.1] [P-1] Rebuild `apps/dashboard/src/app/(dashboard)/page.tsx` — two-column desktop layout (2/3 content, 1/3 sidebar panel), single column on mobile, using PageShell [owner:ui-engineer]
- [x] [4.2] [P-1] Implement system health overview panel — status badge (healthy/degraded/critical), CPU %, memory %, uptime display, backed by `GET /api/server-health` [owner:ui-engineer]
- [x] [4.3] [P-1] Implement active sessions summary — top 5 sessions by status (active first), SessionCard with slug, project, progress bar, duration [owner:ui-engineer]
- [x] [4.4] [P-2] Implement recent activity feed — last 10 daemon events from WebSocket stream with fallback to `GET /api/sessions` polling; each row shows event type, target, timestamp [owner:ui-engineer]
- [x] [4.5] [P-2] Implement obligations summary sidebar panel — counts by owner (Nova/Leo) and status, linked to `/approvals` for Leo's open items [owner:ui-engineer]
- [x] [4.6] [P-2] Add auto-refresh toggle to dashboard header — on by default, 10s interval, subscribes to WebSocket events when available [owner:ui-engineer]

## UI Batch — Sessions Page

- [x] [5.1] [P-1] Create `apps/dashboard/src/app/(dashboard)/sessions/page.tsx` — three-section layout: Active, Idle, Completed (last 20), using PageShell [owner:ui-engineer]
- [x] [5.2] [P-1] Implement session card component — slug/ID, project code, agent name, status dot, progress bar (pct), phase label, started_at relative time, duration, branch badge [owner:ui-engineer]
- [x] [5.3] [P-2] Add filter bar — project dropdown, status tabs (all/active/idle/completed), text search on slug/agent name, debounced 300ms [owner:ui-engineer]
- [x] [5.4] [P-2] Implement session detail drawer — slide-in panel with full session metadata, spec name; on mobile collapses to separate route `/sessions/:id` [owner:ui-engineer]
- [x] [5.5] [P-2] Subscribe to WebSocket `session.updated`, `session.started`, `session.ended` events to update session list in real-time without polling [owner:ui-engineer]

## UI Batch — Message History Page

- [x] [6.1] [P-1] Create `apps/dashboard/src/app/(dashboard)/messages/page.tsx` — chronological list newest-first, paginated 50/page, using PageShell [owner:ui-engineer]
- [x] [6.2] [P-1] Implement message row component — channel icon (Telegram/Discord/Teams/iMessage), sender name, preview (120 char truncated), timestamp (relative + absolute on hover), response latency badge [owner:ui-engineer]
- [x] [6.3] [P-1] Implement expand-inline on row click — shows full message content, Nova's response, and tool calls used in a collapsible section [owner:ui-engineer]
- [x] [6.4] [P-2] Add search bar — full-text search on content and sender via `?search=` param, debounced 300ms, updates URL query string [owner:ui-engineer]
- [x] [6.5] [P-2] Add filter chips — by channel, by date range (today / last 7d / all), updates API query params [owner:ui-engineer]
- [x] [6.6] [P-3] Implement pagination controls — prev/next with current page indicator, keyboard accessible [owner:ui-engineer]

## UI Batch — Approval Queue Page

- [x] [7.1] [P-1] Create `apps/dashboard/src/app/(dashboard)/approvals/page.tsx` — split view layout: queue list (left) and detail panel (right), stacked on mobile, using PageShell [owner:ui-engineer]
- [x] [7.2] [P-1] Implement approval queue list — action type icon, title, project, created_at, urgency indicator; sorted by created_at descending [owner:ui-engineer]
- [x] [7.3] [P-1] Implement approval detail panel — full action description, proposed changes, context snippet, approve and dismiss buttons [owner:ui-engineer]
- [x] [7.4] [P-1] Wire approve button — `POST /api/approvals/:id/approve` with loading state and confirmation; dismiss button — `PATCH /api/obligations/:id` with `status: dismissed` [owner:ui-engineer]
- [x] [7.5] [P-2] Add approvals badge to sidebar nav item — count from `GET /api/obligations?owner=leo&status=open`, updates via WebSocket `approval.created` events [owner:ui-engineer]
- [x] [7.6] [P-2] Subscribe to WebSocket `approval.created` event — prepend new item to queue list in real-time [owner:ui-engineer]

## UI Batch — Settings Page

- [x] [8.1] [P-1] Rebuild `apps/dashboard/src/app/(dashboard)/settings/page.tsx` — replace auto-inferred field discovery with explicit section schema: Daemon, Channels, Integrations, Memory [owner:ui-engineer]
- [x] [8.2] [P-2] Render secret fields as read-only masked values (show `••••••••`) — no input element, no edit capability [owner:ui-engineer]
- [x] [8.3] [P-2] Add restart notice banner — shown when a field flagged `requires_restart: true` has an unsaved change [owner:ui-engineer]
- [x] [8.4] [P-2] Retain unsaved-changes sticky footer from current implementation [owner:ui-engineer]

## UI Batch — Mobile Layout

- [x] [9.1] [P-1] Rebuild sidebar for mobile — icon-only rail at ≤768px (`md:w-16`), hidden at ≤640px (`sm:hidden`) with hamburger menu toggle in a top bar [owner:ui-engineer]
- [x] [9.2] [P-1] Ensure all touch targets are minimum 44×44px (`min-h-11 min-w-11`) across all new components [owner:ui-engineer]
- [x] [9.3] [P-2] Verify main dashboard two-column grid collapses to single column at `md:` breakpoint [owner:ui-engineer]
- [x] [9.4] [P-2] Verify approval queue split view stacks vertically at `md:` breakpoint [owner:ui-engineer]
- [x] [9.5] [P-2] Add `/sessions/:id` detail route for mobile — session detail as full page instead of drawer [owner:ui-engineer]

## Verify

- [ ] [10.1] `pnpm build` passes (no TypeScript errors) [owner:ui-engineer]
- [ ] [10.2] `pnpm lint` passes [owner:ui-engineer]
- [ ] [10.3] All five pages render without runtime errors on desktop viewport (1280px) [owner:ui-engineer]
- [ ] [10.4] All five pages render without layout overflow on mobile viewport (375px) [owner:ui-engineer]
- [ ] [10.5] `GET /api/messages` returns paginated results with correct shape [owner:api-engineer]
- [ ] [10.6] `POST /api/approvals/:id/approve` returns `{ id, status: "approved" }` [owner:api-engineer]
- [ ] [10.7] WebSocket reconnect fires after connection drop — exponential backoff observed in browser devtools [owner:ui-engineer]
- [ ] [10.8] Approval queue badge count updates when a new `approval.created` event is received [owner:ui-engineer]
- [ ] [10.9] [user] Manual test: approve a pending action from the dashboard — confirm it executes in daemon [owner:ui-engineer]
- [ ] [10.10] [user] Manual test: check dashboard from phone (375px) — sidebar, sessions, approvals all navigable [owner:ui-engineer]
