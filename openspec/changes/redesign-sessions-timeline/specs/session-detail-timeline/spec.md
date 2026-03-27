# session-detail-timeline

## MODIFIED Requirements

### Requirement: Vertical interaction timeline

The page SHALL redesign the session detail page (`apps/dashboard/app/sessions/[id]/page.tsx`) to show a vertical timeline of all events within the session. The page header MUST display session metadata (project, status, duration, trigger_type, model, token counts, cost). Below the header, a vertical timeline SHALL render each `session_event` ordered by `created_at`.

Message events MUST display with a direction arrow (right-pointing for user/inbound, left-pointing for assistant/outbound) and the message content. Tool call events MUST display the tool name, a truncated preview of inputs, and an expand/collapse toggle to reveal full inputs and outputs. API request events MUST display the HTTP method, endpoint path, and status code badge (green for 2xx, amber for 4xx, red for 5xx).

The page MUST remove the daemon real-time update subscription (`useDaemonEvents`). The page SHALL fetch all data from `GET /api/sessions/[id]` (session metadata) and `GET /api/sessions/[id]/events` (timeline events).

#### Scenario: Session with mixed event types

Given a session has 5 message events, 3 tool call events, and 2 API request events,
when the user navigates to /sessions/{id},
then all 10 events are rendered in chronological order in a vertical timeline with appropriate icons and formatting per event type.

#### Scenario: Tool call expand/collapse

Given a tool call event is displayed with truncated inputs,
when the user clicks the expand toggle,
then the full inputs and outputs JSON are revealed,
and clicking again collapses them.

#### Scenario: Empty session (no events)

Given a session exists but has no session_events,
when the user navigates to /sessions/{id},
then the session metadata header is shown with an empty state message in the timeline area ("No interactions recorded for this session").

#### Scenario: Session metadata display

Given a session with project "nv", status "completed", duration 45 minutes, model "opus-4", and cost $0.12,
when the page loads,
then the header shows all metadata fields in stat tiles similar to the current design.

### Requirement: Back navigation

The page header MUST include a "Back to Sessions" link that navigates to `/sessions`, preserving any active filter state via URL search params.

#### Scenario: Return to filtered list

Given the user navigated from a filtered session list (project=nv),
when the user clicks "Back to Sessions",
then the browser navigates to /sessions with the project filter preserved.
