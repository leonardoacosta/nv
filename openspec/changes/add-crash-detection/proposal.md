# Proposal: Add Crash Detection

## Change ID
`add-crash-detection`

## Summary
Detect server crashes via uptime decrease, create P1 obligation, spawn investigation session, store cause and recommendation.

## Context
- Extends: nexus/client.rs, obligation_detector.rs, new watchers/server.rs
- Related: PRD functional requirements

## Motivation
Required by Nova v4 PRD.

## Requirements

### Req-1: Core implementation
Detect server crashes via uptime decrease, create P1 obligation, spawn investigation session, store cause and recommendation.

## Scope
- **IN**: Implementation as described
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| nexus/client.rs, obligation_detector.rs, new watchers/server.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
