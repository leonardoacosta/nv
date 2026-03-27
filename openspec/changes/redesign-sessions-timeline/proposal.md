# Proposal: Redesign Sessions Timeline

## Change ID
`redesign-sessions-timeline`

## Summary

Redesign the Sessions page from a live daemon session manager into a historical session timeline viewer with per-session interaction drill-down, and relocate the CC Sessions live panel to a compact dashboard widget.

## Context
- Extends: `apps/dashboard/app/sessions/page.tsx`, `apps/dashboard/app/sessions/[id]/page.tsx`, `apps/dashboard/app/api/sessions/`, `apps/dashboard/app/api/cc-sessions/route.ts`, `packages/db/src/schema/sessions.ts`, `packages/db/src/schema/diary.ts`
- Related: `unify-conversation-streaming` (active -- adds WebSocket event streaming and conversation persistence; no overlap with session timeline UI or schema)
- Dashboard: Next.js 15 App Router, `apiFetch` client, `PageShell`/`SectionHeader` layout components, `ds-*` design tokens

## Motivation

The current Sessions page is a live daemon session manager: it connects via WebSocket to track active/idle/completed Claude Code instances, shows daemon reconnection banners, and renders a CC Sessions table with restart counts. This is operationally useful during active sessions but provides no historical value. Once a session completes, its context is lost -- there is no way to review what happened, which tools were called, or how long interactions took.

The sessions DB table stores only bare metadata (project, command, status, timestamps) with no per-interaction detail. The diary table has richer interaction data (trigger_type, tools_used, tokens) but is not linked to sessions. This means the dashboard cannot answer "what did Nova do in this session?" or "show me all sessions for project X last week."

Redesigning the page into a reverse-chronological timeline of historical sessions -- with expandable interaction detail -- transforms it from a monitoring tool into an audit and review surface. Moving the CC Sessions live panel to a small dashboard widget preserves live monitoring without cluttering the historical view.

## Requirements

### Req-1: Extend Sessions Schema with Interaction Tracking

Add columns to the `sessions` table for trigger metadata and aggregate counters. Add a new `session_events` table to store per-session interactions (messages, tool calls, API requests) with a foreign key to `sessions.id`.

### Req-2: Historical Session Timeline List

Replace the current sessions page with a reverse-chronological, paginated list of ALL sessions from the DB. Each row displays: project name, duration, message count, tool call count, status badge, and relative timestamp. Remove all WebSocket/daemon-status UI, active/idle/completed grouping, and real-time merge logic from this page.

### Req-3: Session Timeline Filters

Add filter controls: project dropdown (from distinct projects in sessions table), date range picker (start/end date inputs), and trigger type selector (manual/watcher/briefing/all). Filters apply server-side via query parameters to the sessions API endpoint. Retain the existing text search capability.

### Req-4: Session Detail Interaction Timeline

Redesign the session detail page (`/sessions/[id]`) to show a vertical timeline of every interaction within that session. Messages display with direction arrows (user/assistant). Tool calls show name, truncated inputs/outputs with expand toggle. API requests show method, endpoint, and status code. Remove the daemon real-time update subscription.

### Req-5: CC Sessions Dashboard Widget

Move the CC Sessions live panel (currently a collapsible section on the sessions page) to a compact widget on the main dashboard page. The widget shows a summary count of running CC sessions with a link to the full CC sessions view. Remove the CC Session panel toggle and `CCSessionPanel` import from the sessions page.

### Req-6: Sessions API Enhancements

Extend `GET /api/sessions` to accept query parameters for pagination (`page`, `limit`), filtering (`project`, `trigger_type`, `date_from`, `date_to`), and sorting. Add a new `GET /api/sessions/[id]/events` endpoint that returns the ordered list of session events for the detail timeline.

## Scope
- **IN**: Schema extension (sessions columns + session_events table), sessions list page rewrite, session detail page rewrite, dashboard CC widget, API pagination/filtering/events endpoint
- **OUT**: Modifying the daemon session creation logic, changing how sessions are started/stopped, WebSocket event streaming (handled by `unify-conversation-streaming`), session cost/billing calculations, session replay/playback functionality

## Impact
| Area | Change |
|------|--------|
| DB schema | New `session_events` table, new columns on `sessions` (trigger_type, message_count, tool_count) |
| API routes | Extended `/api/sessions` with pagination/filters, new `/api/sessions/[id]/events` |
| Sessions page | Full rewrite from live manager to historical timeline list |
| Session detail page | Full rewrite from stat tiles to vertical interaction timeline |
| Dashboard page | Add CC Sessions summary widget |
| Types | New `SessionEvent`, `SessionTimelineResponse`, `SessionEventsResponse` types |

## Risks
| Risk | Mitigation |
|------|-----------|
| Session events table grows large over time | Add `created_at` index, implement pagination with cursor-based offset, consider retention policy later |
| Diary entries not linked to sessions retroactively | Migration only adds schema; backfill is out of scope -- new sessions will populate events going forward |
| CC Sessions widget adds load to dashboard | Widget fetches only a count/summary, not full session list |
| Active spec `unify-conversation-streaming` touches conversation persistence | No overlap -- that spec handles message persistence and WebSocket streaming, not session timeline UI or schema |
