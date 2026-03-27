# Proposal: Upgrade Tailwind CSS v3 to v4

## Change ID
`upgrade-tailwind-v4`

## Summary
Upgrade the dashboard from Tailwind CSS 3.4.17 to Tailwind CSS 4.x, migrating from JS-based configuration to CSS-first configuration with the `@theme` directive, removing the PostCSS setup (Tailwind v4 is its own PostCSS plugin), and converting all `@tailwind` directives to `@import "tailwindcss"`.

## Context
- Extends: `apps/dashboard/tailwind.config.ts`, `apps/dashboard/postcss.config.js`, `apps/dashboard/app/globals.css`, `apps/dashboard/package.json`
- Related: `geist-token-standardization` (completed -- established the ds-* token system now being migrated), `global-density-pass` (completed -- spacing/layout)
- Affects: all 68 files using `ds-*` Tailwind classes, plus build tooling

## Motivation
Tailwind CSS v4 is a ground-up rewrite with significant DX improvements. The current setup uses Tailwind v3's JS-based configuration (`tailwind.config.ts`) with PostCSS and autoprefixer as separate dependencies. Tailwind v4 replaces this with:

1. **CSS-first configuration** -- theme customization moves into CSS via `@theme` instead of a JS config file, keeping design tokens co-located with the styles that consume them.
2. **Built-in PostCSS plugin** -- Tailwind v4 includes its own PostCSS integration, eliminating the need for separate `postcss` and `autoprefixer` packages.
3. **Automatic content detection** -- v4 automatically discovers template files, removing the need for the `content` array in configuration.
4. **Native CSS cascade layers** -- v4 uses `@layer` natively, improving specificity predictability.
5. **Modernized color system** -- v4 uses `oklch` color space by default for built-in colors, with better perceptual uniformity.

The dashboard's Geist token system (ds-* CSS variables + Tailwind config mapping) is a clean migration target since the tokens are already defined as CSS custom properties in `globals.css`. The JS config merely duplicates those values for Tailwind class generation.

## Requirements

### Req-1: Replace Package Dependencies
Uninstall `tailwindcss` v3, `postcss`, and `autoprefixer`. Install `tailwindcss` v4 and `@tailwindcss/postcss` (the v4 PostCSS integration). The `@tailwindcss/postcss` package replaces both the old `tailwindcss` PostCSS plugin and `autoprefixer`.

### Req-2: Migrate PostCSS Configuration
Replace `postcss.config.js` contents to use `@tailwindcss/postcss` as the sole plugin, removing `tailwindcss` and `autoprefixer` entries.

### Req-3: Migrate CSS Entry Point
In `apps/dashboard/app/globals.css`, replace the three `@tailwind` directives (`@tailwind base`, `@tailwind components`, `@tailwind utilities`) with a single `@import "tailwindcss"` at the top of the file.

### Req-4: Convert Theme Configuration to CSS @theme Directive
Move all theme customizations from `tailwind.config.ts` into a `@theme` block in `globals.css`. This includes:
- **Colors**: `ds.bg.100`, `ds.bg.200`, `ds.gray.*`, `ds.gray.alpha-*`, `blue.*`, `green.*`, `amber.*`, `red.*` -- mapped to `--color-ds-*` CSS theme variables
- **Font families**: `sans` (Geist Sans Variable) and `mono` (Geist Mono Variable) -- mapped to `--font-sans` and `--font-mono`

After migration, delete `tailwind.config.ts` entirely.

### Req-5: Preserve @layer Directives
Tailwind v4 uses native CSS `@layer` which is compatible with the existing `@layer base`, `@layer components`, and `@layer utilities` blocks in `globals.css`. Verify these continue to function correctly. The `@apply` directives within `@layer base` must still resolve against the new theme.

### Req-6: Handle @apply Compatibility
Tailwind v4 still supports `@apply` but with stricter resolution rules. Verify the four `@apply` usages in `globals.css` resolve correctly:
- `@apply bg-ds-bg-100 text-ds-gray-1000 font-sans` (html, body)
- `@apply border-ds-gray-400` (universal border default)
- `@apply bg-ds-gray-400 rounded-full` (scrollbar thumb)
- `@apply bg-ds-gray-500` (scrollbar thumb hover)

### Req-7: Handle Breaking Class Name Changes
Tailwind v4 renames or removes several utilities. Audit all 68 component/page files for:
- `shadow-sm` -> `shadow-xs`, `shadow` -> `shadow-sm` (shadow scale shift)
- `ring` -> `ring-3` (default ring width changed from 3px to 1px)
- `blur` -> `blur-sm` (blur scale shift)
- `rounded` -> `rounded-sm` (border-radius scale shift if using bare `rounded`)
- `outline-none` -> `outline-hidden` (renamed)
- `decoration-slice/clone` -> `box-decoration-slice/clone` (renamed)
- `flex-grow/shrink` -> `grow/shrink` (already aliased in v3, verify no v3-only forms)

### Req-8: Handle CSS @property Compatibility
The `@property --num` declaration in `globals.css` (used for countUp animations) is standard CSS and not Tailwind-specific. Verify it continues to work after migration -- it should, as Tailwind v4 does not interfere with standard CSS `@property` declarations.

### Req-9: Verify Dark Mode and CSS Variable System
The dashboard is dark-mode-only (no `dark:` variant classes exist anywhere). All colors are hardcoded dark values in CSS variables. Verify that:
- CSS custom properties in `:root` continue to resolve correctly
- No Tailwind v4 default light-mode styles bleed through
- The `prose prose-invert` classes (used in 2 files for markdown rendering) still function -- note that `@tailwindcss/typography` is NOT installed; these classes may be non-functional already

### Req-10: Verify Build Pipeline
Run `pnpm build` in the dashboard to confirm Next.js 15 + Tailwind v4 integration works end-to-end. The `next.config.ts` does not reference Tailwind directly, so no changes are expected there.

## Scope
- **IN**: Package upgrades, PostCSS config migration, CSS entry point migration, theme config conversion to `@theme`, `tailwind.config.ts` deletion, breaking class name audit and fixes, build verification
- **OUT**: New design tokens, new color definitions, visual redesign, component refactoring, adding `@tailwindcss/typography` plugin, changing the dark-mode-only approach, Next.js config changes

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/package.json` | MODIFY -- swap tailwindcss v3 for v4, add @tailwindcss/postcss, remove autoprefixer |
| `apps/dashboard/postcss.config.js` | MODIFY -- replace plugins with @tailwindcss/postcss only |
| `apps/dashboard/tailwind.config.ts` | DELETE -- configuration moves to CSS |
| `apps/dashboard/app/globals.css` | MODIFY -- replace @tailwind directives with @import, add @theme block with all color/font definitions |
| `apps/dashboard/**/*.tsx` (up to 68 files) | MODIFY -- fix any breaking class name changes (shadow, ring, blur, rounded, outline renames) |

## Risks

| Risk | Mitigation |
|------|-----------|
| Tailwind v4 default color palette uses oklch which may clash with hardcoded hex ds-* tokens | Custom theme colors defined in @theme override defaults; ds-* hex values are preserved exactly as-is |
| @apply in @layer base may not resolve in v4 | v4 still supports @apply; test early in migration. Fallback: convert @apply to plain CSS property declarations |
| Shadow/ring/rounded scale shifts cause subtle visual regressions across 68 files | Automated grep audit for affected class names before and after; batch-fix all occurrences |
| Next.js 15 + Tailwind v4 PostCSS integration may have edge cases | Next.js 15 officially supports Tailwind v4 via @tailwindcss/postcss; test with `pnpm build` |
| prose/prose-invert classes become broken without typography plugin | These classes are already non-functional (plugin not installed); migration does not change this. Document as known gap for future fix |
| Automatic content detection may miss files | Tailwind v4 detects .tsx files in the project root by default; verify all app/ and components/ files are scanned |
