# Proposal: Add Dashboard Page Dashboard

## Change ID
`add-dashboard-page-dashboard`

## Summary
Recent sessions feed with trigger icons, Leo involvement, service tags, today summary cards.

## Context
- Extends: dashboard/src/pages/
- Related: PRD FR-11, wireframes

## Motivation
Dashboard page per Nova v4 PRD and approved wireframes.

## Requirements

### Req-1: Page implementation
Recent sessions feed with trigger icons, Leo involvement, service tags, today summary cards. Reference wireframe for layout and interaction patterns.

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
