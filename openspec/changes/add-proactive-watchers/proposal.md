# Proposal: Add Proactive Watchers

## Change ID
`add-proactive-watchers`

## Summary
Cron-triggered watchers: deploy_watcher, sentry_watcher, stale_ticket_watcher, ha_watcher. Each evaluates alert rules and creates obligations.

## Context
- Extends: orchestrator.rs, new watchers/ module
- Related: PRD FR-7

## Motivation
Required by Nova v4 PRD. See functional requirements FR-7.

## Requirements

### Req-1: Core implementation
Cron-triggered watchers: deploy_watcher, sentry_watcher, stale_ticket_watcher, ha_watcher. Each evaluates alert rules and creates obligations.

## Scope
- **IN**: Cron-triggered watchers: deploy_watcher, sentry_watcher, stale_ticket_watcher, ha_watcher. Each evaluates alert rules and creates obligations.
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| orchestrator.rs, new watchers/ module | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
