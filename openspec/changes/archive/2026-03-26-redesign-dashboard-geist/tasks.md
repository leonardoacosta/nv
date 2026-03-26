# Implementation Tasks

<!-- beads:epic:nv-n3iq -->

## Foundation — Strip Cosmic, Install Geist

- [x] [1.1] [P-1] Rewrite `apps/dashboard/app/globals.css` — remove ALL cosmic CSS vars and classes; add Geist `--ds-*` dark mode tokens (background-100: #0a0a0a, background-200: #111111, gray-100 through gray-1000, alpha variants, blue/green/amber/red status colors); add Geist type scale utility classes (.text-heading-32/24/20/16, .text-label-16/14/13/12, .text-copy-16/14/13, .text-label-13-mono, .text-button-14); add material surface classes (.surface-base/card/raised/inset); add animation keyframes (fade-in-up, shimmer) and stagger utilities; set body bg to ds-bg-100, text to ds-gray-1000 [owner:ui-engineer]
- [x] [1.2] [P-1] Rewrite `apps/dashboard/tailwind.config.ts` — remove cosmic color object, cosmic-gradient backgroundImage, cosmic shadows, cosmic borderRadius; add ds.bg, ds.gray (100-1000), blue, green, amber, red color tokens; add animation timing utilities [owner:ui-engineer]
- [x] [1.3] [P-2] Global search-replace across ALL files in `apps/dashboard/` — replace every cosmic class with Geist equivalent per Req-11 table: bg-cosmic-dark→bg-ds-bg-100, bg-cosmic-surface→bg-ds-gray-100, text-cosmic-text→text-ds-gray-1000, text-cosmic-bright→text-ds-gray-1000, text-cosmic-muted→text-ds-gray-900, border-cosmic-border→border-ds-gray-400, bg-cosmic-gradient→bg-ds-bg-100, shadow-cosmic-sm→shadow-sm, rounded-cosmic→rounded-xl, etc. Run `grep -r "cosmic" apps/dashboard/` after to catch any remaining [owner:ui-engineer]

## Shared Components — Geist Patterns

- [x] [2.1] [P-1] Redesign `apps/dashboard/components/layout/StatCard.tsx` — surface-card material; 4px left accent bar (default ds-gray-600, green/amber/red for status); icon 20px text-ds-gray-700; value .text-heading-32 tabular-nums text-ds-gray-1000; label .text-label-13 text-ds-gray-900; optional trend arrow; hover: border-ds-gray-500 translateY(-1px) shadow upgrade; transition 150ms [owner:ui-engineer]
- [x] [2.2] [P-1] Redesign `apps/dashboard/components/layout/EmptyState.tsx` — centered; icon 40px text-ds-gray-600; title .text-heading-16 text-ds-gray-1000; description .text-copy-14 text-ds-gray-900 max-w-xs; optional action button surface-base; fade-in 200ms [owner:ui-engineer]
- [x] [2.3] [P-1] Redesign `apps/dashboard/components/layout/ErrorBanner.tsx` — bg red-700/8%; left border 3px ds-red-700; AlertCircle 16px ds-red-700; text .text-copy-14 ds-red-900; ghost retry button; rounded-md (6px) [owner:ui-engineer]
- [x] [2.4] [P-1] Redesign `apps/dashboard/components/layout/PageSkeleton.tsx` — variant prop (stat-grid|list|detail); skeleton blocks bg-ds-gray-alpha-200 with shimmer animation; shapes match target content [owner:ui-engineer]
- [x] [2.5] [P-1] Redesign `apps/dashboard/components/layout/SectionHeader.tsx` — .text-label-12 uppercase tracking-widest text-ds-gray-700; optional count badge (pill bg-ds-gray-alpha-200); optional status dot [owner:ui-engineer]

## Sidebar — Geist Navigation

- [x] [3.1] [P-1] Rewrite `apps/dashboard/components/Sidebar.tsx` styling — bg-ds-bg-200; logo "Nova" in .text-label-16 font-semibold; nav items .text-label-14 text-ds-gray-900 with 18px icons; active state: bg-ds-gray-alpha-200 text-ds-gray-1000 border-l-2 border-ds-gray-1000 (white accent, NOT purple); hover: bg-ds-gray-alpha-100; transitions 150ms [owner:ui-engineer]
- [x] [3.2] [P-2] Add nav section groups — "Overview" (Dashboard, Briefing), "Activity" (Obligations, Approvals, Diary, Sessions, Messages), "Data" (Projects, Contacts, Integrations), "System" (Usage, Cold Starts, Memory, CC Session, Settings); group labels .text-label-12 uppercase text-ds-gray-700; thin border-b dividers between groups [owner:ui-engineer]
- [x] [3.3] [P-2] WebSocket status in sidebar footer — .text-label-12 text-ds-gray-700; dot color green-700/amber-700/red-700; label "Connected"/"Reconnecting"/"Offline" [owner:ui-engineer]

## Page Updates — Apply Geist Everywhere

- [x] [4.1] [P-2] Update `apps/dashboard/app/page.tsx` (Home) — .text-heading-24 page title; .text-label-12 uppercase section headers; StatCard with icons; staggered fade-in on stat grid; surface-card for list sections [owner:ui-engineer]
- [x] [4.2] [P-2] Update `apps/dashboard/app/briefing/page.tsx` — surface-card for sections; EmptyState with Sun icon; ErrorBanner for API failures [owner:ui-engineer]
- [x] [4.3] [P-2] Update `apps/dashboard/app/obligations/page.tsx` — surface-card items; tab active/hover states per Geist interactive pattern; staggered list items [owner:ui-engineer]
- [x] [4.4] [P-2] Update `apps/dashboard/app/approvals/page.tsx` — EmptyState with ShieldCheck icon; surface-card queue items; surface-raised for detail panel [owner:ui-engineer]
- [x] [4.5] [P-2] Update `apps/dashboard/app/diary/page.tsx` — surface-card entries; .text-label-13-mono for timestamps; summary bar with StatCard [owner:ui-engineer]
- [x] [4.6] [P-2] Update `apps/dashboard/app/sessions/page.tsx` — surface-card session cards; status dots ds-green/amber/red-700; filter tabs Geist interactive; EmptyState with Layers icon [owner:ui-engineer]
- [x] [4.7] [P-2] Update `apps/dashboard/app/messages/page.tsx` — surface-base rows with hover bg-ds-gray-200; filter chips as pills; pagination with interactive states [owner:ui-engineer]
- [x] [4.8] [P-2] Update `apps/dashboard/app/projects/page.tsx` — surface-card cards; EmptyState with FolderOpen; search input surface-inset [owner:ui-engineer]
- [x] [4.9] [P-2] Update `apps/dashboard/app/nexus/page.tsx` — surface-card health tiles with accent bars (green/amber/red per status) [owner:ui-engineer]
- [x] [4.10] [P-2] Update `apps/dashboard/app/integrations/page.tsx` — surface-card integration cards; .text-label-12 section headers [owner:ui-engineer]
- [x] [4.11] [P-2] Update `apps/dashboard/app/usage/page.tsx` — StatCard for cost/token values with DollarSign icon; .text-heading-32 tabular-nums [owner:ui-engineer]
- [x] [4.12] [P-2] Update `apps/dashboard/app/cold-starts/page.tsx` — StatCard for percentiles with Timer icon; surface-inset for chart area [owner:ui-engineer]
- [x] [4.13] [P-2] Update `apps/dashboard/app/memory/page.tsx` — surface-card items; EmptyState with Brain icon; search surface-inset [owner:ui-engineer]
- [x] [4.14] [P-2] Update `apps/dashboard/app/session/page.tsx` — surface-card status card with accent bar; surface-inset font-mono log viewer; Geist button states [owner:ui-engineer]
- [x] [4.15] [P-2] Update `apps/dashboard/app/settings/page.tsx` — surface-card sections with .text-label-16 titles; surface-inset inputs; masked field styling [owner:ui-engineer]

## Verify

- [x] [5.1] `grep -r "cosmic" apps/dashboard/` returns zero matches (complete removal) [owner:ui-engineer]
- [x] [5.2] `cd apps/dashboard && npx next build` passes with zero errors [owner:ui-engineer]
- [ ] [5.3] [user] Visual review: all pages use neutral gray Geist palette, no purple anywhere
- [ ] [5.4] [user] Mobile check: sidebar collapses, grids reflow, touch targets >= 44px at 375px
