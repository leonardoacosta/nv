# Implementation Tasks

## UI Batch 1 -- Shared Utilities + Layout Components

- [x] [1.1] [P-1] Create `apps/dashboard/lib/brand-colors.ts` -- export a `PLATFORM_BRAND` const map with keys `telegram`, `discord`, `slack`, `cli`, `api`; each value has `{ bg, text, border, dot }` Tailwind class strings using the platform's official hex colors; export a `getPlatformColor(channel: string)` helper that returns the brand entry or a neutral ds-gray fallback [owner:ui-engineer]
- [x] [1.2] [P-1] Update `apps/dashboard/components/layout/EmptyState.tsx` -- verify it supports the compact pattern: single line text-copy-13 text-ds-gray-900 + optional action button using text-button-14 + optional help link; max py-4; no icons above 20px; if the component already supports this from global-density-pass, no changes needed [owner:ui-engineer]
- [x] [1.3] [P-2] Update `apps/dashboard/components/layout/SectionHeader.tsx` -- replace any raw text-sm or text-xs classes with text-label-12 or text-label-14 from the type scale [owner:ui-engineer]
- [x] [1.4] [P-2] Update `apps/dashboard/components/layout/ErrorBoundary.tsx` -- tighten spacing (reduce py-6/py-8 to py-3/py-4); replace any raw text sizing with type scale classes [owner:ui-engineer]
- [x] [1.5] [P-2] Update `apps/dashboard/components/layout/PageSkeleton.tsx` -- tighten spacing (reduce py-6/gap-6 to py-3/gap-3) [owner:ui-engineer]
- [x] [1.6] [P-3] Update `apps/dashboard/lib/markdown.tsx` -- replace raw text-sm/text-xs with type scale equivalents (text-copy-13, text-copy-14) [owner:ui-engineer]

## UI Batch 2 -- Status Color Standardization (Components)

- [x] [2.1] [P-1] Update `apps/dashboard/components/ObligationItem.tsx` -- replace all bg-[#EF4444], bg-[#F97316], bg-[#6B7280], bg-[#374151] and their /10 /20 /30 variants with ds-* status tokens (bg-red-700, bg-amber-700, bg-ds-gray-600, bg-ds-gray-500); replace border-[#hex] with border-red-700/30 or border-ds-gray-alpha-400; replace raw text sizes with type scale [owner:ui-engineer]
- [x] [2.2] [P-1] Update `apps/dashboard/components/approvals/ApprovalQueueItem.tsx` -- replace bg-[#EF4444], bg-[#F97316] status dots with bg-red-700, bg-amber-700; replace raw text sizes with type scale [owner:ui-engineer]
- [x] [2.3] [P-1] Update `apps/dashboard/components/ServerHealth.tsx` -- replace bg-[#EF4444], bg-[#F97316] and their /20 text variants with bg-red-700, bg-amber-700, text-red-700, text-amber-700; replace raw text-sm/text-xs with type scale [owner:ui-engineer]
- [x] [2.4] [P-1] Update `apps/dashboard/components/ProjectAccordion.tsx` -- replace bg-[#EF4444], bg-[#F97316] and text-[#hex] variants with ds-* status tokens; replace raw text sizes with type scale [owner:ui-engineer]
- [x] [2.5] [P-2] Update `apps/dashboard/components/ServiceRow.tsx` -- replace bg-green-700 (already correct as Tailwind maps to ds-green), verify bg-red-700 matches; replace raw text sizes with type scale [owner:ui-engineer]
- [x] [2.6] [P-2] Update `apps/dashboard/components/Sidebar.tsx` -- replace bg-green-700, bg-amber-700, bg-red-700 WS status dots (verify these already map to ds-* via tailwind.config); replace bg-amber-700/20 text-amber-700 update badge colors; replace raw text-sm with type scale [owner:ui-engineer]

## UI Batch 3 -- Platform Brand Color Consolidation (Components)

- [x] [3.1] [P-1] Update `apps/dashboard/components/SessionCard.tsx` -- remove local CHANNEL_COLOR map; import from brand-colors.ts; replace raw text sizes with type scale [owner:ui-engineer]
- [x] [3.2] [P-1] Update `apps/dashboard/components/ActiveSession.tsx` -- remove local CHANNEL_COLOR map; import from brand-colors.ts; replace raw text sizes with type scale [owner:ui-engineer]
- [x] [3.3] [P-2] Update `apps/dashboard/components/DiaryEntry.tsx` -- replace hardcoded bg-[#229ED9]/20 text-[#229ED9] border-[#229ED9]/30 with brand-colors.ts import for telegram; replace raw text sizes with type scale [owner:ui-engineer]
- [x] [3.4] [P-2] Update `apps/dashboard/components/ContactCard.tsx` -- replace raw text sizes with type scale; standardize contact type colors (bg-red-700/20 text-red-700, bg-amber-700/20 text-amber-700) [owner:ui-engineer]
- [x] [3.5] [P-2] Update `apps/dashboard/components/ContactDetailPanel.tsx` -- replace raw text sizes with type scale; standardize contact type colors; tighten spacing (py-6 to py-3) [owner:ui-engineer]

## UI Batch 4 -- Typography + Spacing (Shared Components)

- [x] [4.1] [P-2] Update `apps/dashboard/components/NovaBadge.tsx` -- replace raw text-xs with text-label-12 [owner:ui-engineer]
- [x] [4.2] [P-2] Update `apps/dashboard/components/LeoBadge.tsx` -- replace raw text-xs with text-label-12 [owner:ui-engineer]
- [x] [4.3] [P-2] Update `apps/dashboard/components/SessionWidget.tsx` -- replace raw text-sm/text-xs with type scale (text-copy-13, text-label-12) [owner:ui-engineer]
- [x] [4.4] [P-2] Update `apps/dashboard/components/SessionDashboard.tsx` -- replace raw text sizes with type scale; tighten spacing (space-y-6/gap-6 to space-y-3/gap-3) [owner:ui-engineer]
- [x] [4.5] [P-2] Update `apps/dashboard/components/LatencyChart.tsx` -- replace raw text-sm/text-xs with type scale [owner:ui-engineer]
- [x] [4.6] [P-2] Update `apps/dashboard/components/MemoryPreview.tsx` -- replace raw text-sm/text-xs with type scale [owner:ui-engineer]
- [x] [4.7] [P-2] Update `apps/dashboard/components/ObligationSummaryBar.tsx` -- replace raw text-sm with type scale [owner:ui-engineer]
- [x] [4.8] [P-3] Update `apps/dashboard/components/ColdStartsPanel.tsx` -- tighten spacing; replace raw text sizes if any [owner:ui-engineer]
- [x] [4.9] [P-3] Update `apps/dashboard/components/ActivityFeed.tsx` -- tighten spacing; replace raw text sizes if any [owner:ui-engineer]

## UI Batch 5 -- Page-Level Token Standardization (Core Pages)

- [ ] [5.1] [P-1] Update `apps/dashboard/app/page.tsx` -- replace text-amber-500, text-amber-200, text-blue-400, text-blue-300, bg-amber-500, bg-red-500 with ds-* status tokens; replace raw text-sm/text-xs with type scale; tighten any remaining generous spacing [owner:ui-engineer]
- [ ] [5.2] [P-1] Update `apps/dashboard/app/obligations/page.tsx` -- replace bg-[#EF4444], bg-[#F97316], bg-[#6B7280], bg-[#374151] priority dot map with ds-* status tokens; replace text-[#F97316] with text-amber-700; replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [5.3] [P-1] Update `apps/dashboard/app/chat/page.tsx` -- replace bg-green-500, bg-[#229ED9], bg-blue-900/20, text-blue-400 with ds-* tokens or brand-colors import; replace raw text sizes with type scale; tighten spacing [owner:ui-engineer]
- [ ] [5.4] [P-2] Update `apps/dashboard/app/sessions/page.tsx` -- replace raw text-sm/text-xs (38 occurrences) with type scale classes [owner:ui-engineer]
- [ ] [5.5] [P-2] Update `apps/dashboard/app/sessions/[id]/page.tsx` -- remove local CHANNEL_COLOR map; import from brand-colors.ts; replace raw text sizes with type scale [owner:ui-engineer]

## UI Batch 6 -- Page-Level Token Standardization (Secondary Pages)

- [ ] [6.1] [P-2] Update `apps/dashboard/app/contacts/page.tsx` -- replace raw text sizes with type scale; tighten spacing [owner:ui-engineer]
- [ ] [6.2] [P-2] Update `apps/dashboard/app/messages/page.tsx` -- replace raw text sizes (19 occurrences) with type scale; replace any remaining hex colors [owner:ui-engineer]
- [ ] [6.3] [P-2] Update `apps/dashboard/app/diary/page.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [6.4] [P-2] Update `apps/dashboard/app/automations/page.tsx` -- replace bg-amber-700/20, bg-green-700/20, text-red-700, text-green-700 (verify these already use correct ds-* mapped values via tailwind.config); replace raw text sizes with type scale; tighten spacing [owner:ui-engineer]
- [ ] [6.5] [P-3] Update `apps/dashboard/app/briefing/page.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [6.6] [P-3] Update `apps/dashboard/app/usage/page.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [6.7] [P-3] Update `apps/dashboard/app/nexus/page.tsx` -- tighten spacing; replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [6.8] [P-3] Update `apps/dashboard/app/integrations/page.tsx` -- tighten spacing [owner:ui-engineer]
- [ ] [6.9] [P-3] Update `apps/dashboard/app/memory/page.tsx` -- replace raw text sizes if any [owner:ui-engineer]
- [ ] [6.10] [P-3] Update `apps/dashboard/app/login/page.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [6.11] [P-3] Update `apps/dashboard/app/projects/page.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]

## UI Batch 7 -- Settings + Approvals Subcomponents

- [ ] [7.1] [P-3] Update `apps/dashboard/app/settings/page.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [7.2] [P-3] Update `apps/dashboard/app/settings/components/SaveRestartBar.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [7.3] [P-3] Update `apps/dashboard/app/settings/components/SettingsSection.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [7.4] [P-3] Update `apps/dashboard/app/approvals/components/ApprovalQueueItem.tsx` -- replace hex status colors with ds-* tokens (mirror changes from 2.2 for the duplicate component) [owner:ui-engineer]
- [ ] [7.5] [P-3] Update `apps/dashboard/app/approvals/components/BatchActionBar.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]
- [ ] [7.6] [P-3] Update `apps/dashboard/app/approvals/components/QueueClearCelebration.tsx` -- replace raw text sizes with type scale [owner:ui-engineer]

## Verify

- [ ] [8.1] `cd apps/dashboard && pnpm typecheck` passes -- zero TypeScript errors [owner:ui-engineer]
- [ ] [8.2] `cd apps/dashboard && pnpm build` passes -- no build errors [owner:ui-engineer]
- [ ] [8.3] Grep audit: `grep -rn 'text-sm \|text-xs \|text-lg \|text-xl \|text-2xl \|text-3xl \|text-base ' apps/dashboard/ --include='*.tsx'` returns zero results (all raw text sizes replaced) [owner:ui-engineer]
- [ ] [8.4] Grep audit: `grep -rn 'bg-\[#\|text-\[#\|border-\[#' apps/dashboard/ --include='*.tsx'` returns only brand-colors.ts and files importing platform brand colors (no ad-hoc hex in component classNames) [owner:ui-engineer]
- [ ] [8.5] Grep audit: `grep -rn 'bg-zinc-\|text-gray-\|border-gray-\|bg-gray-\|text-zinc-\|border-zinc-' apps/dashboard/ --include='*.tsx'` returns zero results (all raw Tailwind grays replaced with ds-* tokens) [owner:ui-engineer]
- [ ] [8.6] [user] Visual review: verify font rendering -- Geist Sans Variable for body, Geist Mono Variable for mono elements (timestamps, IDs)
- [ ] [8.7] [user] Visual review: verify status colors are consistent across obligations, approvals, server health, projects, sidebar
- [ ] [8.8] [user] Visual review: verify spacing is data-dense and compact across all pages
- [ ] [8.9] [user] Visual review: verify empty states show single-line text + action button pattern
- [ ] [8.10] [user] Visual review: compare dashboard density against Vercel dashboard screenshots
