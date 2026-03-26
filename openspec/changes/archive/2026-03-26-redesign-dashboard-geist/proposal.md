# Proposal: Redesign Dashboard with Geist Design System

## Change ID
`redesign-dashboard-geist`

## Summary

Replace the cosmic purple theme entirely with Vercel's Geist design system. Pure neutral gray
scale, proper `--ds-*` token system, Geist materials for elevation, Geist type scale for hierarchy.
The dashboard should look like a Vercel product — clean, high-contrast dark mode with no purple.

## Context
- Extends: `apps/dashboard/` (all pages, components, globals.css, tailwind.config.ts)
- Replaces: All `cosmic-*` color tokens, `bg-cosmic-gradient`, `shadow-cosmic-*`, `rounded-cosmic`
- Source: Geist design system (`--ds-*` tokens, materials, type scale, status colors)

## Motivation

The current cosmic purple theme was a creative choice that doesn't match the product's identity.
Nova is a developer tool — it should look like one. Geist provides:

1. **Neutral gray scale** — `--ds-gray-100` through `--ds-gray-1000`, no purple tinting
2. **True dark mode** — `--ds-background-100: #0a0a0a`, `--ds-background-200: #111111`
3. **Systematic elevation** — materials define hierarchy through shadow/blur, not color
4. **High contrast** — accessible, readable, no murky purple-on-purple
5. **Consistent radius** — 6px (small), 12px (medium), 16px (large) only

## Requirements

### Req-1: Strip Cosmic Theme Entirely

Remove ALL cosmic-* tokens and replace with Geist tokens:

```css
/* DELETE these */
--color-cosmic-purple, --color-cosmic-rose, --color-cosmic-dark,
--color-cosmic-surface, --color-cosmic-border, --color-cosmic-muted,
--color-cosmic-text, --color-cosmic-bright

/* REPLACE with Geist dark mode tokens */
:root {
  --ds-background-100: #0a0a0a;
  --ds-background-200: #111111;

  --ds-gray-100: #1a1a1a;
  --ds-gray-200: #1f1f1f;
  --ds-gray-300: #292929;
  --ds-gray-400: #2e2e2e;
  --ds-gray-500: #454545;
  --ds-gray-600: #5e5e5e;
  --ds-gray-700: #6e6e6e;
  --ds-gray-800: #7c7c7c;
  --ds-gray-900: #a0a0a0;
  --ds-gray-1000: #ededed;

  --ds-gray-alpha-100: rgba(255,255,255,0.03);
  --ds-gray-alpha-200: rgba(255,255,255,0.06);
  --ds-gray-alpha-400: rgba(255,255,255,0.10);

  --ds-blue-700: #0070f3;
  --ds-blue-900: #52a8ff;
  --ds-green-700: #0cce6b;
  --ds-green-900: #52e78c;
  --ds-amber-700: #f5a623;
  --ds-amber-900: #ffcc4d;
  --ds-red-700: #e5484d;
  --ds-red-900: #ff6369;
}
```

Remove from tailwind.config.ts:
- `colors.cosmic` object
- `backgroundImage['cosmic-gradient']`
- `boxShadow['cosmic-*']`
- `borderRadius.cosmic`

Replace with Geist-mapped Tailwind tokens.

### Req-2: Tailwind Config — Geist Tokens

```ts
colors: {
  ds: {
    bg: { 100: '#0a0a0a', 200: '#111111' },
    gray: {
      100: '#1a1a1a', 200: '#1f1f1f', 300: '#292929',
      400: '#2e2e2e', 500: '#454545', 600: '#5e5e5e',
      700: '#6e6e6e', 800: '#7c7c7c', 900: '#a0a0a0',
      1000: '#ededed',
    },
  },
  blue: { 700: '#0070f3', 900: '#52a8ff' },
  green: { 700: '#0cce6b', 900: '#52e78c' },
  amber: { 700: '#f5a623', 900: '#ffcc4d' },
  red: { 700: '#e5484d', 900: '#ff6369' },
}
```

### Req-3: Typography — Geist Type Scale

Add utility classes matching Geist exactly:

| Role | Class | Size | Weight | Tracking |
|------|-------|------|--------|----------|
| Page title | `.text-heading-24` | 24px | 600 | -0.01em |
| Stat value | `.text-heading-32` | 32px | 700 | -0.02em, tabular-nums |
| Section header | `.text-label-12` | 12px | 500 | 0.05em, uppercase |
| Card title | `.text-label-16` | 16px | 500 | 0 |
| Body | `.text-copy-14` | 14px | 400 | 0 |
| Small label | `.text-label-13` | 13px | 400 | 0 |
| Mono | `.text-label-13-mono` | 13px | 400 | Geist Mono |
| Button | `.text-button-14` | 14px | 500 | 0 |

### Req-4: Materials — Surface Elevation

| Material | Background | Border | Radius | Shadow |
|----------|-----------|--------|--------|--------|
| `surface-base` | ds-gray-100 | ds-gray-400 | 6px | none |
| `surface-card` | ds-gray-100 | ds-gray-alpha-400 | 12px | 0 1px 2px rgba(0,0,0,0.3) |
| `surface-raised` | ds-gray-200 | ds-gray-500 | 12px | 0 4px 12px rgba(0,0,0,0.4) |
| `surface-inset` | ds-bg-100 | ds-gray-alpha-200 | 6px | inset 0 1px 2px rgba(0,0,0,0.4) |

Hover on cards: border → ds-gray-500, translateY(-1px), shadow upgrade. 150ms ease.

### Req-5: StatCard — Geist Style

- Clean card with `surface-card` material
- Small colored left accent bar (4px): green=success, amber=warning, red=error, default=ds-gray-600
- Icon (20px, ds-gray-700)
- Value in `.text-heading-32 tabular-nums text-ds-gray-1000`
- Label in `.text-label-13 text-ds-gray-900`
- Optional trend: arrow + percentage in green/red
- Hover lift + border brighten

### Req-6: Empty State — Geist Pattern

- Centered in container
- Icon: 40px, `text-ds-gray-600`
- Title: `.text-heading-16 text-ds-gray-1000`
- Description: `.text-copy-14 text-ds-gray-900 max-w-xs`
- Optional action button: `.surface-base` styled
- Fade-in entrance 200ms

### Req-7: Error Banner — Geist Pattern

- Background: `rgba(229, 72, 77, 0.08)` (red-700 at 8%)
- Left border: 3px solid ds-red-700
- Icon: AlertCircle 16px in ds-red-700
- Text: `.text-copy-14` in ds-red-900
- Retry button: ghost style
- Radius: 6px

### Req-8: Sidebar — Geist Navigation

- Background: `ds-bg-200`
- Logo: "Nova" in `.text-label-16 font-semibold text-ds-gray-1000`
- Nav items: `.text-label-14 text-ds-gray-900`, icon 18px
- Active: `bg-ds-gray-alpha-200`, `text-ds-gray-1000`, left border 2px `ds-gray-1000` (white, not purple)
- Hover: `bg-ds-gray-alpha-100`
- Section dividers: thin `border-ds-gray-alpha-200` + group labels `.text-label-12 uppercase text-ds-gray-700`
- Footer: WebSocket dot (green/amber/red) + status text `.text-label-12`

### Req-9: Page Transitions & Interactive States

Transitions:
- Page content: fade-in + translateY(8px), 200ms ease (CSS only)
- List items: staggered 50ms delay (max 10)
- Stat cards: staggered 75ms

Interactive states:
| State | Change |
|-------|--------|
| Hover | bg → ds-gray-200, border → ds-gray-500, 150ms |
| Active | bg → ds-gray-300, scale(0.98), 100ms |
| Focus-visible | 2px ring ds-blue-700 with 2px offset |
| Disabled | opacity-50, cursor-not-allowed |

### Req-10: Skeleton Loading

- Blocks: `bg-ds-gray-alpha-200` with shimmer gradient
- Shimmer: linear-gradient sweep, 1.5s infinite
- Match content shape per page variant

### Req-11: Global Search/Replace

Every file in `apps/dashboard/` must have cosmic references replaced:

| Find | Replace |
|------|---------|
| `bg-cosmic-dark` | `bg-ds-bg-100` |
| `bg-cosmic-surface` | `bg-ds-gray-100` |
| `bg-cosmic-gradient` | `bg-ds-bg-100` (remove gradient entirely) |
| `text-cosmic-text` | `text-ds-gray-1000` |
| `text-cosmic-bright` | `text-ds-gray-1000` |
| `text-cosmic-muted` | `text-ds-gray-900` |
| `text-cosmic-purple` | `text-ds-gray-1000` (or status color where semantic) |
| `border-cosmic-border` | `border-ds-gray-400` |
| `border-cosmic-purple` | `border-ds-gray-1000` (or status color) |
| `bg-cosmic-purple` | `bg-ds-gray-700` (or status color where semantic) |
| `bg-cosmic-rose` | `bg-red-700` |
| `shadow-cosmic-sm` | `shadow-sm` |
| `shadow-cosmic` | `shadow-md` |
| `shadow-cosmic-lg` | `shadow-lg` |
| `rounded-cosmic` | `rounded-xl` (12px) |

## Scope
- **IN**: Complete theme replacement, all token references, all 15+ pages, shared components,
  globals.css rewrite, tailwind.config.ts rewrite, design reference HTML
- **OUT**: Layout changes, API fixes, new features. Pure visual layer only.

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/tailwind.config.ts` | Complete rewrite: cosmic → Geist tokens |
| `apps/dashboard/app/globals.css` | Complete rewrite: CSS vars, type scale, materials, animations |
| `apps/dashboard/components/layout/*.tsx` | All 5 shared components redesigned |
| `apps/dashboard/components/Sidebar.tsx` | Geist nav styling |
| `apps/dashboard/app/*/page.tsx` | All pages: cosmic → Geist class replacement |
| `apps/dashboard/components/*.tsx` | All components: cosmic → Geist class replacement |

## Risks

| Risk | Mitigation |
|------|-----------|
| Breaking existing styling | Global search-replace is systematic — Req-11 table covers every cosmic class |
| Missing a cosmic reference | grep -r "cosmic" after replacement to catch stragglers |
| Performance | CSS-only animations, no runtime cost |
