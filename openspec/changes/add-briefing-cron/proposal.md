# Proposal: Add Briefing Cron

## Change ID
`add-briefing-cron`

## Summary

Upgrade the morning briefing pipeline from a DB-only synthesizer to a full-context daily newsletter. The existing scheduler and synthesizer already gather obligations, memory, and messages from Postgres; this spec adds fleet service data sources (calendar via graph-svc, diary entries, messages-svc for channel-grouped summaries), sends the finished briefing to Leo via Telegram, adds a "Generate Now" button to the dashboard, and enriches the dashboard rendering with proper markdown.

## Context
- Extends: `packages/daemon/src/features/briefing/` (scheduler.ts, synthesizer.ts, runner.ts)
- Extends: `apps/dashboard/app/briefing/page.tsx`, `apps/dashboard/app/api/briefing/`
- DB schema: `packages/db/src/schema/briefings.ts` -- briefings table already exists (id, generated_at, content, sources_status, suggested_actions)
- Fleet services: graph-svc (:4107) `/calendar/today`, messages-svc (:4102) `/recent`, memory-svc (:4101) `/read`
- Diary reader: `packages/daemon/src/features/diary/reader.ts` -- `getEntriesByDate()` returns overnight entries
- Telegram: `TelegramAdapter.sendMessage()` in `packages/daemon/src/channels/telegram.ts`
- Agent SDK: `query()` from `@anthropic-ai/claude-agent-sdk` (already used in synthesizer.ts)
- Prior art: archived spec `2026-03-26-add-morning-briefing` covered the initial port; this spec completes data gathering + delivery

## Motivation

The briefing system generates content but it is incomplete: it queries obligations, memory, and messages directly from Postgres but misses today's calendar events, overnight diary entries, and channel-grouped message summaries. The generated briefing is never sent to Telegram, so Leo only sees it if he opens the dashboard. The dashboard page has no way to trigger an immediate briefing and renders content as plain whitespace-preserved text instead of rich markdown. This spec fills all four gaps.

## Requirements

### Req-1: Expand data gathering with fleet services and diary

The synthesizer's `gatherContext()` must fetch two additional data sources in its `Promise.allSettled` call:

1. **Calendar today** -- HTTP GET to graph-svc at `http://localhost:4107/calendar/today`. Returns `{ result: string }` with today's calendar events as formatted text. Record as `sources_status.calendar`.
2. **Overnight diary entries** -- Call `getEntriesByDate(yesterdayDate)` from the diary reader to get entries from the previous night (entries after ~18:00 yesterday through this morning). Record as `sources_status.diary`.

Each new source gets the same 10-second timeout and partial-result handling as the existing sources. The `GatheredContext` interface gains `calendar: string | null` and `diaryEntries: DiaryEntryItem[]` fields.

### Req-2: Enrich the synthesis prompt

Update `BRIEFING_SYSTEM_PROMPT` and `buildBriefingPrompt()` to include:

1. **Unread/new messages summary** -- Group existing messages by channel (telegram, teams, discord) with counts and sender highlights.
2. **Pending obligations with priority** -- Already present; no change needed.
3. **Today's calendar events** -- Inject the calendar text from graph-svc.
4. **Overnight Nova activity** -- Summarise diary entries (tools used, interaction count, channels active).
5. **Memory updates** -- Already present; no change needed.
6. **Suggested actions for the day** -- Already present via JSON block parsing; no change needed.

Update the system prompt section list to match the six briefing sections.

### Req-3: Send briefing to Telegram after generation

After `runMorningBriefing` persists the briefing to the database, send the content to Leo via Telegram:

1. The runner needs access to `TelegramAdapter` and `telegramChatId` from config.
2. Extend `BriefingDeps` to include `telegram: TelegramAdapter | null` and `telegramChatId: string | null`.
3. After DB insert, if both `telegram` and `telegramChatId` are available, send the briefing content as a Markdown message with `disablePreview: true`.
4. Truncate to Telegram's 4096-character limit if necessary (split into multiple messages or truncate with a "... [view full briefing on dashboard]" suffix).
5. Telegram delivery failure must not fail the overall briefing -- catch and log.

### Req-4: Dashboard "Generate Now" button

1. **New API route**: `POST /api/briefing/generate` in the dashboard.
   - Calls the daemon HTTP endpoint `POST /briefing/generate` (new endpoint on the daemon).
   - Returns `200 { success: true, briefing_id: string }` on success.
   - Returns `503` if the daemon is unreachable.

2. **New daemon HTTP endpoint**: `POST /briefing/generate` on the daemon's Hono app.
   - Calls `runMorningBriefing(deps)` directly (same as the scheduler trigger).
   - Returns `200 { id, generated_at }` on success, `500` on error.
   - Requires the same `BriefingDeps` available at HTTP handler scope.

3. **Dashboard UI**: Add a "Generate Now" button next to the existing Refresh button.
   - Uses a `Zap` icon from Lucide.
   - Shows loading state during generation (disable button, show spinner).
   - On success, refreshes the page data to show the new briefing.
   - On error, shows an error banner.

### Req-5: Rich markdown rendering on dashboard

Replace the plain `whitespace-pre-wrap` text rendering in `BriefingSectionCard` with proper markdown rendering:

1. Add `react-markdown` as a dependency to the dashboard.
2. Render `section.body` through `<ReactMarkdown>` with tailwind prose classes.
3. Support: headings, bold, italic, lists, code blocks, links.
4. The section parser (`parseBriefingSections`) already splits by `###` headers -- markdown within each section body should render correctly.

### Req-6: Show next scheduled briefing time

Add a subtle line below the page subtitle showing "Next briefing: Tomorrow at 7:00 AM" (or "Today at 7:00 AM" if before 7am). This is a client-side calculation, not an API call.

## Scope

**IN**: Synthesizer data gathering expansion (calendar, diary), enriched prompt, Telegram delivery, daemon `POST /briefing/generate` endpoint, dashboard `POST /api/briefing/generate` route, "Generate Now" button, react-markdown rendering, next-briefing-time display.

**OUT**: Customisable briefing sections, notification preferences, email delivery, action completion/dismissal API, configurable briefing hour via API, briefing scheduling via the schedules table (remains `setInterval` poll).

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/features/briefing/synthesizer.ts` | Add calendar + diary sources to `gatherContext()`, expand `GatheredContext`, update prompt |
| `packages/daemon/src/features/briefing/runner.ts` | Add Telegram delivery after DB insert |
| `packages/daemon/src/features/briefing/scheduler.ts` | Update `BriefingDeps` import (no logic change) |
| `packages/daemon/src/http.ts` | Add `POST /briefing/generate` route |
| `packages/daemon/src/index.ts` | Pass telegram + chatId into BriefingDeps, pass deps to HTTP app |
| `apps/dashboard/app/api/briefing/generate/route.ts` | New: proxy to daemon POST endpoint |
| `apps/dashboard/app/briefing/page.tsx` | Add "Generate Now" button, next-briefing-time, react-markdown rendering |
| `apps/dashboard/package.json` | Add `react-markdown` dependency |

## Risks

| Risk | Mitigation |
|------|-----------|
| graph-svc SSH tunnel to CloudPC may be down at 7am | Calendar source records "unavailable" in sources_status; briefing continues without calendar. Static fallback already handles missing sources. |
| Telegram 4096-char limit exceeded by long briefings | Truncate with dashboard link suffix. Agent SDK prompt already says "under 500 words" (~2500 chars). |
| "Generate Now" spammed by user | No rate limiting in this spec (acceptable for single-user system). If needed, add a 60-second cooldown in a follow-up. |
| react-markdown bundle size | ~15KB gzipped; acceptable for the dashboard which already loads Lucide icons and other heavy deps. |
| Diary reader import pulls in @nova/db Drizzle client | The daemon already depends on @nova/db transitively through other features (obligations). No new dependency chain. |
