# Implementation Tasks
<!-- beads:epic:nv-gzzz -->

## UI Batch

- [x] [1.1] [P-1] Create `apps/dashboard/app/approvals/components/useApprovalKeyboard.ts` -- custom hook that registers keydown listeners (A=approve, D=dismiss, J/K/ArrowUp/ArrowDown=navigate, Enter=select) scoped to the Approvals page; clean up listeners on unmount [owner:ui-engineer] [beads:nv-0n00]
- [x] [1.2] [P-1] Create `apps/dashboard/app/approvals/components/ApprovalQueueItem.tsx` -- queue item component with urgency color coding (red border for urgent, amber for medium, default for low), checkbox for batch selection, and shortcut hint tooltips on hover [owner:ui-engineer] [beads:nv-7g0l]
- [x] [1.3] [P-1] Create `apps/dashboard/app/approvals/components/BatchActionBar.tsx` -- floating action bar visible when 1+ items selected; shows selected count, "Approve All Selected" and "Dismiss All Selected" buttons; dispatches batch action and clears selection on completion [owner:ui-engineer] [beads:nv-0qp6]
- [x] [1.4] [P-2] Create `apps/dashboard/app/approvals/components/QueueClearCelebration.tsx` -- renders "All clear" success illustration with 800ms fade-in animation when queue reaches zero items; auto-dismisses after display [owner:ui-engineer] [beads:nv-qzr9]
- [x] [1.5] [P-1] Integrate components into `apps/dashboard/app/approvals/page.tsx` -- wire `useApprovalKeyboard`, replace existing queue items with `ApprovalQueueItem`, mount `BatchActionBar` and `QueueClearCelebration`; run `pnpm typecheck` with zero errors [owner:ui-engineer] [beads:nv-sqce]
