# Implementation Tasks

<!-- beads:epic:nv-n3iq -->

## Foundation

- [ ] [1.1] [P-1] Extend `apps/dashboard/app/globals.css` with Geist CSS custom properties (--ds-background-100/200, --ds-gray-100 through 1000, --ds-gray-alpha-100/200/400, --ds-purple-700/900, --ds-green-700, --ds-amber-700, --ds-red-700, --ds-blue-700) mapped to cosmic color values per Req-1 [owner:ui-engineer]
- [ ] [1.2] [P-1] Add typography utility classes to `globals.css`: `.text-heading-32`, `.text-heading-24`, `.text-heading-20`, `.text-heading-16`, `.text-label-16`, `.text-label-14`, `.text-label-13`, `.text-label-12`, `.text-copy-16`, `.text-copy-14`, `.text-copy-13`, `.text-label-13-mono`, `.text-button-14` — each with font-size, line-height, letter-spacing, font-weight per Req-2 type scale table [owner:ui-engineer]
- [ ] [1.3] [P-1] Add material surface utility classes to `globals.css`: `.surface-base`, `.surface-card`, `.surface-raised`, `.surface-inset` using `@apply` with background, border, radius, shadow per Req-3 table [owner:ui-engineer]
- [ ] [1.4] [P-1] Add animation keyframes and utilities to `globals.css`: `@keyframes fade-in-up`, `@keyframes shimmer`, `.animate-fade-in-up`, `.animate-stagger-1` through `.animate-stagger-10`, `.animate-shimmer` per Req-8 [owner:ui-engineer]
- [ ] [1.5] [P-2] Extend `tailwind.config.ts` with animation timing utilities (`animation: { 'fade-in-up': 'fade-in-up 200ms ease forwards', 'shimmer': 'shimmer 1.5s infinite' }`) and any missing color tokens [owner:ui-engineer]

## Shared Components

- [ ] [2.1] [P-1] Redesign `apps/dashboard/components/layout/StatCard.tsx` — add props: `icon` (LucideIcon), `accentColor` (string, default cosmic-purple), `trend` (optional { value: number, direction: 'up'|'down' }); render: 4px left accent bar, 24px icon muted, value in `.text-heading-32 tabular-nums`, label in `.text-label-13`, trend arrow + percentage; `.surface-card` material; hover: border-gray-500 + translateY(-1px); transition 150ms [owner:ui-engineer]
- [ ] [2.2] [P-1] Redesign `apps/dashboard/components/layout/EmptyState.tsx` — add props: `icon` (LucideIcon), `title` (string), `description` (string), `action` (optional { label: string, onClick: () => void }); render: centered, icon 48px muted opacity-50, title `.text-heading-16`, description `.text-copy-14 text-cosmic-muted max-w-xs`, action button `.surface-card`; `.animate-fade-in-up` entrance [owner:ui-engineer]
- [ ] [2.3] [P-1] Redesign `apps/dashboard/components/layout/ErrorBanner.tsx` — render: `bg-[var(--ds-red-700)]/10` background, 3px left border `var(--ds-red-700)`, AlertCircle icon 16px, text `.text-copy-14`, optional retry button ghost-styled, optional dismiss X button; `rounded-md` (6px) [owner:ui-engineer]
- [ ] [2.4] [P-1] Redesign `apps/dashboard/components/layout/PageSkeleton.tsx` — add variant prop: `'stat-grid'|'list'|'detail'`; stat-grid: 4-6 skeleton cards matching StatCard shape; list: 5 skeleton rows; detail: header + 3 content blocks; all blocks use `.bg-[var(--ds-gray-alpha-200)] animate-shimmer rounded-md` [owner:ui-engineer]
- [ ] [2.5] [P-1] Redesign `apps/dashboard/components/layout/SectionHeader.tsx` — title in `.text-label-12 uppercase tracking-widest text-cosmic-muted`, optional count badge (numeric in small pill), optional status dot [owner:ui-engineer]

## Sidebar

- [ ] [3.1] [P-1] Update `apps/dashboard/components/Sidebar.tsx` active state — active item: `bg-[var(--ds-gray-alpha-200)]` with `border-l-2 border-cosmic-purple` left accent; hover: `bg-[var(--ds-gray-alpha-100)]` transition 150ms; icons 18px default `text-[var(--ds-gray-900)]`, active `text-cosmic-purple` [owner:ui-engineer]
- [ ] [3.2] [P-2] Add nav section dividers to Sidebar — group items: "Overview" (Dashboard, Briefing), "Activity" (Obligations, Approvals, Diary, Sessions, Messages), "Data" (Projects, Contacts, Integrations), "System" (Usage, Cold Starts, Memory, CC Session, Settings); thin `border-b border-[var(--ds-gray-alpha-200)]` between groups with `.text-label-12 uppercase` group label [owner:ui-engineer]
- [ ] [3.3] [P-2] Add WebSocket status label next to dot in Sidebar footer — `.text-label-12` showing "Connected" / "Reconnecting" / "Offline" next to the colored dot [owner:ui-engineer]

## Page Updates — Apply Design System

- [ ] [4.1] [P-2] Update `apps/dashboard/app/page.tsx` (Home) — page title `.text-heading-24`, section headers `.text-label-12 uppercase`, stat cards use redesigned StatCard with icons (Activity for obligations, Layers for sessions, FolderOpen for projects, Heart for health), `.animate-fade-in-up` on page content, `.animate-stagger-*` on stat grid [owner:ui-engineer]
- [ ] [4.2] [P-2] Update `apps/dashboard/app/briefing/page.tsx` — `.surface-card` for briefing sections, empty state with Sun icon per Req-5 table, error banner uses redesigned ErrorBanner [owner:ui-engineer]
- [ ] [4.3] [P-2] Update `apps/dashboard/app/obligations/page.tsx` — `.surface-card` for obligation items, status tabs styled with active/hover states per Req-9, list items `.animate-stagger-*` [owner:ui-engineer]
- [ ] [4.4] [P-2] Update `apps/dashboard/app/approvals/page.tsx` — empty state with ShieldCheck icon, `.surface-card` for queue items, split panel uses `.surface-base` for list and `.surface-raised` for detail [owner:ui-engineer]
- [ ] [4.5] [P-2] Update `apps/dashboard/app/diary/page.tsx` — diary entries in `.surface-card`, date nav buttons with hover states, summary bar stat cards with StatCard component [owner:ui-engineer]
- [ ] [4.6] [P-2] Update `apps/dashboard/app/sessions/page.tsx` — session cards in `.surface-card`, status dots use `--ds-green/amber/red-700`, filter tabs with active state, empty state with Layers icon [owner:ui-engineer]
- [ ] [4.7] [P-2] Update `apps/dashboard/app/messages/page.tsx` — message rows in `.surface-base` with hover `.bg-[var(--ds-gray-200)]`, channel filter chips styled as pills, pagination buttons with Req-9 states [owner:ui-engineer]
- [ ] [4.8] [P-2] Update `apps/dashboard/app/projects/page.tsx` — project cards in `.surface-card`, empty state with FolderOpen icon, search input uses `.surface-inset` [owner:ui-engineer]
- [ ] [4.9] [P-2] Update `apps/dashboard/app/nexus/page.tsx` — health cards in `.surface-card` with accent bars (green=healthy, amber=degraded, red=critical), session list with `.animate-stagger-*` [owner:ui-engineer]
- [ ] [4.10] [P-2] Update `apps/dashboard/app/integrations/page.tsx` — integration cards in `.surface-card`, section headers `.text-label-12 uppercase`, empty cards with placeholder styling [owner:ui-engineer]
- [ ] [4.11] [P-2] Update `apps/dashboard/app/usage/page.tsx` — cost summary cards as StatCard with DollarSign icon, token/tool sections with `.surface-card`, `.text-heading-32 tabular-nums` for values [owner:ui-engineer]
- [ ] [4.12] [P-2] Update `apps/dashboard/app/cold-starts/page.tsx` — percentile cards as StatCard with Timer icon, chart area in `.surface-inset`, stats row with `.surface-card` [owner:ui-engineer]
- [ ] [4.13] [P-2] Update `apps/dashboard/app/memory/page.tsx` — memory items in `.surface-card`, search uses `.surface-inset`, empty state with Brain icon [owner:ui-engineer]
- [ ] [4.14] [P-2] Update `apps/dashboard/app/session/page.tsx` — status card in `.surface-card` with accent bar matching state color, log viewer in `.surface-inset font-mono`, buttons with Req-9 states [owner:ui-engineer]
- [ ] [4.15] [P-2] Update `apps/dashboard/app/settings/page.tsx` — section cards in `.surface-card` with `.text-label-16` section titles, form inputs in `.surface-inset`, masked fields styled distinctly [owner:ui-engineer]

## Verify

- [ ] [5.1] `cd apps/dashboard && npx next build` passes with zero errors [owner:ui-engineer]
- [ ] [5.2] [user] Visual review: navigate all 15 pages, verify consistent typography hierarchy, material surfaces, and transitions
- [ ] [5.3] [user] Mobile check: verify sidebar collapses, stat grids reflow, touch targets >= 44px at 375px viewport
