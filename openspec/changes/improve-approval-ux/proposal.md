# Proposal: Improve Approval UX

## Change ID
`improve-approval-ux`

## Summary

Add keyboard shortcuts for rapid approval processing, urgency color coding on queue items, batch approve/dismiss actions, and a "queue clear" celebration animation.

## Context
- Extends: `apps/dashboard/app/approvals/page.tsx`

## Motivation

The Approvals page is a high-frequency triage interface but lacks power-user affordances. All items look identical regardless of urgency. No keyboard shortcuts exist for the most common actions (approve, dismiss, navigate). When the queue clears, there is no positive feedback.

1. **Keyboard shortcuts** -- approve, dismiss, and navigate the queue without touching the mouse.
2. **Urgency color coding** -- visually distinguish items by urgency level so the operator can triage at a glance.
3. **Batch actions** -- process multiple items in one operation to reduce repetitive clicks.
4. **Queue celebration** -- provide positive feedback when the last item is cleared.

## Requirements

### Req-1: Keyboard Shortcuts

A for approve, D for dismiss, J/K or arrow keys to navigate queue, Enter to select. Show shortcut hints on hover.

### Req-2: Urgency Color Coding

Color-code queue items by urgency level: red border for urgent, amber for medium, default for low.

### Req-3: Batch Actions

Checkbox selection on queue items, "Approve All Selected" and "Dismiss All Selected" actions in a floating action bar.

### Req-4: Queue Celebration

When the last item is cleared, show a brief "All clear" animation (gentle fade-in of a success illustration, 800ms).

## Scope
- **IN**: Keyboard shortcuts, urgency colors, batch actions, celebration animation
- **OUT**: Approval creation, approval detail editing, notification changes

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/approvals/page.tsx` | Modified: add keyboard handler, urgency styling, batch selection, celebration |
| `apps/dashboard/app/approvals/components/` | New: `ApprovalQueueItem.tsx`, `BatchActionBar.tsx`, `QueueClearCelebration.tsx`, `useApprovalKeyboard.ts` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Keyboard shortcuts conflict with browser shortcuts | Only active when Approvals page is focused; use standard vim-like navigation (j/k) |
| Urgency level not present in approval data model | Fall back to "low" (default styling) when urgency is absent |
| Batch action accidentally processes wrong items | Require explicit checkbox selection; floating bar shows count of selected items |
