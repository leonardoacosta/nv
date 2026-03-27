# Proposal: Cleanup Diary Page

## Change ID
`cleanup-diary-page`

## Summary
Remove developer-facing metrics from the Diary page, add day-based entry grouping with Vercel-style
compact rows, and make entries scannable with expandable detail.

## Context
- Extends: `apps/dashboard/app/diary/page.tsx`, `apps/dashboard/components/DiaryEntry.tsx`
- Extends: `apps/dashboard/app/api/diary/route.ts` (add channel count + last interaction)
- Extends: `apps/dashboard/types/api.ts` (response type additions)
- Related: `enrich-diary-narratives` (archived) -- improved `result_summary` to narrative sentences
- Related: `add-interaction-diary` (archived) -- established the diary system
- Related: `global-density-pass` (active) -- dashboard-wide density alignment

## Motivation
The Diary page currently serves as a developer debug log rather than a user-facing activity record.
Token counts and average latency are meaningless to end users. Entries dump full content inline
without clear visual boundaries, making the page impossible to scan. There is no day grouping despite
the date navigation already being day-scoped. The title "Interaction Diary" is unnecessarily verbose.

The page should read like Vercel's deployment list: compact rows with monospace timestamps, source
icons, one-line summaries, and collapsible detail -- organized under day headers.

## Requirements

### Req-1: Remove Developer Metrics from Stats Bar
Remove "Tokens" and "Avg Latency" stats from the summary bar. Replace with user-meaningful metrics:
entries today count, distinct active channels count, and last interaction relative timestamp.

### Req-2: Compact Entry Rows with Expandable Detail
Replace the current card-per-entry layout with compact rows showing: `[HH:MM:SS]` monospace
timestamp, channel source icon (color-coded via `PLATFORM_BRAND`), trigger type badge, and the
one-line `result_summary`. Clicking a row expands it to reveal tool pills, full content in a
collapsible code block, and the latency/token metadata (moved here from the top-level display).

### Req-3: Day Grouping with Date Headers
Group entries under sticky date headers (e.g. "Today", "Yesterday", "Monday, March 24, 2026").
The existing date navigation selects a single day, so the header acts as a contextual label at the
top of each day's entries. When multi-day support is added later, this pattern extends naturally.

### Req-4: Collapsible Raw Content
Move `result_summary` full text (which may contain multi-line prompt/response content) into a
collapsible section within the expanded entry row. Render in a monospace code block with
`overflow-x-auto` for long lines. Developer metadata (tokens, latency) lives here too.

### Req-5: Simplify Page Title
Change "Interaction Diary" to "Activity Log". Update the subtitle to "Nova's interaction history".

### Req-6: API Response Enhancement
Add `distinct_channels` (count of unique `channel` values for the day) and `last_interaction_at`
(ISO timestamp of the most recent entry) to the `DiaryGetResponse` type, computed server-side in
the API route.

## Scope
- **IN**: Stats bar metric replacement, compact row layout, expand/collapse entry detail, day header,
  title rename, API response enhancement for new stat fields, collapsible code block for raw content
- **OUT**: Multi-day view (single-day navigation preserved), pagination/infinite scroll, search/filter,
  diary schema changes, daemon-side changes, new database columns

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/diary/page.tsx` | Replace stats computation, add day header, update title |
| `apps/dashboard/components/DiaryEntry.tsx` | Rewrite as compact row with expand/collapse |
| `apps/dashboard/app/api/diary/route.ts` | Add `distinct_channels` and `last_interaction_at` to response |
| `apps/dashboard/types/api.ts` | Extend `DiaryGetResponse` with two new fields |

## Risks
| Risk | Mitigation |
|------|-----------|
| Expand/collapse state loss on re-render | Use local component state per entry, keyed by entry time+index |
| Long `result_summary` text overflows compact row | Truncate to single line with `truncate` class, full text in expanded view |
| `global-density-pass` spec touches dashboard-wide spacing | This spec is scoped to diary-only components; density tokens are compatible |
