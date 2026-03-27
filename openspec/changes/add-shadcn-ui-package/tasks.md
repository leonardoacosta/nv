# Implementation Tasks

<!-- beads:epic:nv-6yql -->

## DB Batch

(No database changes required for this spec.)

## API Batch

- [ ] [2.1] [P-1] Create `packages/ui/` scaffold: `package.json` (`@nova/ui`), `tsconfig.json`, `components.json`, `tailwind.config.ts`, `src/lib/utils.ts` with `cn()` helper [owner:api-engineer] [beads:nv-6nfs]
- [ ] [2.2] [P-1] Add shadcn CSS variable bridge to `apps/dashboard/app/globals.css` -- map `ds-*` hex values to HSL-format `--background`, `--foreground`, `--card`, `--muted`, `--accent`, `--destructive`, `--border`, `--input`, `--ring`, `--primary`, `--secondary`, `--popover` variables [owner:api-engineer] [beads:nv-0x7x]
- [ ] [2.3] [P-2] Update `apps/dashboard/tailwind.config.ts` to include `../../packages/ui/src/**/*.{ts,tsx}` in the content array [owner:api-engineer] [beads:nv-67h3]
- [ ] [2.4] [P-2] Add `"@nova/ui": "workspace:*"` dependency to `apps/dashboard/package.json` and install [owner:api-engineer] [beads:nv-9fqe]
- [ ] [2.5] [P-2] Create `packages/ui/src/index.ts` barrel export file [owner:api-engineer] [beads:nv-h3dr]

## UI Batch

- [ ] [3.1] [P-1] Add shadcn Button component to `packages/ui/` with variants: default, secondary, destructive, ghost, outline, link; sizes: default, sm, lg, icon [owner:ui-engineer] [beads:nv-pami]
- [ ] [3.2] [P-1] Add shadcn Badge component to `packages/ui/` with variants: default, destructive, success, warning, outline [owner:ui-engineer] [beads:nv-q0nx]
- [ ] [3.3] [P-1] Add shadcn Input and Label components to `packages/ui/` themed with `ds-gray-100` bg, `ds-gray-400` border, `ds-gray-1000/60` focus ring [owner:ui-engineer] [beads:nv-baq7]
- [ ] [3.4] [P-1] Add shadcn Select component to `packages/ui/` with Radix dropdown, themed trigger and content [owner:ui-engineer] [beads:nv-dnk6]
- [ ] [3.5] [P-1] Add shadcn Skeleton component to `packages/ui/` using the existing `animate-shimmer` gradient [owner:ui-engineer] [beads:nv-hopa]
- [ ] [3.6] [P-1] Add shadcn Separator component to `packages/ui/` using `ds-gray-400` color [owner:ui-engineer] [beads:nv-dp5s]
- [ ] [3.7] [P-1] Add shadcn Dialog component to `packages/ui/` with `bg-ds-bg-100` content, `bg-black/40 backdrop-blur-sm` overlay [owner:ui-engineer] [beads:nv-qzf1]
- [ ] [3.8] [P-1] Add shadcn Card component to `packages/ui/` with default className applying the `surface-card` material styling [owner:ui-engineer] [beads:nv-w9sr]
- [ ] [3.9] [P-1] Add shadcn Alert component to `packages/ui/` with destructive variant using `ds-red-700` [owner:ui-engineer] [beads:nv-w0zs]
- [ ] [3.10] [P-1] Add shadcn ScrollArea component to `packages/ui/` with `ds-gray-400` thumb [owner:ui-engineer] [beads:nv-h2jm]
- [ ] [3.11] [P-2] Refactor `CreateProjectDialog` -- replace hand-built modal with `<Dialog>` from `@nova/ui`, replace `<input>`/`<label>`/`<select>`/submit button with `<Input>`, `<Label>`, `<Select>`, `<Button>` [owner:ui-engineer] [beads:nv-kvmy]
- [ ] [3.12] [P-2] Refactor `ContactCard` -- replace inline badge spans with `<Badge>` from `@nova/ui` [owner:ui-engineer] [beads:nv-79fj]
- [ ] [3.13] [P-2] Refactor `ErrorBanner` -- compose with `<Alert variant="destructive">` from `@nova/ui`, keep retry button as child [owner:ui-engineer] [beads:nv-orox]
- [ ] [3.14] [P-2] Refactor `PageSkeleton` -- replace shimmer divs with `<Skeleton>` from `@nova/ui` [owner:ui-engineer] [beads:nv-ol3d]
- [ ] [3.15] [P-2] Refactor `BatchActionBar` -- replace inline button elements with `<Button variant="...">` from `@nova/ui` [owner:ui-engineer] [beads:nv-py23]
- [ ] [3.16] [P-2] Refactor `ApprovalQueueItem` -- replace inline badge/button patterns with `<Badge>` and `<Button variant="ghost">` from `@nova/ui` [owner:ui-engineer] [beads:nv-d70z]
- [ ] [3.17] [P-2] Update `packages/ui/src/index.ts` barrel export with all added components [owner:ui-engineer] [beads:nv-fpog]

## E2E Batch

- [ ] [4.1] Verify `pnpm build` succeeds for both `packages/ui` and `apps/dashboard` with no type errors [owner:e2e-engineer] [beads:nv-pj26]
- [ ] [4.2] Visual verification: compare refactored `CreateProjectDialog`, `ErrorBanner`, `ContactCard`, `PageSkeleton`, `BatchActionBar`, `ApprovalQueueItem` against current rendered appearance -- no visual regressions [owner:e2e-engineer] [beads:nv-e0hz]
