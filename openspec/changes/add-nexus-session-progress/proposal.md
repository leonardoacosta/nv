# Proposal: Add Nexus Session Progress

## Change ID
`add-nexus-session-progress`

## Summary
Track progress for /apply, /ci:gh --fix, /feature sessions. Parse Nexus events for phase detection. Expose via /api/sessions.

## Context
- Extends: nexus/client.rs, new nexus/events.rs, http.rs
- Related: PRD functional requirements

## Motivation
Required by Nova v4 PRD.

## Requirements

### Req-1: Core implementation
Track progress for /apply, /ci:gh --fix, /feature sessions. Parse Nexus events for phase detection. Expose via /api/sessions.

## Scope
- **IN**: Implementation as described
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| nexus/client.rs, new nexus/events.rs, http.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
