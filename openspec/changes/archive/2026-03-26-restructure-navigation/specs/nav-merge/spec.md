# Capability: Navigation Restructure — Merge and Regroup

## MODIFIED Requirements

### Requirement: Usage page supports tabbed layout with Performance tab
The Usage page (`apps/dashboard/app/usage/page.tsx`) SHALL render a tab bar with "Cost" and "Performance" tabs. The active tab MUST be controlled by the `?tab` query parameter. When `tab=performance`, the page SHALL render the `ColdStartsPanel` component containing all content currently in `apps/dashboard/app/cold-starts/page.tsx`. When no `tab` param is present, the page MUST default to the "Cost" tab showing existing usage content.

#### Scenario: Default tab is Cost
Given a user navigates to `/usage` with no query params
When the page renders
Then the "Cost" tab is active and existing usage analytics are displayed

#### Scenario: Performance tab shows Cold Starts content
Given a user navigates to `/usage?tab=performance`
When the page renders
Then the "Performance" tab is active and Cold Starts analytics are displayed

#### Scenario: Invalid tab param falls back to Cost
Given a user navigates to `/usage?tab=invalid`
When the page renders
Then the "Cost" tab is active by default

### Requirement: Sessions page includes CC Session panel
The Sessions page (`apps/dashboard/app/sessions/page.tsx`) SHALL render a `CCSessionPanel` component as a card at the top of the sessions list. The panel MUST be visible when the `?panel=cc` query parameter is present. When no `panel` param is present, the CC Session panel MUST still be visible as a persistent card above the sessions list.

#### Scenario: CC Session panel visible by default
Given a user navigates to `/sessions`
When the page renders
Then the CC Session panel card is displayed above the sessions list

#### Scenario: Direct link to CC Session panel
Given a user navigates to `/sessions?panel=cc`
When the page renders
Then the CC Session panel card is displayed and scrolled into view

### Requirement: Sidebar nav items reduced from 15 to 11
The Sidebar component (`apps/dashboard/components/Sidebar.tsx`) MUST remove the "Cold Starts" and "CC Session" nav entries. The "Memory" item MUST be moved from the SYSTEM group to the DATA group, positioned between "Projects" and "Integrations". The total visible nav item count SHALL be 11.

#### Scenario: Cold Starts no longer appears in sidebar
Given the dashboard is loaded
When the user inspects the sidebar
Then there is no "Cold Starts" nav item

#### Scenario: CC Session no longer appears in sidebar
Given the dashboard is loaded
When the user inspects the sidebar
Then there is no "CC Session" nav item

#### Scenario: Memory appears in DATA group
Given the dashboard is loaded
When the user inspects the DATA group in the sidebar
Then "Memory" appears between "Projects" and "Integrations"

## ADDED Requirements

### Requirement: Old routes redirect permanently to new locations
`next.config.ts` SHALL define permanent redirects (status 301): `/cold-starts` MUST redirect to `/usage?tab=performance`, and `/session` MUST redirect to `/sessions?panel=cc`. The redirects MUST be permanent to update search engine indexes and browser caches.

#### Scenario: /cold-starts redirects to usage performance tab
Given a user or bookmark navigates to `/cold-starts`
When the request reaches the server
Then the response is a 301 redirect to `/usage?tab=performance`

#### Scenario: /session redirects to sessions CC panel
Given a user or bookmark navigates to `/session`
When the request reaches the server
Then the response is a 301 redirect to `/sessions?panel=cc`

#### Scenario: Redirects preserve additional query params
Given a user navigates to `/cold-starts?debug=true`
When the request reaches the server
Then the response is a 301 redirect to `/usage?tab=performance&debug=true`

### Requirement: Removed page directories are deleted
The directories `apps/dashboard/app/cold-starts/` and `apps/dashboard/app/session/` SHALL be deleted after their content is extracted into reusable components (`ColdStartsPanel` and `CCSessionPanel` respectively). No orphan route handlers MUST remain.

#### Scenario: /cold-starts route no longer resolves to a page component
Given the `apps/dashboard/app/cold-starts/` directory has been removed
When a request to `/cold-starts` bypasses the redirect (e.g., direct file access)
Then Next.js returns a 404

#### Scenario: /session route no longer resolves to a page component
Given the `apps/dashboard/app/session/` directory has been removed
When a request to `/session` bypasses the redirect
Then Next.js returns a 404
