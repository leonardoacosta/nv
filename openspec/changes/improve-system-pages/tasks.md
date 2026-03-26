# Implementation Tasks
<!-- beads:epic:nv-dtut -->

## UI Batch

- [x] [1.1] [P-1] Create `apps/dashboard/app/settings/components/SettingsSection.tsx` -- collapsible section component accepting title, item count, and children; default expanded; animate open/close with CSS transition; persist expanded state in localStorage [owner:ui-engineer] [beads:nv-u7yn]
- [x] [1.2] [P-1] Refactor `apps/dashboard/app/settings/page.tsx` -- group 28 settings into 4 categories (General, Network, Scheduling, Advanced) using SettingsSection; add green background save confirmation flash (300ms fade) on field save [owner:ui-engineer] [beads:nv-0xwo]
- [x] [1.3] [P-1] Create `apps/dashboard/app/settings/components/SaveRestartBar.tsx` -- fixed-bottom floating bar with unsaved changes count badge; appears when restart-required fields are dirty; "Save & Restart" and "Discard" actions [owner:ui-engineer] [beads:nv-7xaf]
- [x] [1.4] [P-1] Update `apps/dashboard/app/integrations/page.tsx` and IntegrationCard -- add deterministic hash-to-color function (8-color curated palette) for avatar backgrounds; dim disconnected items (opacity 0.6); elevate connected items (full opacity + shadow) [owner:ui-engineer] [beads:nv-9w6u]
- [x] [1.5] [P-2] Add connected pulse CSS animation -- 2s ease-in-out infinite green glow on "Connected" status badges; box-shadow only, no layout shift; define as Tailwind arbitrary animation or CSS keyframes [owner:ui-engineer] [beads:nv-7smn]
- [x] [1.6] [P-1] Update `apps/dashboard/app/memory/page.tsx` -- render memory file content as formatted markdown in detail panel using lightweight renderer; show last-modified timestamp and word count in file list sidebar [owner:ui-engineer] [beads:nv-vac8]
- [x] [1.7] [P-2] Run `pnpm typecheck` and `pnpm build` in `apps/dashboard/` -- zero errors [owner:ui-engineer] [beads:nv-5tyo]
