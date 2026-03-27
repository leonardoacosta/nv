# Proposal: Geist Token Standardization

## Change ID
`geist-token-standardization`

## Summary

Audit and standardize the entire dashboard to use the established Geist design token system consistently. Replace all raw Tailwind color classes, ad-hoc hex values, generic text sizing, and raw border colors with the ds-* CSS custom properties, surface-* elevation classes, type scale classes, and status color tokens already defined in globals.css and tailwind.config.ts. Tighten spacing to match Vercel's data-dense layout. Standardize empty states to use a single-line explanation with a primary action button.

## Context
- Extends: `apps/dashboard/app/globals.css` (ds-* CSS custom properties, type scale, surface materials), `apps/dashboard/tailwind.config.ts` (ds namespace in Tailwind)
- Related: `global-density-pass` (density and spacing tightening -- complementary), `polish-dashboard` (dashboard home improvements)
- Affects: all pages under `apps/dashboard/app/` and all components under `apps/dashboard/components/`

## Motivation

The dashboard has a comprehensive Geist-inspired token system already defined -- ds-* color variables, surface-* elevation materials, a full type scale (text-heading-*, text-label-*, text-copy-*, text-button-*), and status colors (blue/green/amber/red .700/.900). However, this system is inconsistently applied across the codebase:

1. **Raw Tailwind colors instead of ds-* tokens** -- 285 occurrences of raw Tailwind sizing classes (text-sm, text-xs, text-base, text-lg) across 38 files. Components use bg-green-500, text-amber-500, text-blue-400, bg-red-500 instead of the ds-* status tokens. Some files use bg-zinc-900 or text-gray-400 instead of ds-gray equivalents.

2. **Hardcoded hex values instead of ds-* tokens** -- 14 files use arbitrary bg-[#EF4444], text-[#F97316], bg-[#229ED9], bg-[#5865F2] etc. directly in className strings. These should reference ds-* status tokens for common status colors, and be consolidated into a channel brand color map for platform-specific colors (Telegram, Discord, Slack).

3. **Inconsistent typography** -- Many components use ad-hoc text-sm text-gray-500 or text-xs font-medium instead of the defined type scale classes (text-copy-13, text-label-12, text-label-14). The type scale provides consistent letter-spacing, line-height, and weight that raw Tailwind classes miss.

4. **Inconsistent borders** -- Some components use border-ds-gray-alpha-400 (correct), others use raw border colors or border-[#hex] values. The globals.css already sets `* { @apply border-ds-gray-400 }` as the default, but explicit border color overrides bypass the token system.

5. **Spacing too generous** -- Pages still use py-6, py-8, space-y-6, gap-6 patterns that waste vertical space. Vercel's dashboard is data-dense and compact. Vertical padding and margins should be tightened globally.

6. **Empty states lack directional guidance** -- Current EmptyState usage varies: some show text-only with no action, others have large icons. The standard should be: one line of explanation + primary action button + optional help link. No illustrations, no icons larger than 20px.

7. **Status indicators use inconsistent colors** -- Some components use ds-red-700/ds-green-700 (correct), others use raw Tailwind bg-red-500, bg-green-500, text-amber-500 which are different hues than the Geist status palette.

8. **Font loading not verified** -- Geist Sans Variable and Geist Mono Variable are declared in globals.css via @font-face, but no audit has confirmed they render correctly on all pages (especially dynamically loaded content, modals, and error boundaries).

## Requirements

### Req-1: Replace Raw Tailwind Colors with ds-* Tokens

Audit all .tsx files under apps/dashboard/ for raw Tailwind color usage and replace with ds-* equivalents:

| Raw Tailwind | ds-* Token |
|--------------|-----------|
| text-gray-400, text-gray-500 | text-ds-gray-700 or text-ds-gray-900 (depending on intended contrast) |
| text-gray-300 | text-ds-gray-600 |
| bg-zinc-900, bg-gray-900 | bg-ds-bg-100 or bg-ds-gray-100 |
| bg-gray-800 | bg-ds-gray-200 |
| border-gray-700, border-gray-800 | border-ds-gray-400 (default) or border-ds-gray-alpha-400 |

For status colors (not platform brand colors):

| Raw Color | ds-* Token |
|-----------|-----------|
| bg-red-500, bg-red-700, text-red-400, text-red-700 | bg-red-700, text-red-700 (Tailwind config maps to ds-red-700 #e5484d) |
| bg-green-500, bg-green-700, text-green-400, text-green-700 | bg-green-700, text-green-700 (maps to ds-green-700 #0cce6b) |
| bg-amber-500, text-amber-500, text-amber-400, text-amber-700 | bg-amber-700, text-amber-700 (maps to ds-amber-700 #f5a623) |
| bg-blue-500, text-blue-400, text-blue-700 | bg-blue-700, text-blue-700 (maps to ds-blue-700 #0070f3) |

Affected files (14 with hardcoded hex, 38+ with raw Tailwind sizes): ObligationItem, ApprovalQueueItem, ServerHealth, ProjectAccordion, SessionCard, ActiveSession, DiaryEntry, ContactCard, ContactDetailPanel, Sidebar, NovaBadge, LeoBadge, ServiceRow, chat/page, obligations/page, sessions/[id]/page, messages/page, page.tsx (dashboard home).

### Req-2: Consolidate Platform Brand Colors

Create a shared channel/platform brand color map at `apps/dashboard/lib/brand-colors.ts`:

```typescript
export const PLATFORM_BRAND = {
  telegram: { bg: "bg-[#229ED9]/20", text: "text-[#229ED9]", border: "border-[#229ED9]/30", dot: "bg-[#229ED9]" },
  discord:  { bg: "bg-[#5865F2]/20", text: "text-[#5865F2]", border: "border-[#5865F2]/30", dot: "bg-[#5865F2]" },
  slack:    { bg: "bg-[#4A154B]/20", text: "text-[#E01E5A]", border: "border-[#E01E5A]/30", dot: "bg-[#E01E5A]" },
} as const;
```

Replace the duplicated CHANNEL_COLOR/CHANNEL_ICONS maps in SessionCard, ActiveSession, sessions/[id]/page, chat/page, and DiaryEntry with imports from this shared module. Platform brand colors are the one legitimate use of hardcoded hex -- they are not part of the Geist design system.

### Req-3: Standardize Typography to Type Scale

Replace all raw Tailwind text sizing classes with the Geist type scale:

| Raw Tailwind | Type Scale Class |
|--------------|-----------------|
| text-3xl font-bold, text-2xl font-bold | text-heading-32 |
| text-xl font-semibold | text-heading-20 |
| text-lg font-semibold | text-heading-16 |
| text-sm (body context) | text-copy-13 |
| text-sm font-medium (label context) | text-label-14 or text-label-13 |
| text-xs font-medium uppercase | text-label-12 |
| text-xs (secondary text) | text-copy-13 |
| text-base (body) | text-copy-16 or text-copy-14 |
| text-sm font-medium (button context) | text-button-14 |

Scope: all 38 files currently using raw Tailwind text sizing. Preserve semantic meaning -- a section heading should use text-heading-*, a data label should use text-label-*, body text should use text-copy-*, button text should use text-button-14.

### Req-4: Standardize Borders to ds-gray-alpha Tokens

Replace explicit border color classes with ds-* border tokens:

- Default borders (structural dividers): rely on the global `* { @apply border-ds-gray-400 }` -- remove redundant explicit border-ds-gray-400 classes
- Subtle/alpha borders (cards, insets): use `border-ds-gray-alpha-400` or `border-ds-gray-alpha-200`
- Remove all `border-[#hex]` usages except platform brand colors (handled by Req-2)

Affected files: ObligationItem (border-[#EF4444]/30 etc.), DiaryEntry (border-[#229ED9]/30).

### Req-5: Tighten Vertical Spacing Globally

Reduce vertical padding and margins across all pages to match Vercel's data-dense layout:

| Current | Target | Where |
|---------|--------|-------|
| py-6, py-8 | py-3, py-4 | Page content wrappers, section containers |
| space-y-6, space-y-8 | space-y-3, space-y-4 | Page-level section stacking |
| gap-6, gap-8 | gap-3, gap-4 | Grid and flex containers |
| py-12, py-16 | py-6, py-8 | Hero/empty state areas |
| mb-6, mb-4 | mb-2, mb-3 | Section header margins |

Affected files (18 with generous spacing): contacts/page, ContactDetailPanel, integrations/page, automations/page, page.tsx, obligations/page, chat/page, SessionDashboard, LatencyChart, ColdStartsPanel, ActivityFeed, sessions/[id]/page, nexus/page, ErrorBoundary, PageSkeleton, ProjectAccordion.

Note: `global-density-pass` already addresses spacing in some components (PageShell, Sidebar). This spec covers remaining pages and components not addressed there. Defer to global-density-pass for any overlapping files -- check completed status before modifying.

### Req-6: Standardize Empty States

All empty states must follow a consistent pattern:
- Single line of muted text (text-copy-13 text-ds-gray-900) explaining the empty condition
- Primary action button (if applicable) using text-button-14 style
- Optional help link (text-copy-13 text-ds-gray-700 underline) below the button
- No illustrations, no icons above 20px, no large centered layouts
- Maximum vertical padding: py-4

Audit all EmptyState component usages and inline empty state patterns. Ensure the EmptyState component in `apps/dashboard/components/layout/EmptyState.tsx` supports this pattern (it may already after global-density-pass updates).

### Req-7: Standardize Status Indicators

All status indicators (health dots, priority badges, state labels) must use the ds-* status color tokens:

| Status | Background | Text | Dot |
|--------|-----------|------|-----|
| Success/Healthy/Active | bg-green-700/20 | text-green-700 | bg-green-700 |
| Warning/Degraded | bg-amber-700/20 | text-amber-700 | bg-amber-700 |
| Error/Critical/Offline | bg-red-700/20 | text-red-700 | bg-red-700 |
| Info/Pending | bg-blue-700/20 | text-blue-700 | bg-blue-700 |
| Neutral/Unknown | bg-ds-gray-alpha-200 | text-ds-gray-700 | bg-ds-gray-600 |

Replace all instances where status indicators use different color values (e.g., bg-[#EF4444] should be bg-red-700, bg-[#F97316] should be bg-amber-700).

Affected components: ServerHealth, ObligationItem, ApprovalQueueItem, ProjectAccordion, Sidebar (WS status), ServiceRow, obligations/page priority dots.

### Req-8: Verify Font Loading

Audit font rendering across all pages:
- Confirm Geist Sans Variable renders for all body text (check font-family in computed styles)
- Confirm Geist Mono Variable renders for mono elements (timestamps, IDs, code blocks)
- Verify the @font-face declarations in globals.css are correctly resolved (woff2 path)
- Verify font-display: swap is working (text visible during font load)
- Check error boundaries and dynamically loaded content (modals, drawers, popovers) inherit font-family from the root

This is a verification task, not a code change task. If issues are found, create fix tasks.

## Scope
- **IN**: Replace raw Tailwind colors with ds-* tokens, consolidate platform brand colors into shared module, standardize typography to type scale classes, standardize borders to ds-gray-alpha tokens, tighten vertical spacing, standardize empty states, standardize status indicators, verify font loading
- **OUT**: New design tokens, new CSS custom properties, changes to globals.css or tailwind.config.ts token definitions, data model changes, API changes, new features, component refactoring beyond class name changes

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/lib/brand-colors.ts` | NEW -- shared platform brand color map |
| `apps/dashboard/components/ObligationItem.tsx` | MODIFY -- replace hex colors with ds-* status tokens |
| `apps/dashboard/components/approvals/ApprovalQueueItem.tsx` | MODIFY -- replace hex colors with ds-* status tokens |
| `apps/dashboard/components/ServerHealth.tsx` | MODIFY -- replace hex colors, raw Tailwind text sizes |
| `apps/dashboard/components/ProjectAccordion.tsx` | MODIFY -- replace hex colors, raw Tailwind text sizes |
| `apps/dashboard/components/SessionCard.tsx` | MODIFY -- replace CHANNEL_COLOR with brand-colors import, raw text sizes |
| `apps/dashboard/components/ActiveSession.tsx` | MODIFY -- replace CHANNEL_COLOR with brand-colors import, raw text sizes |
| `apps/dashboard/components/DiaryEntry.tsx` | MODIFY -- replace hex colors, raw text sizes |
| `apps/dashboard/components/ContactCard.tsx` | MODIFY -- replace raw text sizes, standardize colors |
| `apps/dashboard/components/ContactDetailPanel.tsx` | MODIFY -- replace raw text sizes, tighten spacing |
| `apps/dashboard/components/ServiceRow.tsx` | MODIFY -- replace status colors with ds-* tokens |
| `apps/dashboard/components/Sidebar.tsx` | MODIFY -- replace raw status colors with ds-* tokens |
| `apps/dashboard/components/NovaBadge.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/components/LeoBadge.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/components/SessionWidget.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/components/SessionDashboard.tsx` | MODIFY -- replace raw text sizes, tighten spacing |
| `apps/dashboard/components/LatencyChart.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/components/MemoryPreview.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/components/ObligationSummaryBar.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/components/ColdStartsPanel.tsx` | MODIFY -- tighten spacing |
| `apps/dashboard/components/ActivityFeed.tsx` | MODIFY -- tighten spacing |
| `apps/dashboard/components/layout/EmptyState.tsx` | MODIFY -- verify compact pattern with action button |
| `apps/dashboard/components/layout/SectionHeader.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/components/layout/ErrorBoundary.tsx` | MODIFY -- tighten spacing |
| `apps/dashboard/components/layout/PageSkeleton.tsx` | MODIFY -- tighten spacing |
| `apps/dashboard/app/page.tsx` | MODIFY -- replace raw colors, text sizes, tighten spacing |
| `apps/dashboard/app/chat/page.tsx` | MODIFY -- replace raw colors, CHANNEL_COLOR import, text sizes |
| `apps/dashboard/app/obligations/page.tsx` | MODIFY -- replace hex status colors, raw text sizes |
| `apps/dashboard/app/sessions/page.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/sessions/[id]/page.tsx` | MODIFY -- replace CHANNEL_COLOR, raw text sizes |
| `apps/dashboard/app/contacts/page.tsx` | MODIFY -- replace raw text sizes, tighten spacing |
| `apps/dashboard/app/messages/page.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/diary/page.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/briefing/page.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/usage/page.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/automations/page.tsx` | MODIFY -- replace raw colors, text sizes, tighten spacing |
| `apps/dashboard/app/nexus/page.tsx` | MODIFY -- tighten spacing |
| `apps/dashboard/app/integrations/page.tsx` | MODIFY -- tighten spacing |
| `apps/dashboard/app/login/page.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/settings/page.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/settings/components/SaveRestartBar.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/settings/components/SettingsSection.tsx` | MODIFY -- replace raw text sizes |
| `apps/dashboard/app/approvals/components/ApprovalQueueItem.tsx` | MODIFY -- replace hex colors |
| `apps/dashboard/lib/markdown.tsx` | MODIFY -- replace raw text sizes |

## Risks

| Risk | Mitigation |
|------|-----------|
| Changing color values shifts visual appearance in subtle ways | ds-* tokens are intentionally close to the raw Tailwind grays; test each page visually after changes. The tailwind.config.ts already maps green-700, amber-700 etc. to the Geist values, so bg-green-700 already resolves correctly. |
| Replacing text-sm with text-copy-13 changes line-height and letter-spacing | This is intentional -- the type scale provides consistent vertical rhythm. Review dense lists and tables after changes to verify readability. |
| Spacing reduction may cause content to feel cramped | Apply in batches per page; compare against Vercel dashboard screenshots for density benchmarking. Err toward tighter -- it is easier to add space back than to remove it. |
| Overlap with global-density-pass on spacing changes | Check global-density-pass task completion status before modifying shared components (PageShell, EmptyState, StatCard, Sidebar). This spec only touches files not already addressed by that spec. |
| Platform brand colors (Telegram #229ED9, Discord #5865F2) are not in ds-* | This is by design -- brand colors are external and do not belong in the design token system. The shared brand-colors.ts module documents them in one place. |
| Empty state standardization removes visual personality | The goal is data-dense and functional, not decorative. A clear text line + action button is more useful than a large illustration when the page has no data. |
