# Proposal: Restructure Navigation

## Change ID
`restructure-navigation`

## Summary

Reduce sidebar from 15 to 11 items by merging Cold Starts into Usage as a tab, CC Session into
Sessions as a panel, moving Memory from SYSTEM to DATA group, and optionally consolidating Diary
into Dashboard.

## Context
- Phase: Polish | Wave: TBD
- Stack: Next.js (App Router), React, Tailwind CSS
- Extends: `apps/dashboard/components/Sidebar.tsx`, `apps/dashboard/app/usage/page.tsx`,
  `apps/dashboard/app/sessions/page.tsx`
- Related: `apps/dashboard/app/cold-starts/` (to be removed),
  `apps/dashboard/app/session/` (to be removed), `next.config.ts` (redirects)

## Motivation

15 nav items across 4 groups creates cognitive overload. The System group alone has 5 items --
Usage and Cold Starts both show analytics, Sessions and CC Session both manage sessions. Users
cannot quickly find what they need.

Key motivations:

1. **Cognitive load** -- 15 sidebar items exceed the 7 +/- 2 working memory threshold. Reducing to
   11 brings navigation closer to scannable territory.
2. **Semantic overlap** -- Cold Starts is a subset of usage analytics; CC Session is a specialized
   session view. Both belong alongside their parent concepts, not as separate top-level entries.
3. **Group imbalance** -- SYSTEM has 5 items while DATA has 3. Moving Memory to DATA rebalances
   the groups (4/4).
4. **UX inventory finding** -- identified as P2 strategic improvement during dashboard UX audit.

## Requirements

### Req-1: Merge Cold Starts into Usage Page as "Performance" Tab
Add a "Performance" tab to the existing Usage page (`apps/dashboard/app/usage/page.tsx`) that
displays the current Cold Starts content alongside existing cost analytics.

### Req-2: Merge CC Session into Sessions Page as Dedicated Panel
Add a CC Session panel/card at the top of the Sessions page (`apps/dashboard/app/sessions/page.tsx`)
that displays the current CC Session content inline with the sessions list.

### Req-3: Move Memory from SYSTEM Nav Group to DATA Group
Reorder the Memory nav item in `Sidebar.tsx` from the SYSTEM group to the DATA group, positioned
between Projects and Integrations.

### Req-4: Add Next.js Redirects for Old Routes
Add permanent (301) redirects in `next.config.ts`:
- `/cold-starts` redirects to `/usage?tab=performance`
- `/session` redirects to `/sessions?panel=cc`

### Req-5: Update Sidebar.tsx Nav Items
Remove Cold Starts and CC Session entries from the sidebar nav configuration. Reorder Memory into
the DATA group. Net result: 15 items reduced to 11 (Cold Starts removed, CC Session removed,
Memory moved, Diary consolidation deferred).

## Scope
- **IN**: Nav restructuring, page merges (Cold Starts into Usage, CC Session into Sessions), route
  redirects, Sidebar.tsx item updates, Memory group reassignment
- **OUT**: New features on merged pages, visual redesign of merged content, Diary consolidation
  (deferred to separate proposal), mobile responsive changes

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/components/Sidebar.tsx` | Remove 2 nav items, move Memory to DATA group |
| `apps/dashboard/app/usage/page.tsx` | Add "Performance" tab with Cold Starts content |
| `apps/dashboard/app/sessions/page.tsx` | Add CC Session panel at top of sessions list |
| `apps/dashboard/app/cold-starts/` | Remove directory (content moved to usage) |
| `apps/dashboard/app/session/` | Remove directory (content moved to sessions) |
| `next.config.ts` | Add 2 permanent redirects |

## Risks
| Risk | Mitigation |
|------|-----------|
| Bookmarked URLs break for `/cold-starts` and `/session` | 301 redirects in `next.config.ts` preserve all existing links |
| Users accustomed to separate nav items cannot find merged content | Tab/panel labels match original page names; redirects guide users automatically |
| Cold Starts page has unique data fetching that conflicts with Usage | Extract shared components; tab content loads independently via its own query |
