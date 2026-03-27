# session-timeline-list

## MODIFIED Requirements

### Requirement: Reverse-chronological session timeline

The page SHALL replace the current sessions page (`apps/dashboard/app/sessions/page.tsx`) with a paginated, reverse-chronological list of all historical sessions. Each session row MUST display: project name, computed duration (started_at to stopped_at or "running"), message_count, tool_count, status badge, trigger_type badge, and relative timestamp. The page MUST remove all WebSocket/daemon real-time logic (`useDaemonEvents`, `useDaemonStatus`, `DaemonOfflineBanner`, session map merge, real-time overlay). The page MUST remove the CC Session panel toggle and `CCSessionPanel` import. The `SessionAnalytics` section SHALL be retained at the top.

#### Scenario: Page loads with historical sessions

Given the sessions table has 50 completed sessions,
when the user navigates to /sessions,
then the page displays the first page of sessions in reverse-chronological order (newest first) with project, duration, message count, tool count, status, and timestamp per row.

#### Scenario: Pagination

Given more sessions exist than the page limit (default 25),
when the user clicks "Load more" or scrolls to the pagination control,
then the next page of sessions is fetched and appended.

#### Scenario: Empty state

Given the sessions table has no sessions,
when the user navigates to /sessions,
then a clean empty state is shown without daemon-related messaging (no "Daemon offline" banners).

### Requirement: Session timeline filters

The page SHALL add filter controls above the session list: a project dropdown populated from distinct session projects, a date range picker (two date inputs for start/end), and a trigger type selector (all/manual/watcher/briefing). Filters MUST be applied server-side by passing query parameters to `GET /api/sessions`. The existing text search input SHALL be retained. A "Clear filters" button MUST reset all filters.

#### Scenario: Filter by project

Given sessions exist for projects "nv", "oo", and "tc",
when the user selects "nv" from the project dropdown,
then only sessions with project "nv" are displayed.

#### Scenario: Filter by date range

Given sessions exist across multiple dates,
when the user sets date_from to "2026-03-20" and date_to to "2026-03-25",
then only sessions started within that range are displayed.

#### Scenario: Filter by trigger type

Given sessions exist with trigger_type "manual" and "watcher",
when the user selects "watcher" from the trigger dropdown,
then only sessions with trigger_type "watcher" are displayed.

#### Scenario: Combined filters

Given multiple filter controls are set simultaneously,
when the API request is made,
then all active filters are combined with AND logic.

### Requirement: Click-through to session detail

Each session row MUST be a clickable link navigating to `/sessions/[id]` for the full interaction timeline. The page SHALL remove the slide-out drawer (`SessionDetailDrawer`) in favor of full-page navigation.

#### Scenario: Session row click navigates to detail

Given a session row is displayed in the timeline list,
when the user clicks the row,
then the browser navigates to `/sessions/{session.id}`.
