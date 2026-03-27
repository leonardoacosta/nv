# Proposal: Global Density Pass

## Change ID
`global-density-pass`

## Summary

Apply Geist design density across all dashboard pages. Remove max-width constraints, replace card-based layouts with border separators, tighten spacing globally, increase information density per pixel, simplify empty states, and consolidate low-value routes.

## Context
- Extends: `apps/dashboard/components/layout/PageShell.tsx`, `apps/dashboard/components/layout/EmptyState.tsx`, `apps/dashboard/components/layout/StatCard.tsx`, `apps/dashboard/components/Sidebar.tsx`
- Affects: every page under `apps/dashboard/app/` that uses PageShell, StatCard, or EmptyState
- Related: `polish-dashboard`, `improve-obligation-ux` (prior layout work)

## Motivation

The current dashboard uses generous spacing, rounded card containers, and centered max-width content areas that waste horizontal and vertical space. On a wide monitor, the `max-w-6xl mx-auto` constraint in PageShell leaves large empty gutters. Card-based layouts (`surface-card`) add padding, borders, and rounded corners that consume pixels without adding information. Empty states use full-height centered layouts with large icons that dominate the viewport when there is no data.

1. **Wasted horizontal space** -- PageShell wraps content in `max-w-6xl mx-auto`, leaving unused gutters on wide displays. Content should fill from sidebar edge to right edge.
2. **Card overhead** -- `surface-card` containers add 12px border-radius, 1px border, box-shadow, padding, and hover transforms. For dense data (stat rows, list items, contact rows), a simple `border-b` separator is more space-efficient.
3. **Excessive vertical spacing** -- `space-y-6` between sections, `py-5` page headers, `py-6` content areas, and `text-heading-24` page titles all add vertical overhead before useful content appears.
4. **Oversized empty states** -- Full-page centered EmptyState with 40px icon and `py-16` padding wastes the viewport when a single line of muted text would suffice.
5. **Sidebar density** -- Nav items use `py-2 min-h-9`, icons at 18px, and group labels at 12px with generous padding. These can be tightened for a denser navigation rail.
6. **Route sprawl** -- Approvals is a separate page but conceptually belongs as a tab on Obligations. Usage has no telemetry data yet. Settings occupies a full page when a modal or dropdown would suffice.

## Requirements

### Req-1: Remove Max-Width Constraint

Remove `max-w-6xl mx-auto` from PageShell's content wrapper. Content fills edge-to-edge within the main area (sidebar to right viewport edge). Keep horizontal padding (`px-6`) for breathing room.

### Req-2: Replace Card Layouts with Border Layouts

Replace `surface-card` containers used for stat groups, list items, and sidebar panels with `border-b border-ds-gray-400` separators. Specifically:
- Dashboard stat cards: render as a horizontal row with border separators instead of individual rounded cards
- Dashboard sidebar panels (obligations, session breakdown, messages): use `border-b` sections instead of `surface-card`
- Diary stat cards: inline stat row with border separators
- Obligation cards: keep the existing card layout (already dense with priority bar) but remove hover transform
- Session rows, contact rows: use `border-b` row layout instead of card containers

### Req-3: Tighten Spacing Globally

- PageShell header: reduce `py-5` to `py-3`, reduce `text-heading-24` to `text-heading-20`
- PageShell content area: reduce `py-6` to `py-4`
- Inter-section spacing: replace `space-y-6` with `space-y-3` across all pages
- List item padding: reduce `py-4` to `py-2.5` on list rows
- Section header gaps: reduce `mb-6` / `mb-4` to `mb-2`

### Req-4: Typography Density

- Data values (timestamps, counts, IDs): use `text-label-13-mono` (Geist Mono 13px)
- Section labels: use `text-label-12` (already 12px uppercase tracking-wider)
- Body content: use `text-copy-13` instead of `text-copy-14` where possible
- Page subtitles: use `text-copy-13` instead of `text-copy-14`

### Req-5: Compact Empty States

Replace full-page centered EmptyState usages with inline text:
- Current: `<EmptyState icon={...} title="No X yet" description="..." />`
- Target: `<p className="text-copy-13 text-ds-gray-900 py-3">No items</p>`
- Keep the EmptyState component for cases where a CTA button is needed, but use it inline (remove `py-16`, reduce icon to 20px)

### Req-6: Sidebar Density

- Nav item padding: reduce `py-2 min-h-9` to `py-1.5 min-h-8`
- Nav icons: reduce from `size={18}` to `size={16}`
- Group label padding: reduce `pt-2 pb-1` to `pt-1.5 pb-0.5`
- Logo section: reduce `py-4` to `py-3`
- WS status footer: reduce `py-2.5 min-h-11` to `py-2 min-h-9`

### Req-7: Merge Approvals into Obligations

- Add an "Approvals" tab to the obligations page (alongside existing "Active" and "History" tabs)
- The Approvals tab renders the approval queue list + detail panel (existing components from `apps/dashboard/app/approvals/`)
- Remove `/approvals` from the sidebar NAV_GROUPS
- Keep the `/approvals` route as a redirect to `/obligations?tab=approvals` for backward compatibility

### Req-8: Hide Low-Value Pages

- Usage: remove from sidebar NAV_GROUPS (keep the route and page for direct URL access)
- Settings: remove from sidebar NAV_GROUPS, add a settings icon button to the sidebar footer (next to logout), which opens a modal/drawer with settings content

## Scope
- **IN**: CSS/layout changes across all pages, sidebar density, merge approvals tab, hide usage/settings from sidebar
- **OUT**: Data model changes, API changes, new features, new data fetching logic

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/components/layout/PageShell.tsx` | Remove max-width, tighten header/content spacing |
| `apps/dashboard/components/layout/EmptyState.tsx` | Add compact variant, reduce default padding |
| `apps/dashboard/components/layout/StatCard.tsx` | Add inline/border variant (no card container) |
| `apps/dashboard/components/Sidebar.tsx` | Tighten spacing, remove Approvals/Usage/Settings from nav, add settings icon to footer |
| `apps/dashboard/app/page.tsx` | Replace card layouts with border layouts, tighten spacing |
| `apps/dashboard/app/obligations/page.tsx` | Add Approvals tab, tighten spacing |
| `apps/dashboard/app/approvals/page.tsx` | Redirect to `/obligations?tab=approvals` |
| `apps/dashboard/app/diary/page.tsx` | Inline stats, tighten spacing |
| `apps/dashboard/app/sessions/page.tsx` | Border rows, tighten spacing |
| `apps/dashboard/app/contacts/page.tsx` | Border rows, tighten spacing |
| `apps/dashboard/app/usage/page.tsx` | Tighten spacing (page kept, route hidden) |
| `apps/dashboard/app/settings/page.tsx` | Tighten spacing, potentially extract to modal component |
| `apps/dashboard/app/messages/page.tsx` | Tighten spacing |
| `apps/dashboard/app/briefing/page.tsx` | Tighten spacing |
| `apps/dashboard/app/memory/page.tsx` | Tighten spacing |
| `apps/dashboard/app/integrations/page.tsx` | Tighten spacing |
| `apps/dashboard/app/projects/page.tsx` | Tighten spacing |

## Risks
| Risk | Mitigation |
|------|-----------|
| Removing max-width may cause readability issues on ultrawide displays | Keep `px-6` padding; grid layouts naturally constrain line lengths |
| Merging approvals into obligations increases page complexity | Approvals tab reuses existing components wholesale; lazy-load if needed |
| Hiding settings from sidebar may confuse users | Settings icon in sidebar footer is a common pattern (VS Code, Discord); tooltip on hover |
| Compact empty states may feel too sparse | Keep muted text styling consistent; single line is sufficient for "no data" messaging |
