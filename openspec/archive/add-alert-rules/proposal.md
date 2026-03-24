# Proposal: Add Alert Rules

## Change ID
`add-alert-rules`

## Summary
Alert rule system: deploy_failure, sentry_spike, stale_ticket, ha_anomaly rules that create obligations when triggered.

## Context
- Extends: messages.rs (migration), new alert_rules.rs, config.rs
- Related: PRD FR-6

## Motivation
Required by Nova v4 PRD. See functional requirements FR-6.

## Requirements

### Req-1: Core implementation
Alert rule system: deploy_failure, sentry_spike, stale_ticket, ha_anomaly rules that create obligations when triggered.

## Scope
- **IN**: Alert rule system: deploy_failure, sentry_spike, stale_ticket, ha_anomaly rules that create obligations when triggered.
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| messages.rs (migration), new alert_rules.rs, config.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
