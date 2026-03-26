# Spec: Dashboard Animation System

## ADDED Requirements

### Requirement: countUp number animation

The dashboard MUST provide a `useCountUp` React hook at `hooks/useCountUp.ts` and a corresponding CSS `@property --num` interpolation rule in `globals.css` that animates numeric values from 0 to their target over 600-800ms with ease-out easing. StatCard components SHALL use this hook for all numeric displays.

#### Scenario: StatCard animates from zero to target

Given a StatCard displaying a cost value of `$42.50`,
when the component mounts,
then the displayed number animates from `0` to `42.50` over 600-800ms with ease-out easing.

#### Scenario: countUp handles zero values

Given a StatCard displaying a value of `0`,
when the component mounts,
then no animation runs and `0` is displayed immediately.

### Requirement: crossfade tab transitions

The dashboard MUST provide a `.crossfade` utility class in `globals.css` that applies an opacity `1 -> 0 -> 1` transition over 150ms with ease timing. All tab-switching components SHALL use this utility for content transitions.

#### Scenario: Tab switch triggers crossfade

Given a tabbed interface with two tabs,
when the user switches from tab A to tab B,
then the outgoing content fades out and the incoming content fades in over 150ms total.

### Requirement: slide-out dismiss animations

The dashboard MUST provide `.slide-out-left` and `.slide-out-right` utility classes in `globals.css` that animate `translateX(0)` to `translateX(-100%)` or `translateX(100%)` respectively, with `opacity 1 -> 0` over 300ms ease-in.

#### Scenario: Approval card dismissed left

Given an approval card in the approvals list,
when the user dismisses the approval,
then the card slides out to the left with fading opacity over 300ms before being removed from the DOM.

#### Scenario: Obligation completed with slide-out

Given an obligation item marked as complete,
when the completion animation triggers,
then the item slides out to the right over 300ms ease-in.

### Requirement: height-reveal accordion animation

The dashboard MUST provide a `.height-reveal` utility class in `globals.css` using the `grid-template-rows: 0fr -> 1fr` technique combined with opacity transition, over 200ms ease-out.

#### Scenario: Obligation detail expands

Given a collapsed obligation item,
when the user clicks to expand details,
then the detail section smoothly reveals its height from 0 to full content height over 200ms with simultaneous opacity fade-in.

#### Scenario: Settings section collapses

Given an expanded settings section,
when the user clicks to collapse it,
then the section smoothly shrinks from full height to 0 over 200ms with opacity fade-out.

### Requirement: bar-grow chart animation

The dashboard MUST provide a `.bar-grow` utility class in `globals.css` that animates `width: 0` to the target percentage over 400ms ease-out, with staggered delays when multiple bars are present.

#### Scenario: Usage page bars animate in sequence

Given the Usage page with 5 token usage bars,
when the page loads,
then each bar grows from 0 to its target width over 400ms, with each successive bar starting 50-100ms after the previous one.

### Requirement: glow-pulse active state

The dashboard MUST provide a `.glow-pulse` utility class in `globals.css` that applies a `box-shadow` keyframe animation cycling through green or amber glow over 2s infinite ease-in-out.

#### Scenario: Active session shows glow

Given a session card with status "connected",
when the card is rendered,
then a green glow-pulse animation plays continuously on the card border/shadow.

### Requirement: path-draw SVG animation

The dashboard MUST provide a `.path-draw` utility class in `globals.css` that animates SVG paths using `stroke-dasharray` and `stroke-dashoffset` over 800ms ease-out.

#### Scenario: Latency chart line draws in

Given the Cold Starts latency chart with an SVG line path,
when the chart mounts,
then the line draws from left to right over 800ms ease-out.

### Requirement: slide-date direction-aware navigation

The dashboard MUST provide `.slide-date-left` and `.slide-date-right` utility classes in `globals.css` that animate `translateX(+-20px)` with opacity over 200ms ease, direction determined by navigation direction.

#### Scenario: Navigate to next date

Given the Diary page showing March 25,
when the user navigates forward to March 26,
then the date content slides left (old exits left, new enters from right) over 200ms.

#### Scenario: Navigate to previous date

Given the Briefing history showing the current briefing,
when the user navigates backward,
then the content slides right (old exits right, new enters from left) over 200ms.

### Requirement: prefers-reduced-motion support

The dashboard MUST include a `@media (prefers-reduced-motion: reduce)` block in `globals.css` that disables all animation utilities by setting `animation: none` and `transition: none` on all animation classes.

#### Scenario: Reduced motion preference active

Given a user with `prefers-reduced-motion: reduce` set in their OS,
when any dashboard page loads,
then no animations play and all content appears in its final state immediately.

### Requirement: Extended stagger system

The dashboard SHALL extend existing stagger utilities (stagger-1 through stagger-10) to work with all new animation classes, not just fade-in-up. Page section groups (stats, content, sidebar) MUST animate in with 100ms stagger between groups.

#### Scenario: Dashboard page sections stagger in

Given the Dashboard page with stats section, content section, and sidebar,
when the page loads,
then the stats section animates first, the content section starts 100ms later, and the sidebar starts 200ms after the stats section.
