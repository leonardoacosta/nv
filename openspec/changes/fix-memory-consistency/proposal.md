# Proposal: Fix Memory Consistency

## Change ID
`fix-memory-consistency`

## Summary
Update system-prompt.md to read memory before responding, add memory file listing to prompt injection.

## Context
- Extends: system-prompt.md, agent.rs
- Related: PRD functional requirements

## Motivation
Required by Nova v4 PRD.

## Requirements

### Req-1: Core implementation
Update system-prompt.md to read memory before responding, add memory file listing to prompt injection.

## Scope
- **IN**: Implementation as described
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| system-prompt.md, agent.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
