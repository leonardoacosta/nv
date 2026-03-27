# cc-sessions-dashboard-widget

## ADDED Requirements

### Requirement: CC Sessions summary widget on dashboard

The dashboard SHALL add a compact CC Sessions widget to the main dashboard page (`apps/dashboard/app/page.tsx`). The widget MUST display the count of currently running CC sessions (fetched from `GET /api/cc-sessions`), a status indicator (green dot if any running, gray if none), and a "View all" link to `/sessions?panel=cc` or a dedicated CC sessions view. The widget MUST fit within the existing dashboard grid layout using the `surface-card` pattern.

#### Scenario: Active CC sessions shown

Given 3 CC sessions are currently running,
when the dashboard loads,
then the CC Sessions widget shows "3 running" with a green status dot.

#### Scenario: No active CC sessions

Given no CC sessions are running,
when the dashboard loads,
then the widget shows "0 sessions" with a gray status dot.

#### Scenario: Widget fetch failure

Given the `/api/cc-sessions` endpoint returns an error,
when the dashboard loads,
then the CC Sessions widget shows a muted error state without disrupting other dashboard widgets.

## REMOVED Requirements

### Requirement: CC Session panel on sessions page

The sessions page MUST remove the CC Session panel toggle button, the `CCSessionPanel` component import, and the collapsible panel section. The `CCSessionPanel` component file and `SessionDashboard` component SHALL be retained for potential reuse but MUST no longer be rendered on the sessions page.

#### Scenario: CC panel removed from sessions page

Given the sessions page is loaded,
when the page renders,
then no CC Session toggle button or panel is present in the DOM.
