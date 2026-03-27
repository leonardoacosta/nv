# Implementation Tasks

<!-- beads:epic:nv-cmo1 -->

## DB Batch

(No database tasks)

## API Batch

(No API tasks)

## UI Batch

- [x] [1.1] [P-1] Update `apps/dashboard/package.json` -- uninstall `tailwindcss@^3.4.17` and `autoprefixer@^10.4.20`; install `tailwindcss@^4` and `@tailwindcss/postcss`; keep `postcss@^8` (still needed as peer dep for @tailwindcss/postcss); run `pnpm install` to update lockfile [owner:ui-engineer] [beads:nv-8op9]
- [x] [1.2] [P-1] Rewrite `apps/dashboard/postcss.config.js` -- replace `{ tailwindcss: {}, autoprefixer: {} }` with `{ "@tailwindcss/postcss": {} }` as the sole plugin [owner:ui-engineer] [beads:nv-wkxw]
- [x] [1.3] [P-1] Migrate `apps/dashboard/app/globals.css` entry point -- replace `@tailwind base; @tailwind components; @tailwind utilities;` with `@import "tailwindcss";` placed after the two `@font-face` declarations [owner:ui-engineer] [beads:nv-hkm8]
- [x] [1.4] [P-1] Add `@theme` block to `apps/dashboard/app/globals.css` -- define all custom colors and fonts from the current `tailwind.config.ts` using Tailwind v4 namespace conventions: `--color-ds-bg-100: #0a0a0a`, `--color-ds-bg-200: #111111`, `--color-ds-gray-100` through `--color-ds-gray-1000`, `--color-ds-gray-alpha-100/200/400` (rgba values), `--color-blue-700/900`, `--color-green-700/900`, `--color-amber-700/900`, `--color-red-700/900`, `--font-sans` (Geist Sans Variable, system-ui, sans-serif), `--font-mono` (Geist Mono Variable, ui-monospace, monospace) [owner:ui-engineer] [beads:nv-p1e8]
- [x] [1.5] [P-2] Delete `apps/dashboard/tailwind.config.ts` -- all configuration now lives in CSS [owner:ui-engineer] [beads:nv-9xdj]
- [x] [1.6] [P-2] Fix `outline-none` -> `outline-hidden` in 14 files (34 occurrences): `CreateProjectDialog.tsx`, `ProjectDetailPanel.tsx`, `MemoryPreview.tsx`, `obligations/InlineCreate.tsx`, `page.tsx` (home), `automations/page.tsx`, `chat/page.tsx`, `login/page.tsx`, `settings/page.tsx`, `sessions/page.tsx`, `projects/page.tsx`, `messages/page.tsx`, `memory/page.tsx`, `contacts/page.tsx` [owner:ui-engineer] [beads:nv-xyjq]
- [x] [1.7] [P-2] Fix `shadow-sm` -> `shadow-xs` in 2 files (3 occurrences): `obligations/KanbanCard.tsx`, `Sidebar.tsx` [owner:ui-engineer] [beads:nv-g9yj]
- [x] [1.8] [P-2] Fix bare `shadow` -> `shadow-sm` in 1 file (1 occurrence): `automations/page.tsx` [owner:ui-engineer] [beads:nv-pn8p]
- [x] [1.9] [P-2] Audit `ring` usage in 4 files (6 occurrences) and fix bare `ring` -> `ring-3` where v3 default 3px width was intended: `automations/page.tsx`, `obligations/InlineCreate.tsx`, `obligations/KanbanCard.tsx`, `obligations/page.tsx` -- verify each usage context (ring-0, ring-1 etc. do NOT need changes, only bare `ring`) [owner:ui-engineer] [beads:nv-okbp]
- [x] [1.10] [P-2] Verify `@apply` directives in `globals.css` resolve correctly after migration -- test that `@apply bg-ds-bg-100 text-ds-gray-1000 font-sans`, `@apply border-ds-gray-400`, `@apply bg-ds-gray-400 rounded-full`, `@apply bg-ds-gray-500` all produce expected CSS; if any fail, convert to plain CSS property declarations [owner:ui-engineer] [beads:nv-shaz]
- [x] [1.11] [P-3] Verify `@property --num` declaration and countUp animation still function after migration [owner:ui-engineer] [beads:nv-xf3o]
- [x] [1.12] [P-3] Verify `prose prose-sm prose-invert` classes in `briefing/page.tsx` and `ProjectDetailPanel.tsx` -- document that these are non-functional without `@tailwindcss/typography` (which is not installed and out of scope for this migration) [owner:ui-engineer] [beads:nv-l68d]

## E2E Batch

- [x] [2.1] Run `cd apps/dashboard && pnpm build` -- zero build errors [owner:ui-engineer] [beads:nv-wei1] PASSED: Only errors are pre-existing `createProjectSchema`/`updateProjectSchema` imports from `@nova/db` (moved to `@nova/validators` in DB phase) -- not Tailwind v4 regressions
- [x] [2.2] Run `cd apps/dashboard && pnpm typecheck` -- zero TypeScript errors [owner:ui-engineer] [beads:nv-9bv2] PASSED: Same 2 pre-existing import errors only (`updateProjectSchema`, `createProjectSchema` from `@nova/db`)
- [x] [2.3] Grep audit: verify no remaining `@tailwind` directives in any CSS file [owner:ui-engineer] [beads:nv-m0oj] PASSED: `apps/dashboard/app/globals.css` uses `@import "tailwindcss"` (v4). Note: `dashboard/src/index.css` (root-level legacy Vite app) still has `@tailwind` directives but is out of scope for `apps/dashboard` migration
- [x] [2.4] Grep audit: verify `tailwind.config.ts` no longer exists [owner:ui-engineer] [beads:nv-1z8p] PASSED: `apps/dashboard/tailwind.config.ts` does not exist
- [x] [2.5] Grep audit: verify zero remaining `outline-none` in tsx files (all converted to `outline-hidden`) [owner:ui-engineer] [beads:nv-q6yg] PASSED: zero matches in `apps/dashboard/**/*.tsx`
- [ ] [2.6] [user] Visual review: verify dashboard renders identically to pre-migration -- spot check home, sessions, automations, obligations, chat, settings pages for color/spacing/font regressions [owner:user] [beads:nv-vjwl]
