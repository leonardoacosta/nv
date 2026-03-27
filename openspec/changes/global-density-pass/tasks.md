# Implementation Tasks

## UI Batch 1 -- Core Layout Components

- [x] [1.1] [P-1] Update `apps/dashboard/components/layout/PageShell.tsx` -- remove `max-w-6xl mx-auto` from content wrapper; reduce header padding from `py-5` to `py-3`; reduce content padding from `py-6` to `py-4`; change title from `text-heading-24` to `text-heading-20`; change subtitle from `text-copy-14` to `text-copy-13` [owner:ui-engineer]
- [x] [1.2] [P-1] Update `apps/dashboard/components/layout/EmptyState.tsx` -- reduce default padding from `py-16 px-6` to `py-4 px-4`; reduce icon container from `w-10 h-10` to `w-6 h-6`; reduce default icon size from `40` to `20`; change title from `text-heading-16` to `text-copy-14 font-medium`; change description from `text-copy-14` to `text-copy-13`; add an `inline` boolean prop that renders as a single line `<p className="text-copy-13 text-ds-gray-900 py-3">` with just the title text and no icon [owner:ui-engineer]
- [x] [1.3] [P-2] Update `apps/dashboard/components/layout/StatCard.tsx` -- add an `inline` boolean prop; when true, render as a flat row (no `surface-card`, no border-radius, no box-shadow, no accent bar) with icon + label + value in a single horizontal flex line separated by `border-r border-ds-gray-400` on the right edge; keep existing card variant as default [owner:ui-engineer]
- [x] [1.4] [P-1] Update `apps/dashboard/components/Sidebar.tsx` -- reduce nav item padding from `py-2 min-h-9` to `py-1.5 min-h-8`; reduce icon size from `18` to `16`; reduce group label padding from `pt-2 pb-1` to `pt-1.5 pb-0.5`; reduce logo section `py-4` to `py-3`; reduce WS status footer `py-2.5 min-h-11` to `py-2 min-h-9`; remove Approvals item from NAV_GROUPS Activity section; remove Usage item from NAV_GROUPS System section; remove Settings item from NAV_GROUPS System section; add a settings gear icon button to the sidebar footer (before logout button) that navigates to `/settings` [owner:ui-engineer]

## UI Batch 2 -- Dashboard Page

- [x] [2.1] [P-1] Update `apps/dashboard/app/page.tsx` -- replace `space-y-6` with `space-y-3` throughout; replace `surface-card` sidebar panels (ObligationsSidebar, Session Breakdown, Messages) with `border-b border-ds-gray-400` sections using simple `py-3` padding instead of `p-4`; reduce grid gap from `gap-6` to `gap-4` [owner:ui-engineer]
- [x] [2.2] [P-2] Update dashboard stat cards in `apps/dashboard/app/page.tsx` -- use the new `inline` StatCard variant; render operational and performance stat rows as horizontal border-separated rows instead of grid cards; remove the `surface-card` containers; reduce section label `mb-2` to `mb-1` [owner:ui-engineer]

## UI Batch 3 -- Obligations + Approvals Merge

- [x] [3.1] [P-1] Update `apps/dashboard/app/obligations/page.tsx` -- reduce `p-8` to `p-4`; replace `space-y-6` with `space-y-3`; reduce `max-w-7xl` to full width; add a third tab "Approvals" alongside "Active" and "History"; when Approvals tab is active, render the approval queue and detail panel from existing approval components; read initial tab from `?tab=` search param [owner:ui-engineer]
- [x] [3.2] [P-2] Update `apps/dashboard/app/approvals/page.tsx` -- replace page content with a redirect to `/obligations?tab=approvals` using `redirect()` from `next/navigation`; keep the file for backward compatibility [owner:ui-engineer]
- [x] [3.3] [P-2] Move approval sub-components from `apps/dashboard/app/approvals/components/` to `apps/dashboard/components/approvals/` so they can be imported by the obligations page without cross-route imports [owner:ui-engineer]

## UI Batch 4 -- Page-by-Page Density

- [x] [4.1] [P-2] Update `apps/dashboard/app/diary/page.tsx` -- replace 3 StatCard grid with an inline stat row using border separators; reduce `space-y-6` to `space-y-3`; reduce header title from `text-heading-24` to `text-heading-20`; reduce `p-6 sm:p-8` to `p-4`; remove `max-w-3xl` constraint [owner:ui-engineer]
- [x] [4.2] [P-2] Update `apps/dashboard/app/sessions/page.tsx` -- reduce inter-section spacing to `space-y-3`; replace session row cards with `border-b` rows; replace full EmptyState with inline empty text [owner:ui-engineer]
- [x] [4.3] [P-2] Update `apps/dashboard/app/contacts/page.tsx` -- reduce spacing; replace contact row cards with `border-b` rows; replace full EmptyState with inline empty text [owner:ui-engineer]
- [x] [4.4] [P-3] Update `apps/dashboard/app/usage/page.tsx` -- reduce spacing; tighten stat card layout; replace full EmptyState with inline empty text [owner:ui-engineer]
- [x] [4.5] [P-3] Update `apps/dashboard/app/settings/page.tsx` -- reduce spacing between settings sections [owner:ui-engineer]
- [x] [4.6] [P-3] Update `apps/dashboard/app/messages/page.tsx` -- reduce spacing; replace full EmptyState with inline empty text [owner:ui-engineer]
- [x] [4.7] [P-3] Update `apps/dashboard/app/briefing/page.tsx` -- reduce spacing; replace full EmptyState with inline empty text [owner:ui-engineer]
- [x] [4.8] [P-3] Update `apps/dashboard/app/memory/page.tsx` -- reduce spacing; replace full EmptyState with inline empty text [owner:ui-engineer]
- [x] [4.9] [P-3] Update `apps/dashboard/app/integrations/page.tsx` -- reduce spacing; replace full EmptyState with inline empty text [owner:ui-engineer]
- [x] [4.10] [P-3] Update `apps/dashboard/app/projects/page.tsx` -- reduce spacing; replace full EmptyState with inline empty text [owner:ui-engineer]

## Verify

- [x] [5.1] `cd apps/dashboard && pnpm typecheck` passes -- zero TypeScript errors [owner:ui-engineer]
- [x] [5.2] `cd apps/dashboard && pnpm build` passes -- no build errors [owner:ui-engineer]
- [ ] [5.3] [user] Visual review: verify PageShell content fills edge-to-edge (no max-width gutters on wide display)
- [ ] [5.4] [user] Visual review: verify dashboard stat cards render as inline border-separated row
- [ ] [5.5] [user] Visual review: verify sidebar density is tighter (smaller icons, less padding)
- [ ] [5.6] [user] Visual review: verify Approvals tab appears on Obligations page and renders the approval queue
- [ ] [5.7] [user] Visual review: verify /approvals redirects to /obligations?tab=approvals
- [ ] [5.8] [user] Visual review: verify Usage and Settings are removed from sidebar nav
- [ ] [5.9] [user] Visual review: verify settings gear icon appears in sidebar footer
- [ ] [5.10] [user] Visual review: verify empty states show compact inline text instead of full-page centered layout
