# Proposal: Add Server Health Metrics

## Change ID
`add-server-health-metrics`

## Summary
server_health table, Nexus health endpoint extension, 60s poll and store, /api/server-health endpoint.

## Context
- Extends: messages.rs (migration), nexus/client.rs, http.rs
- Related: PRD functional requirements

## Motivation
Required by Nova v4 PRD.

## Requirements

### Req-1: Core implementation
server_health table, Nexus health endpoint extension, 60s poll and store, /api/server-health endpoint.

## Scope
- **IN**: Implementation as described
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| messages.rs (migration), nexus/client.rs, http.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
