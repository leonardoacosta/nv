# Proposal: Add Dashboard Page Projects

## Change ID
`add-dashboard-page-projects`

## Summary
Project table with provider icons, accordion detail, Solve with Nexus per error, Nova notes.

## Context
- Extends: dashboard/src/pages/
- Related: PRD FR-11, wireframes

## Motivation
Dashboard page per Nova v4 PRD and approved wireframes.

## Requirements

### Req-1: Page implementation
Project table with provider icons, accordion detail, Solve with Nexus per error, Nova notes. Reference wireframe for layout and interaction patterns.

## Scope
- **IN**: React page component, child components, API integration
- **OUT**: API endpoints (in add-dashboard-api), other pages

## Impact
| Area | Change |
|------|--------|
| dashboard/src/pages/ | New page component |

## Risks
| Risk | Mitigation |
|------|-----------|
| Design drift from wireframes | Reference locked wireframes during implementation |
