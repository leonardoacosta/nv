# Spec: Tailwind v4 Migration

## MODIFIED Requirements

### Requirement: Dashboard Build Tooling -- Tailwind CSS Integration
The dashboard CSS pipeline SHALL upgrade from Tailwind CSS v3 (JS config + PostCSS + autoprefixer) to Tailwind CSS v4 (CSS-first config with `@theme` directive and `@tailwindcss/postcss`).

#### Scenario: Package dependency swap
**Given** the dashboard uses `tailwindcss@^3.4.17`, `postcss@^8.5.1`, and `autoprefixer@^10.4.20`
**When** the migration runs
**Then** `tailwindcss` is upgraded to `^4.x`, `@tailwindcss/postcss` is added, and `autoprefixer` is removed from devDependencies

#### Scenario: PostCSS config migration
**Given** `postcss.config.js` lists `tailwindcss` and `autoprefixer` as plugins
**When** the migration runs
**Then** `postcss.config.js` lists only `@tailwindcss/postcss` as a plugin

#### Scenario: CSS entry point migration
**Given** `globals.css` contains `@tailwind base`, `@tailwind components`, `@tailwind utilities`
**When** the migration runs
**Then** those three directives are replaced with `@import "tailwindcss"` at the top of the file (after `@font-face` declarations)

#### Scenario: Theme configuration moves to CSS
**Given** `tailwind.config.ts` defines colors (ds.bg, ds.gray, blue, green, amber, red) and fontFamily (sans, mono) under `theme.extend`
**When** the migration runs
**Then** an `@theme` block in `globals.css` defines equivalent `--color-ds-*`, `--color-blue-*`, `--color-green-*`, `--color-amber-*`, `--color-red-*`, `--font-sans`, and `--font-mono` variables, and `tailwind.config.ts` is deleted

#### Scenario: @apply directives still resolve
**Given** `globals.css` uses `@apply bg-ds-bg-100 text-ds-gray-1000 font-sans` and three other `@apply` statements
**When** the theme is defined via `@theme`
**Then** all `@apply` directives resolve correctly and produce the same CSS output as v3

#### Scenario: Breaking class name renames are fixed
**Given** Tailwind v4 renames shadow/ring/blur/rounded/outline utilities
**When** the migration runs
**Then** all affected classes in `.tsx` files are updated to their v4 equivalents (e.g., `shadow-sm` -> `shadow-xs`, `ring` -> `ring-3`, `outline-none` -> `outline-hidden`)

#### Scenario: CSS @property declarations preserved
**Given** `globals.css` contains `@property --num` for countUp animations
**When** the migration runs
**Then** the `@property` declaration is unchanged and the countUp animation still functions

#### Scenario: Dark mode CSS variables preserved
**Given** `:root` defines `--ds-background-*`, `--ds-gray-*`, `--ds-blue-*` etc. as hardcoded dark hex values
**When** the migration runs
**Then** all CSS custom properties remain in `:root` and resolve to the same hex values

#### Scenario: Build passes
**Given** the migration is complete
**When** `pnpm build` runs in `apps/dashboard`
**Then** the build succeeds with zero errors

## ADDED Requirements

### Requirement: Tailwind v4 @theme Color Namespace
Custom colors MUST be registered under the `@theme` directive using v4's `--color-*` namespace convention so that Tailwind utility classes (e.g., `bg-ds-bg-100`, `text-ds-gray-1000`) continue to resolve.

#### Scenario: ds-* color classes resolve to correct hex values
**Given** `@theme` defines `--color-ds-bg-100: #0a0a0a` and `--color-ds-gray-1000: #ededed`
**When** a component uses `bg-ds-bg-100` or `text-ds-gray-1000`
**Then** the generated CSS applies `background-color: #0a0a0a` and `color: #ededed` respectively

#### Scenario: Status color classes resolve through @theme
**Given** `@theme` defines `--color-blue-700: #0070f3`, `--color-green-700: #0cce6b`, etc.
**When** a component uses `bg-blue-700` or `text-green-700`
**Then** the generated CSS applies the custom hex values, not Tailwind's default palette

#### Scenario: Alpha color variants work with custom hex
**Given** `@theme` defines `--color-ds-gray-alpha-100: rgba(255,255,255,0.03)`
**When** a component uses `bg-ds-gray-alpha-100`
**Then** the generated CSS applies `background-color: rgba(255,255,255,0.03)`
