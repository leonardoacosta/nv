# Proposal: Add Animation System

## Change ID
`add-animation-system`

## Summary

Add 10 reusable CSS animation utilities to the dashboard's Geist design system -- countUp numbers, crossfade tab transitions, slide-out dismiss, height-reveal accordions, bar-grow charts, glow-pulse for active states, SVG path-draw, and direction-aware date slides.

## Context

- Extends: `apps/dashboard/app/globals.css` (existing animations: fade-in-up, shimmer, stagger-1..10)
- No animation library needed -- pure CSS keyframes + transitions

## Motivation

The dashboard currently has only 2 animations (fade-in-up, shimmer). All page transitions, tab switches, card dismissals, and data visualizations are instant. Adding 10 reusable CSS utilities creates a cohesive motion language that makes the interface feel alive without adding dependencies.

## Requirements

### Req-1: countUp animation

CSS `@property --num` interpolation in `globals.css` plus a 15-line React hook (`useCountUp`) at `hooks/useCountUp.ts`. Duration 600-800ms ease-out. Used on all StatCards, cost values, percentiles.

### Req-2: crossfade utility

Opacity `1 -> 0 -> 1` transition, 150ms ease, for tab content transitions. Used on all tab switches throughout the dashboard.

### Req-3: slide-out-left and slide-out-right

`translateX(0) -> translateX(+-100%)` with `opacity 1 -> 0`, 300ms ease-in. Used for approval approve/dismiss, obligation completion.

### Req-4: height-reveal

`grid-template-rows 0fr -> 1fr` plus opacity, 200ms ease-out. Used for obligation detail expansion, settings sections, project accordions.

### Req-5: bar-grow

`width 0 -> target%`, 400ms ease-out, staggered. Used on Usage page and Cold Starts.

### Req-6: glow-pulse

`box-shadow` green/amber keyframe, 2s infinite ease-in-out. Used on active sessions and connected integrations.

### Req-7: path-draw

SVG `stroke-dasharray`/`stroke-dashoffset` animation, 800ms ease-out. Used on Cold Starts latency chart.

### Req-8: slide-date

Direction-aware `translateX(+-20px)` plus opacity, 200ms ease. Used on Diary date nav and Briefing history.

### Req-9: Stagger improvements

Extend existing stagger utilities to work with all new animations, not just fade-in-up.

### Req-10: Section-level stagger

Each page section group (stats, content, sidebar) should animate in with 100ms stagger between groups.

## Scope

- **IN**: CSS keyframes in globals.css, React hooks for countUp, utility classes for all animations
- **OUT**: Third-party animation libraries (framer-motion, react-spring), WebGL/canvas effects, page transition animations (Next.js level)

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/app/globals.css` | Extended -- add keyframes + utility classes for all 10 animations |
| `apps/dashboard/hooks/useCountUp.ts` | New file -- React hook for countUp number interpolation |
| `apps/dashboard/components/StatCard.tsx` | Extended -- integrate useCountUp for numeric values |

## Risks

| Risk | Mitigation |
|------|-----------|
| Animation performance on low-end devices | Use `prefers-reduced-motion` media query to disable all animations |
| CSS-only approach limits complex sequences | Pure CSS keyframes minimize paint/layout thrashing; sufficient for planned animations |
| `@property` CSS support in older browsers | Progressive enhancement -- numbers display instantly without animation on unsupported browsers |
