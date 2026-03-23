# Proposal: Add Dashboard Api

## Change ID
`add-dashboard-api`

## Summary
REST API endpoints: /api/obligations, /api/projects, /api/sessions, /api/server-health, /api/memory, /api/config.

## Context
- Extends: http.rs
- Related: PRD FR-9

## Motivation
Required by Nova v4 PRD. See functional requirements FR-9.

## Requirements

### Req-1: Core implementation
REST API endpoints: /api/obligations, /api/projects, /api/sessions, /api/server-health, /api/memory, /api/config.

## Scope
- **IN**: REST API endpoints: /api/obligations, /api/projects, /api/sessions, /api/server-health, /api/memory, /api/config.
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| http.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
