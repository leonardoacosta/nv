# Proposal: Add Obligation Detection

## Change ID
`add-obligation-detection`

## Summary
Obligation detection pipeline: classify inbound messages via Claude, store obligations, notify on P0-P1.

## Context
- Extends: orchestrator.rs, new obligation_detector.rs
- Related: PRD FR-1 FR-2 FR-5

## Motivation
Required by Nova v4 PRD. See functional requirements FR-1 FR-2 FR-5.

## Requirements

### Req-1: Core implementation
Obligation detection pipeline: classify inbound messages via Claude, store obligations, notify on P0-P1.

## Scope
- **IN**: Obligation detection pipeline: classify inbound messages via Claude, store obligations, notify on P0-P1.
- **OUT**: Unrelated systems

## Impact
| Area | Change |
|------|--------|
| orchestrator.rs, new obligation_detector.rs | Implementation per PRD |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Stick to PRD requirements |
