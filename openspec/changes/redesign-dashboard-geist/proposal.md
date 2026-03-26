# Proposal: Redesign Dashboard with Geist Design System

## Change ID
`redesign-dashboard-geist`

## Summary

Overhaul the Nova dashboard's visual layer to align with Vercel's Geist design system while
preserving the cosmic dark identity. Fix flat surfaces, bare empty states, weak typography
hierarchy, missing transitions, and inconsistent component styling across all 15 pages.

## Context
- Extends: `apps/dashboard/` (all pages, components, globals.css, tailwind.config.ts)
- Related: rebuild-dashboard-wireframes (v7), all fix-dashboard-* remediation specs (current)
- Source: Geist design system (`--ds-*` token system, materials, type scale, status colors)

## Motivation

The dashboard was built by agents across 34 specs in a single session. Each agent produced
functional code but made independent styling decisions. The result:

1. **No surface elevation** -- Every card, panel, and container is the same visual depth. Geist
   uses material layers (base/small/medium/large) with distinct border-radius, background, and
   shadow. Currently everything is `bg-cosmic-surface border border-cosmic-border` with no
   variation.

2. **Flat stat cards** -- StatCard shows value + label but has no icon, no trend indicator, no
   accent color, no hover state. Compare: Geist's metric tiles have colored accent bars, subtle
   background tints, and bold value typography.

3. **Bare empty states** -- Pages show "No data found" text with no illustration, no contextual
   help, no CTA button. Geist's EmptyState pattern uses a centered icon (48px, muted), a title,
   a description, and an optional action button.

4. **Weak typography hierarchy** -- Page titles, section headers, card titles, labels, and body
   text all use similar sizes and weights. Geist specifies: page title = heading-24 (600),
   section label = label-14 (500 uppercase tracking-wide), body = copy-14, stat value = heading-32
   (700 tabular-nums).

5. **No transitions** -- Page loads are instant with no entrance animation. Cards pop into
   existence. Hover states are binary (no color to color). Geist uses 150ms ease transitions on
   interactive elements and staggered fade-in for lists.

6. **Error banners lack structure** -- Red background with text. Geist errors use `ds-red-100`
   background, `ds-red-400` border, `ds-red-900` text, with an icon prefix and optional retry
   action.

7. **Inconsistent border radius** -- Some elements use `rounded-cosmic` (12px), others use
   `rounded-lg` (8px), `rounded-md` (6px). Geist materials specify: base/small = 6px,
   medium/large = 12px, modal = 12px, fullscreen = 16px.

## Requirements

### Req-1: Token Layer -- Cosmic x Geist Hybrid

Extend `tailwind.config.ts` and `globals.css` with a Geist-compatible token system mapped to
cosmic colors. This enables using Geist patterns while keeping the cosmic identity.

```css
:root {
  /* Surface system (Geist materials mapped to cosmic) */
  --ds-background-100: #0F0B1A;     /* cosmic-dark */
  --ds-background-200: #1A1425;     /* cosmic-surface */

  /* Gray scale (cosmic-aligned) */
  --ds-gray-100: #1A1425;           /* component bg */
  --ds-gray-200: #211A30;           /* hover bg */
  --ds-gray-300: #2A2040;           /* active bg */
  --ds-gray-400: #2D2640;           /* border */
  --ds-gray-500: #3D3555;           /* hover border */
  --ds-gray-600: #4D4570;           /* active border */
  --ds-gray-700: #5D5580;           /* high contrast bg */
  --ds-gray-900: #6B5B8A;           /* secondary text */
  --ds-gray-1000: #E8E0F0;          /* primary text */

  /* Alpha overlays */
  --ds-gray-alpha-100: rgba(124, 58, 237, 0.04);
  --ds-gray-alpha-200: rgba(124, 58, 237, 0.08);
  --ds-gray-alpha-400: rgba(124, 58, 237, 0.15);

  /* Accent */
  --ds-purple-700: #7C3AED;         /* cosmic-purple */
  --ds-purple-900: #A78BFA;         /* lighter purple for text-on-dark */

  /* Status */
  --ds-green-700: #10B981;          /* emerald success */
  --ds-amber-700: #F59E0B;          /* amber warning */
  --ds-red-700: #F43F5E;            /* cosmic-rose error */
  --ds-blue-700: #3B82F6;           /* info */
}
```

### Req-2: Typography Hierarchy

Establish clear type scale classes using Geist conventions:

| Role | Class | Size | Weight | Tracking |
|------|-------|------|--------|----------|
| Page title | `text-heading-24` | 24px | 600 | -0.01em |
| Page subtitle | `text-copy-14` | 14px | 400 | 0 |
| Section header | `text-label-12` | 12px | 500 | 0.05em, uppercase |
| Card title | `text-label-16` | 16px | 500 | 0 |
| Stat value | `text-heading-32` | 32px | 700 | -0.02em, tabular-nums |
| Stat label | `text-label-13` | 13px | 400 | 0 |
| Body text | `text-copy-14` | 14px | 400 | 0 |
| Mono/code | `text-label-13-mono` | 13px | 400 | Geist Mono |
| Button | `text-button-14` | 14px | 500 | 0 |

Add these as Tailwind utility classes in globals.css.

### Req-3: Material Surfaces

Replace flat `bg-cosmic-surface border-cosmic-border` with layered materials:

| Material | Background | Border | Radius | Shadow | Usage |
|----------|-----------|--------|--------|--------|-------|
| `surface-base` | gray-100 | gray-400 | 6px | none | Default containers |
| `surface-card` | gray-100 | gray-alpha-400 | 12px | cosmic-sm | Cards, stat tiles |
| `surface-raised` | gray-200 | gray-500 | 12px | cosmic | Modals, drawers, popovers |
| `surface-overlay` | gray-300 | gray-600 | 16px | cosmic-lg | Full-screen overlays |
| `surface-inset` | background-100 | gray-alpha-200 | 6px | inset 0 1px 2px | Input fields, code blocks |

Implement as Tailwind `@apply` utility classes.

### Req-4: StatCard Redesign

Replace the current flat stat card with a Geist-inspired metric tile:

- Left accent bar (4px, colored by status: purple=default, green=healthy, amber=warning, red=error)
- Icon slot (24px, muted, from lucide-react)
- Value in `text-heading-32` with `tabular-nums` for alignment
- Label in `text-label-13` muted
- Optional trend indicator: up arrow green / down arrow red + percentage
- Hover: border transitions to gray-500, subtle background shift to gray-200
- Transition: all 150ms ease

### Req-5: Empty State Pattern

Replace bare "No data" text with structured empty states:

- Centered layout within parent
- Icon: 48px lucide icon, `text-cosmic-muted opacity-50`
- Title: `text-heading-16` (500), e.g. "No sessions found"
- Description: `text-copy-14` muted, max-width 320px, e.g. "Sessions will appear here when the daemon processes messages"
- Optional CTA button: `surface-card` style, e.g. "View documentation"
- Entrance: fade-in 300ms ease

Per-page empty states:
| Page | Icon | Title | Description |
|------|------|-------|-------------|
| /sessions | Layers | No sessions found | Sessions appear when the daemon processes messages |
| /messages | MessageSquare | No messages yet | Messages will appear as Nova processes conversations |
| /approvals | ShieldCheck | No pending approvals | Nova will ask for your approval when needed |
| /briefing | Sun | No briefing yet today | Nova generates a briefing each morning at 7am |
| /diary | BookOpen | No diary entries | Entries are logged as Nova processes each interaction |
| /contacts | Users | No contacts yet | Contacts are created as Nova encounters new people |
| /projects | FolderOpen | No projects found | Projects are detected from your conversations |
| /cold-starts | Timer | No cold start data | Data appears after the first message is processed |

### Req-6: Error Banner Redesign

Replace generic red banners with structured Geist-style error display:

- Background: `var(--ds-red-700)` at 10% opacity
- Border-left: 3px solid `var(--ds-red-700)`
- Icon: `AlertCircle` (16px) in `var(--ds-red-700)`
- Text: `text-copy-14` in `var(--ds-red-700)` light variant
- Optional retry button: ghost button aligned right
- Dismiss: X icon button, optional
- Border-radius: 6px (material-base)

### Req-7: Sidebar Polish

- Active item: `bg-gray-alpha-200` with `border-l-2 border-cosmic-purple` accent
- Hover: `bg-gray-alpha-100` transition 150ms
- Icons: 18px, `text-gray-900` default, `text-cosmic-purple` when active
- Section dividers between nav groups (Dashboard, Data, System)
- Footer: WebSocket status dot + "Connected"/"Reconnecting" label in `text-label-12`
- Logo area: Nova mark with `text-heading-16` "Nova" label, subtle bottom border

### Req-8: Page Load Transitions

- Page content: fade-in + translateY(8px) on mount, 200ms ease, via CSS `@keyframes`
- List items: staggered entrance, 50ms delay between items (max 10 items animated)
- Stat cards: staggered fade-in across the grid, 75ms delay
- Cards on hover: `transform: translateY(-1px)` + shadow upgrade, 150ms
- Buttons: scale(0.98) on active, 100ms

Implement via Tailwind animation utilities in globals.css:
```css
@keyframes fade-in-up {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
.animate-fade-in-up { animation: fade-in-up 200ms ease forwards; }
.animate-stagger-1 { animation-delay: 50ms; }
.animate-stagger-2 { animation-delay: 100ms; }
/* ... up to stagger-10 */
```

### Req-9: Interactive States

Every interactive element (buttons, cards, links, tabs) needs three states:

| State | Visual Change |
|-------|--------------|
| Default | Base styling |
| Hover | Background lightens (gray-200), border brightens (gray-500), 150ms ease |
| Active/Pressed | Background darkens (gray-300), scale(0.98), 100ms |
| Focus-visible | 2px ring in cosmic-purple with 2px offset, for keyboard navigation |
| Disabled | opacity-50, cursor-not-allowed, no hover effect |

### Req-10: Skeleton Loading States

Replace blank loading with animated skeletons matching content shape:

- Skeleton blocks: `bg-gray-alpha-200` with shimmer gradient animation
- Match the expected content layout (stat card shape for stat cards, row shape for lists)
- Shimmer: linear-gradient sweep from left to right, 1.5s infinite
- At least 3-5 skeleton items per list, 3-6 skeleton stat cards per grid

## Scope
- **IN**: Token system, typography utilities, material surfaces, StatCard, EmptyState, ErrorBanner,
  Sidebar polish, page transitions, skeleton loaders, interactive states, all 15 pages updated
- **OUT**: Page layout restructuring (handled by fix-dashboard-content-rendering), API proxy fixes,
  new pages, feature additions. This is pure visual layer.

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/tailwind.config.ts` | Extended with Geist token mappings, animation utilities |
| `apps/dashboard/app/globals.css` | Geist CSS vars, type scale classes, material classes, animations |
| `apps/dashboard/components/layout/StatCard.tsx` | Redesigned with accent bar, icon, trend |
| `apps/dashboard/components/layout/EmptyState.tsx` | Redesigned with icon, title, description, CTA |
| `apps/dashboard/components/layout/ErrorBanner.tsx` | Redesigned with Geist error pattern |
| `apps/dashboard/components/layout/PageSkeleton.tsx` | Redesigned with shimmer skeletons |
| `apps/dashboard/components/Sidebar.tsx` | Active states, section dividers, polish |
| `apps/dashboard/app/*/page.tsx` | All 15 pages: apply new typography, materials, transitions |

## Risks

| Risk | Mitigation |
|------|-----------|
| Visual regression on working pages | Apply changes page-by-page, verify each |
| Performance impact from animations | Use CSS-only animations (no JS), will-change hints |
| Token conflicts with existing cosmic classes | Geist tokens are additive (--ds-* namespace), cosmic classes stay |
| Inconsistent agent implementation | Provide exact class names in tasks, not descriptions |
