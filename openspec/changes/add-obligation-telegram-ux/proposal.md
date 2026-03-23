# Proposal: Add Obligation Telegram Ux

## Change ID
`add-obligation-telegram-ux`

## Summary
Obligation notifications on Telegram with inline keyboard, morning briefing digest with obligation queue summary.

## Context
- Extends: channels/telegram/client.rs, orchestrator.rs
- Related: PRD functional requirements

## Motivation
Required by Nova v4 PRD.

## Requirements

### Req-1: Core implementation
Obligation notifications on Telegram with inline keyboard, morning briefing digest with obligation queue summary.

## Scope
- **IN**: Implementation as described
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| channels/telegram/client.rs, orchestrator.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
