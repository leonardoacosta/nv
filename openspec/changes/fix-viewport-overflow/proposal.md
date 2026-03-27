# Proposal: Fix Viewport Overflow

## Change ID
`fix-viewport-overflow`

## Summary
Fix the main layout viewport overflow bug where AppShell uses `min-h-dvh` instead of `h-dvh`, causing the entire page to scroll rather than constraining scroll to the main content panel.

## Context
- Extends: `apps/dashboard/components/AppShell.tsx` (root layout wrapper)
- Related: `apps/dashboard/components/layout/PageShell.tsx` (page-level wrapper, already correct but depends on parent height constraint)

## Motivation
The dashboard AppShell wrapper currently uses `min-h-dvh` which allows the page body to grow beyond viewport height. This causes full-page scrolling instead of scroll being isolated to the main content area. The sidebar visually appears correct because it has its own height constraint, but the main content panel pushes the entire document height beyond the viewport. Fixing this ensures the dashboard behaves as a fixed-viewport application where only the content panel scrolls.

## Requirements

### Req-1: Constrain AppShell to exact viewport height
The root layout wrapper must use `h-dvh` (exact viewport height) with `overflow-hidden` to prevent any content from pushing the document beyond viewport bounds.

### Req-2: Isolate scroll to main content area
The `<main>` element must remain the sole scrollable region. The sidebar and page header must stay fixed in place while content scrolls.

## Scope
- **IN**: CSS class changes in AppShell.tsx (`min-h-dvh` to `h-dvh`, add `overflow-hidden`)
- **OUT**: Sidebar styling (already correct), PageShell changes (already uses `h-full` + `overflow-auto` correctly), any JavaScript/logic changes, API changes

## Impact
| Area | Change |
|------|--------|
| AppShell.tsx | `min-h-dvh` replaced with `h-dvh overflow-hidden` on wrapper div |
| All dashboard pages | Content panel scroll becomes isolated (visual improvement) |
| Login page | No change (bypasses AppShell) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Content cut off if nested elements assume unbounded height | PageShell already uses `flex-1 overflow-auto` on content area -- no issue |
| Mobile viewport quirks with `dvh` | Already using `dvh` (dynamic viewport height) -- no regression |
