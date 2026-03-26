# Proposal: Improve Obligation Visibility

## Change ID
`improve-obligation-visibility`

## Summary

Three-pronged improvement to obligation visibility: richer dashboard obligations page with a
structured real-time activity feed, full Telegram obligation CRUD commands (/obligations, /ob done,
/ob assign, /ob create), and meaningful content in obligation cards.

## Context
- Extends: `apps/dashboard/app/obligations/page.tsx`, `crates/nv-daemon/src/orchestrator.rs`
  (bot commands), `crates/nv-daemon/src/http.rs` (activity feed endpoint),
  `crates/nv-daemon/src/obligation_store.rs`
- Related: `add-autonomous-obligation-execution` (just shipped — obligations now have
  `proposed_done` status and execution lifecycle), `proactive-followups` (watcher), `rebuild-dashboard-wireframes` (current dashboard layout)

## Motivation

Now that Nova autonomously executes obligations, Leo needs to see what's happening in real time.
The current obligations page shows cards with basic text but no execution history, no live feed,
and no way to manage obligations from Telegram without opening the dashboard. Leo should be able
to check on Nova's work from his phone (Telegram) and his desktop (dashboard) with equal depth.

## Requirements

### Req-1: Structured Activity Feed (Dashboard)

Add a real-time activity feed to the obligations dashboard page, delivered via the existing
WebSocket `/ws/events` infrastructure.

**New daemon event types** (add to `DaemonEvent` enum in `http.rs`):
- `obligation.detected` — when obligation_detector finds a new one
- `obligation.execution_started` — when idle executor picks up an obligation
- `obligation.tool_called` — each tool call during autonomous execution (tool_name, duration_ms)
- `obligation.execution_completed` — result summary, proposed_done or failed
- `obligation.confirmed` — Leo confirmed done
- `obligation.reopened` — Leo reopened

**Dashboard component**: `<ActivityFeed>` panel on the obligations page, right side or below the
obligation list. Shows the last 50 events in reverse chronological order. Each event has:
- Timestamp (relative, e.g., "2m ago")
- Event type icon (colored: green=completed, amber=tool_call, red=failed, blue=detected)
- Description (e.g., "Nova started working on: Pull Teams messages")
- Obligation ID link (click to scroll to the card)

Events arrive via WebSocket and prepend to the feed in real-time. Fallback: poll
`GET /api/obligations/activity?limit=50` every 10s.

### Req-2: Rich Obligation Cards (Dashboard)

Redesign the obligation card to show the full lifecycle:

**Card sections:**
1. **Header**: detected_action (title), status badge, priority pill, owner badge (Nova/Leo)
2. **Context**: source_channel icon + source_message (truncated to 200 chars, expandable)
3. **Execution History**: timeline of attempts:
   - Each attempt: timestamp, result (completed/failed/timeout), summary (truncated)
   - Most recent attempt shown expanded, older collapsed
4. **Research Notes**: from obligation_research (if any), collapsible section
5. **Actions**: buttons based on status:
   - `open`: [Start] (assigns to Nova for immediate execution)
   - `in_progress`: [Cancel] (mark dismissed)
   - `proposed_done`: [Confirm Done] [Reopen] (same as Telegram keyboard)
   - `done`: [Reopen]

**Data source**: Extend `GET /api/obligations` response to include:
- `notes: Vec<ObligationNote>` — from obligation_notes table
- `attempt_count: u32` — count of execution attempts
- `last_attempt_at: Option<String>` — from the new column

### Req-3: Dashboard Stats Bar

Add a stats bar at the top of the obligations page:

| Stat | Source |
|------|--------|
| Open (Nova) | count where owner=nova, status=open |
| In Progress | count where status=in_progress |
| Proposed Done | count where status=proposed_done |
| Completed Today | count where status=done, updated_at >= today |
| Open (Leo) | count where owner=leo, status=open |

Use StatCard components with appropriate icons and accent colors.

### Req-4: Telegram Obligation Commands

Add bot commands to the orchestrator's command dispatch:

**`/obligations`** (or `/ob`)
- Lists all open obligations, grouped by owner (Nova / Leo)
- Format: `{priority} {status_icon} {detected_action (truncated 60 chars)}\n  Owner: {owner} | {relative_time}`
- Max 10 items, "...and N more" footer if truncated

**`/ob done <id_prefix>`**
- Matches obligation by ID prefix (first 6+ chars of UUID)
- Transitions to `done` regardless of current status
- Replies: "Marked done: {detected_action (truncated)}"

**`/ob assign <id_prefix> nova|leo`**
- Changes obligation owner
- Replies: "Assigned to {owner}: {detected_action}"

**`/ob create <text>`**
- Creates a new obligation with:
  - `detected_action` = the provided text
  - `owner` = "nova" (default, Nova will pick it up when idle)
  - `priority` = 2
  - `source_channel` = "telegram"
  - `status` = "open"
- Replies: "Created obligation: {text} (assigned to Nova)"

**`/ob status`**
- Summary: "Nova: 3 open, 2 in progress, 1 proposed done | Leo: 5 open"

### Req-5: Obligation Activity API Endpoint

Add `GET /api/obligations/activity?limit=50` to the daemon HTTP server.

Returns the last N obligation-related events from an in-memory ring buffer (capacity 200).
Events are also broadcast via WebSocket.

The ring buffer is populated by the obligation_executor, obligation_detector, and callback
handlers — each appends an `ObligationActivityEvent` when something happens.

## Scope
- **IN**: Activity feed (WebSocket + API), rich obligation cards, stats bar, 5 Telegram commands,
  activity API endpoint, obligation notes in API response
- **OUT**: Obligation editing from dashboard (text changes), obligation priority auto-adjustment,
  obligation decomposition into sub-tasks, push notifications (Telegram IS the notification)

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/http.rs` | New: `GET /api/obligations/activity`, extend obligations response with notes |
| `crates/nv-daemon/src/obligation_store.rs` | Add `list_notes(id)`, `get_stats()` methods |
| `crates/nv-daemon/src/orchestrator.rs` | Add /ob command dispatch (5 commands) |
| `crates/nv-daemon/src/obligation_executor.rs` | Emit activity events on start/tool_call/complete |
| `crates/nv-daemon/src/obligation_detector.rs` | Emit activity event on detection |
| `apps/dashboard/app/obligations/page.tsx` | Redesign: stats bar, rich cards, activity feed |
| `apps/dashboard/app/api/obligations/activity/route.ts` | New proxy route |
| `apps/dashboard/types/api.ts` | Add ObligationNote, ObligationActivity types |

## Risks

| Risk | Mitigation |
|------|-----------|
| Activity feed volume overwhelms dashboard | Ring buffer capped at 200; UI shows last 50 |
| Telegram command parsing ambiguity | Strict prefix matching on /ob subcommands |
| Obligation notes table grows large | Notes are append-only but pruned per obligation (keep last 10) |
| WebSocket not connected | Fallback polling every 10s |
