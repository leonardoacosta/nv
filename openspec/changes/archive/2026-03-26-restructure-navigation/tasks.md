# Implementation Tasks

<!-- beads:epic:nv-jimr -->

## UI Batch

- [ ] [1.1] [P-1] Update `Sidebar.tsx` nav configuration: remove Cold Starts entry, remove CC Session entry, move Memory from SYSTEM group to DATA group (between Projects and Integrations); verify item count drops from 15 to 11 [owner:ui-engineer] [beads:nv-6a85]
- [ ] [1.2] [P-1] Extract Cold Starts page content into a reusable `ColdStartsPanel` component at `apps/dashboard/components/ColdStartsPanel.tsx` — lift data fetching and UI from `apps/dashboard/app/cold-starts/page.tsx` [owner:ui-engineer] [beads:nv-r61t]
- [ ] [1.3] [P-1] Add tab navigation to Usage page (`apps/dashboard/app/usage/page.tsx`): "Cost" tab (existing content) and "Performance" tab (renders `ColdStartsPanel`); read `?tab` query param to set active tab, default to "cost" [owner:ui-engineer] [beads:nv-04b1]
- [ ] [1.4] [P-1] Extract CC Session page content into a reusable `CCSessionPanel` component at `apps/dashboard/components/CCSessionPanel.tsx` — lift data fetching and UI from `apps/dashboard/app/session/page.tsx` [owner:ui-engineer] [beads:nv-y5nk]
- [ ] [1.5] [P-1] Integrate `CCSessionPanel` into Sessions page (`apps/dashboard/app/sessions/page.tsx`): render as a card/panel at the top of the sessions list; show/hide via `?panel=cc` query param [owner:ui-engineer] [beads:nv-oulc]
- [ ] [1.6] [P-1] Add permanent redirects in `next.config.ts`: `/cold-starts` to `/usage?tab=performance` (301), `/session` to `/sessions?panel=cc` (301) [owner:ui-engineer] [beads:nv-zs6d]
- [ ] [1.7] [P-2] Remove `apps/dashboard/app/cold-starts/` directory after confirming all content is extracted to `ColdStartsPanel` [owner:ui-engineer] [beads:nv-bi9m]
- [ ] [1.8] [P-2] Remove `apps/dashboard/app/session/` directory after confirming all content is extracted to `CCSessionPanel` [owner:ui-engineer] [beads:nv-c8in]
