# Design: add-shadcn-ui-package

## Architecture

### Package Structure

```
packages/ui/
  package.json          # @nova/ui, deps: tailwindcss, @radix-ui/*, class-variance-authority, clsx, tailwind-merge
  tsconfig.json         # extends root tsconfig, paths for @/
  tailwind.config.ts    # extends the Geist token palette from apps/dashboard
  components.json       # shadcn CLI config (style: "default", rsc: false, aliases)
  src/
    index.ts            # barrel export of all components
    lib/
      utils.ts          # cn() helper (clsx + tailwind-merge)
    components/
      button.tsx
      badge.tsx
      input.tsx
      label.tsx
      select.tsx
      skeleton.tsx
      separator.tsx
      dialog.tsx
      card.tsx
      alert.tsx
      scroll-area.tsx
```

### Token Bridge Strategy

The bridge lives in `apps/dashboard/app/globals.css` alongside the existing `:root` block. shadcn expects HSL values without the `hsl()` wrapper (e.g., `--background: 0 0% 4%`). The bridge converts the existing hex `ds-*` values to HSL notation.

```css
:root {
  /* Existing ds-* variables (unchanged) */
  --ds-background-100: #0a0a0a;
  ...

  /* shadcn bridge -- derived from ds-* values */
  --background: 0 0% 4%;          /* ds-background-100: #0a0a0a */
  --foreground: 0 0% 93%;         /* ds-gray-1000: #ededed */
  --card: 0 0% 10%;               /* ds-gray-100: #1a1a1a */
  --card-foreground: 0 0% 93%;    /* ds-gray-1000 */
  --popover: 0 0% 7%;             /* ds-bg-200: #111111 */
  --popover-foreground: 0 0% 93%; /* ds-gray-1000 */
  --primary: 0 0% 93%;            /* ds-gray-1000 (inverted button) */
  --primary-foreground: 0 0% 4%;  /* ds-background-100 */
  --secondary: 0 0% 10%;          /* ds-gray-100 */
  --secondary-foreground: 0 0% 93%;
  --muted: 0 0% 10%;              /* ds-gray-100 */
  --muted-foreground: 0 0% 63%;   /* ds-gray-900: #a0a0a0 */
  --accent: 0 0% 12%;             /* ds-gray-200: #1f1f1f */
  --accent-foreground: 0 0% 93%;
  --destructive: 1 74% 57%;       /* ds-red-700: #e5484d */
  --destructive-foreground: 0 0% 93%;
  --border: 0 0% 18%;             /* ds-gray-400: #2e2e2e */
  --input: 0 0% 18%;              /* ds-gray-400 */
  --ring: 0 0% 27%;               /* ds-gray-500: #454545 */
  --radius: 0.75rem;              /* 12px, matching surface-card border-radius */
}
```

### Why shadcn over alternatives

| Option | Verdict | Reasoning |
|--------|---------|-----------|
| shadcn/ui | Chosen | Zero runtime CSS, copy-paste ownership, Radix accessibility, Tailwind-native |
| Radix Themes | Rejected | Opinionated styling conflicts with existing Geist tokens |
| Headless UI | Rejected | Fewer primitives, React 19 compatibility uncertain |
| Ark UI | Rejected | Smaller community, less Next.js ecosystem alignment |
| Keep hand-building | Rejected | No accessibility, high maintenance cost per new component |

### Migration Approach

**Wave 1 (Leaf Primitives)** -- Can be done in parallel since components are independent:
1. Button, Badge, Separator -- zero state, pure display
2. Input, Label -- form primitives
3. Select -- Radix dropdown (only used in CreateProjectDialog)
4. Skeleton -- replace shimmer divs

**Wave 2 (Composed Primitives)** -- Sequential, depends on Wave 1 being importable:
1. Dialog -- refactor CreateProjectDialog
2. Card -- optional adoption (existing surface-card class still works)
3. Alert -- refactor ErrorBanner
4. ScrollArea -- progressive adoption where beneficial

**Not migrated (domain components)**: Sidebar, KanbanBoard, all chart/visualization components, all detail panels, brand marks. These keep their existing implementation. They may incrementally adopt `<Button>` or `<Badge>` from `@nova/ui` as leaf-node replacements within their own JSX.

### CVA Variant Design

Button variants aligned to existing patterns found in the codebase:

| Variant | Existing Pattern | shadcn Mapping |
|---------|-----------------|----------------|
| default (primary) | `bg-ds-gray-1000 text-ds-bg-100 hover:bg-ds-gray-900` | `--primary` / `--primary-foreground` |
| secondary | `bg-ds-gray-100 border-ds-gray-400 text-ds-gray-1000` | `--secondary` / `--secondary-foreground` |
| destructive | `bg-red-700 text-white` | `--destructive` / `--destructive-foreground` |
| ghost | `hover:bg-ds-gray-alpha-100 text-ds-gray-900` | transparent bg, hover accent |
| outline | `border border-ds-gray-400 text-ds-gray-1000 hover:border-ds-gray-500` | border + transparent bg |
| link | `text-blue-700 hover:underline` | no bg, underline on hover |

Badge variants:

| Variant | Existing Pattern | Use Case |
|---------|-----------------|----------|
| default | `bg-ds-gray-alpha-200 text-ds-gray-1000` | Neutral status |
| destructive | `bg-red-700/20 text-red-700` | Error/critical |
| success | `bg-green-700/20 text-green-700` | Connected/complete |
| warning | `bg-amber-700/20 text-amber-700` | In-progress/approaching |
| outline | `border border-ds-gray-400 text-ds-gray-900` | Muted/informational |
