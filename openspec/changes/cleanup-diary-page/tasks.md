# Implementation Tasks

<!-- beads:epic:nv-enas -->

## API Batch

- [x] [2.1] [P-1] Extend `DiaryGetResponse` in `types/api.ts` with `distinct_channels: number` and `last_interaction_at: string | null` fields [owner:api-engineer] [beads:nv-281a]
- [x] [2.2] [P-1] Update `apps/dashboard/app/api/diary/route.ts` to compute `distinct_channels` (count unique `channel` values) and `last_interaction_at` (max `createdAt` ISO string) from query results and include in response [owner:api-engineer] [beads:nv-9apr]

## UI Batch

- [x] [3.1] [P-1] Rewrite `apps/dashboard/components/DiaryEntry.tsx` as compact expandable row: monospace `HH:MM:SS` timestamp, channel icon via `getPlatformColor()`, trigger badge, truncated summary -- click toggles expanded view with tool pills, code block content, and metadata [owner:ui-engineer] [beads:nv-irwd]
- [x] [3.2] [P-1] Update `apps/dashboard/app/diary/page.tsx` stats bar: remove `computeStats` function, remove Tokens and Avg Latency `StatCard`s, replace with Entries (from `data.total`), Channels (from `data.distinct_channels`), Last Activity (relative time from `data.last_interaction_at`) [owner:ui-engineer] [beads:nv-4i8s]
- [x] [3.3] [P-1] Update `apps/dashboard/app/diary/page.tsx` header: change title from "Interaction Diary" to "Activity Log", subtitle to "Nova's interaction history" [owner:ui-engineer] [beads:nv-t6kd]
- [x] [3.4] [P-1] Add day header component above entry list showing contextual label ("Today", "Yesterday", or full formatted date) with date subtitle [owner:ui-engineer] [beads:nv-evwx]
- [x] [3.5] [P-2] Remove unused `Zap` and `Clock` icon imports from `page.tsx`, remove `computeStats` helper, clean up dead code from metric removal [owner:ui-engineer] [beads:nv-nvnn]
