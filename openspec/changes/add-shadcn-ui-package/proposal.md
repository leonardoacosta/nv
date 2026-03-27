# Proposal: Add shadcn/ui and Shared UI Package

## Change ID
`add-shadcn-ui-package`

## Summary
Create a `packages/ui/` workspace package initialized with shadcn/ui, map the existing `ds-*` Geist design tokens to shadcn's CSS variable system, and migrate dashboard components from hand-built primitives to accessible shadcn/Radix-based alternatives on a component-by-component basis.

## Context
- Extends: `apps/dashboard/components/`, `apps/dashboard/app/globals.css`, `apps/dashboard/tailwind.config.ts`, root `package.json` workspaces
- Related: The dashboard has 30 hand-built `.tsx` components plus 18 more in subdirectories (layout/, approvals/, obligations/, providers/). All use custom `ds-*` Tailwind tokens (Geist-inspired dark theme) with no Radix, CVA, or shadcn primitives.

## Motivation
The dashboard currently relies on 48 hand-built components with no accessibility primitives (no ARIA dialog management, no keyboard-navigable selects, no focus trapping). Every new component reinvents basic UI patterns (buttons, badges, inputs, dialogs) from raw divs. shadcn/ui provides accessible, composable, unstyled Radix primitives that can be themed to match the existing Geist aesthetic. A shared `packages/ui/` package also enables future consumers (admin tools, CLI web views) to reuse the design system without duplicating component code.

## Requirements

### Req-1: Shared UI Package
Create `packages/ui/` as a pnpm workspace package (`@nova/ui`) that exports shadcn/ui components themed with the project's existing Geist dark design tokens. The package must be consumable by `apps/dashboard` and any future app in the monorepo.

### Req-2: Design Token Bridge
Map the existing `ds-*` CSS custom properties to shadcn's expected CSS variable names (`--background`, `--foreground`, `--card`, `--muted`, `--accent`, `--destructive`, `--border`, `--input`, `--ring`, `--primary`, `--secondary`, `--popover`). The `ds-*` variables remain the source of truth; shadcn variables are derived aliases. The custom Geist type scale classes (`text-heading-*`, `text-label-*`, `text-copy-*`, `text-button-*`) and surface materials (`surface-card`, `surface-base`, `surface-raised`, `surface-inset`) remain in `globals.css` unchanged.

### Req-3: Component Migration (Wave 1 -- Leaf Primitives)
Replace the following hand-built patterns with shadcn equivalents:
- **Button**: The 8+ inline button patterns across components (submit buttons, ghost buttons, icon buttons) become `<Button variant="..." size="...">`.
- **Badge**: The 12+ inline badge/pill patterns (relationship badges, status badges, priority badges, channel badges) become `<Badge variant="...">`.
- **Input / Label**: The form fields in `CreateProjectDialog` become shadcn `<Input>` + `<Label>`.
- **Select**: The native `<select>` in `CreateProjectDialog` becomes shadcn `<Select>` (Radix-based, keyboard-navigable).
- **Skeleton**: `PageSkeleton` and shimmer divs become shadcn `<Skeleton>`.
- **Separator**: Border-bottom dividers become `<Separator>`.

### Req-4: Component Migration (Wave 2 -- Composed Primitives)
Replace composed patterns with shadcn equivalents:
- **Dialog**: `CreateProjectDialog`'s hand-built modal (backdrop, escape handler, focus management) becomes shadcn `<Dialog>`.
- **Card**: `surface-card` usage in `StatCard`, `ContactCard`, `ProjectCard`, `ActivityFeed` becomes shadcn `<Card>` with the existing surface-card styling applied via className.
- **ScrollArea**: The custom scrollbar CSS and `overflow-y-auto` patterns become shadcn `<ScrollArea>`.
- **Alert**: `ErrorBanner` becomes shadcn `<Alert variant="destructive">`.

### Req-5: Custom Components Retained
The following components are domain-specific and should NOT be replaced by shadcn, but may adopt Radix primitives internally where beneficial:
- `Sidebar` (complex nav with collapse, WS status, approval count -- could use Radix Collapsible)
- `KanbanBoard` / `KanbanColumn` / `KanbanCard` / `KanbanLane` (drag-and-drop obligation management)
- `SessionDashboard` / `SessionTimelineEvent` (daemon session control)
- `LatencyChart` / `MiniChart` / `UsageSparkline` (SVG data visualization)
- `MemoryPreview` / `ColdStartsPanel` / `ServerHealth` (data-dense monitoring panels)
- `ContactDetailPanel` / `ProjectDetailPanel` (detail views with domain logic)
- `NovaMark` / `NovaBadge` / `LeoBadge` (brand identity)
- `AppShell` / `PageShell` (layout shells)
- `ObligationItem` / `ObligationSummaryBar` (domain-specific display)
- `DiaryEntry` (markdown rendering)

## Scope
- **IN**: `packages/ui/` creation, shadcn init + theme config, token bridge CSS, Wave 1 + Wave 2 component migrations, updating `apps/dashboard` imports
- **OUT**: Tailwind 4 upgrade (separate concern), new components not already in the dashboard, changing the visual design (colors, spacing, typography must remain identical), migrating chart/visualization components, adding Storybook or component documentation tooling

## Impact
| Area | Change |
|------|--------|
| `packages/ui/` | New package: shadcn components, tailwind config, CSS token bridge |
| `apps/dashboard/package.json` | Add `@nova/ui` dependency, add `@radix-ui/*` transitive deps |
| `apps/dashboard/components/` | ~12 components refactored to use `@nova/ui` imports |
| `apps/dashboard/app/globals.css` | Add shadcn CSS variable aliases alongside existing `ds-*` vars |
| `apps/dashboard/tailwind.config.ts` | Extend with shadcn `content` path for `packages/ui/` |
| Root `package.json` | Already includes `packages/*` in workspaces -- no change needed |

## Risks
| Risk | Mitigation |
|------|-----------|
| Visual regression from token mapping mismatch | Token bridge is a 1:1 alias layer -- `ds-*` values unchanged. Verify visually after each wave. |
| Bundle size increase from Radix runtime | Radix primitives are tree-shakeable; only imported components add weight. shadcn has zero runtime CSS -- it uses Tailwind classes. |
| Breaking existing component imports during migration | Migrate one component at a time. Old component files are replaced in-place -- no import path changes for consumers. |
| Tailwind content path misconfiguration | `packages/ui/` must be in the dashboard's `tailwind.config.ts` content array to avoid purging shared component classes. |
