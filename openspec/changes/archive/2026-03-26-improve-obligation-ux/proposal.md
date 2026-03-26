# Proposal: Improve Obligation UX

## Change ID
`improve-obligation-ux`

## Summary

Improve Obligations page UX by showing contextual actions per obligation state, adding deadline proximity visual indicators, and using inline expand/collapse for obligation details.

## Context
- Extends: `apps/dashboard/app/obligations/page.tsx`, `apps/dashboard/components/ObligationItem.tsx`
- Related: `add-obligation-system` (obligation data model and store)

## Motivation

Currently every obligation shows Cancel, Confirm Done, and Reopen buttons simultaneously even though they are mutually exclusive states. Items approaching deadline have no visual urgency indicator. The obligation list is dense with always-visible details.

1. **Action confusion** -- all three action buttons render regardless of obligation status, making it unclear which actions are valid for the current state
2. **No urgency signal** -- obligations approaching their deadline look identical to those with distant or no deadlines, providing no visual pressure
3. **Vertical density** -- every obligation item shows full details at all times, causing excessive scrolling and making it harder to scan the list
4. **Stat cards overhead** -- five stat cards consume significant vertical space at the top of the page before any obligation content is visible

## Requirements

### Req-1: Contextual Actions

Render action buttons conditionally based on obligation status:
- `open` / `in_progress`: show "Done" + "Cancel" icon buttons with tooltips
- `proposed_done`: show "Confirm Done" + "Reopen" icon buttons with tooltips
- `done`: show "Reopen" icon button with tooltip
- `dismissed`: no action buttons

Never show all three buttons simultaneously. Use Lucide icon buttons (`Check`, `X`, `RotateCcw`) with tooltips instead of full text labels.

### Req-2: Deadline Proximity Indicator

Obligations approaching deadline (within the `approaching_deadline_hours` config value) display an amber-to-red gradient border or glow. Overdue items get a solid red indicator. Items with no deadline or distant deadline display no indicator. The threshold is read from the daemon config; the dashboard receives it via the existing API contract.

### Req-3: Inline Expand/Collapse for Obligation Details

Obligation details are hidden by default. Clicking the obligation row expands details with a smooth CSS height-reveal animation (150-200ms ease). Reduces vertical space by approximately 60%. The first item in the list is expanded by default. Expand/collapse state persists within the session (not across page reloads).

### Req-4: Stat Card Summary Bar

Collapse the 5 stat cards at the top into a single compact summary bar. Show counts inline (e.g., "3 Open | 2 In Progress | 1 Proposed Done | 5 Done | 0 Dismissed") using colored badges. Reduces vertical stat area from ~120px to ~40px.

## Scope
- **IN**: Obligation card actions, deadline visuals, expand/collapse, stat compaction
- **OUT**: Obligation creation, obligation data model changes, new filtering options

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/obligations/page.tsx` | Refactor stat cards into summary bar; pass expand state |
| `apps/dashboard/components/ObligationItem.tsx` | Contextual actions, deadline indicator, expand/collapse |

## Risks
| Risk | Mitigation |
|------|-----------|
| Hiding details by default may confuse existing users | Expand first item by default; remember expand state within session |
| Deadline threshold not available from API | Fall back to 24 hours default if config value absent |
| Icon-only buttons may lack discoverability | Tooltips on hover; icon choice matches standard conventions (check, x, rotate) |
