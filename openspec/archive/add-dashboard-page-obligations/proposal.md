# Proposal: Add Dashboard Page Obligations

## Change ID
`add-dashboard-page-obligations`

## Summary
Split NOVA CAN HANDLE / LEO MUST HANDLE sections, urgency-sorted, action buttons, handled today.

## Context
- Extends: dashboard/src/pages/
- Related: PRD FR-11, wireframes

## Motivation
Dashboard page per Nova v4 PRD and approved wireframes.

## Requirements

### Req-1: Page implementation
Split NOVA CAN HANDLE / LEO MUST HANDLE sections, urgency-sorted, action buttons, handled today. Reference wireframe for layout and interaction patterns.

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
