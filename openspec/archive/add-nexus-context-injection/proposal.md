# Proposal: Add Nexus Context Injection

## Change ID
`add-nexus-context-injection`

## Summary
Solve with Nexus flow: inject error context into Nexus start_session prompt as /openspec:explore with pre-loaded context.

## Context
- Extends: nexus/client.rs, tools/mod.rs, http.rs
- Related: PRD functional requirements

## Motivation
Required by Nova v4 PRD.

## Requirements

### Req-1: Core implementation
Solve with Nexus flow: inject error context into Nexus start_session prompt as /openspec:explore with pre-loaded context.

## Scope
- **IN**: Implementation as described
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| nexus/client.rs, tools/mod.rs, http.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
