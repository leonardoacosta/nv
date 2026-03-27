# Implementation Tasks
<!-- beads:epic:TBD -->

## API Batch

- [ ] [1.1] [P-1] Create `apps/dashboard/app/api/activity-feed/route.ts` -- new GET endpoint that queries last 24h from `messages`, `obligations`, and `diary` tables using Drizzle `gte(createdAt, twentyFourHoursAgo)` on each, maps rows to `ActivityFeedEvent` shape (`id`, `type`, `timestamp`, `icon_hint`, `summary`), merges all three arrays, sorts by timestamp desc, returns top 50 [owner:api-engineer]
- [ ] [1.2] [P-1] Add `ActivityFeedEvent` and `ActivityFeedGetResponse` types to `apps/dashboard/types/api.ts` -- `ActivityFeedEvent: { id: string; type: "message" | "obligation" | "diary"; timestamp: string; icon_hint: string; summary: string }`, `ActivityFeedGetResponse: { events: ActivityFeedEvent[] }` [owner:api-engineer]
- [ ] [1.3] [P-2] Add POST handler to `apps/dashboard/app/api/obligations/route.ts` -- accepts `{ detected_action: string, owner: string, status: string, priority: number, source_channel: string }`, inserts via Drizzle `db.insert(obligations).values(...)`, returns `{ obligation: { id } }` with 201 status; validates `detected_action` is non-empty string [owner:api-engineer]

## UI Batch

- [ ] [2.1] [P-1] Rewrite `apps/dashboard/app/page.tsx` -- remove all stat card rendering (StatCard imports, Operational/Performance rows, skeleton grid), remove greeting banner (`getGreeting()`, `todayDate`, `briefingSummary` in PageShell title/subtitle), remove SessionWidget and ActiveSession sections, remove ObligationsSidebar and Session Breakdown sidebar; replace PageShell title with "Command Center", keep `action` slot with auto-refresh toggle and Updated timestamp [owner:ui-engineer]
- [ ] [2.2] [P-1] Add PriorityBanner component inline in `page.tsx` -- renders at top of content area; shows amber banner when pending obligations > 0 ("N obligations need attention"), blue banner when briefing exists and no pending obligations ("Briefing available"), hidden when neither condition; single line with icon, text, and link to /obligations or /briefing [owner:ui-engineer]
- [ ] [2.3] [P-1] Add ActivityFeed component in `page.tsx` -- fetches `GET /api/activity-feed` on mount and on auto-refresh interval; renders dense table rows: monospace timestamp (`text-xs font-mono`), event type icon (MessageSquare for message, CheckSquare for obligation, BookOpen for diary), one-line summary (`text-sm truncate`); rows use `py-1.5` padding with `border-b border-ds-gray-400` separators; preserves existing `useDaemonEvents` WebSocket subscription to prepend real-time events [owner:ui-engineer]
- [ ] [2.4] [P-1] Add QuickActions component in `page.tsx` -- four action rows separated by subtle borders: (1) "Message Nova" link to /chat with MessageSquare icon, (2) "Create Obligation" with inline text input + submit button that POSTs to /api/obligations, clears on success, shows error inline, (3) "View Briefing" link to /briefing with preview text if briefing exists, (4) "Fleet Health" one-liner showing DB status from /api/server-health with green/amber/red dot [owner:ui-engineer]
- [ ] [2.5] [P-2] Add RecentConversations component in `page.tsx` -- fetches `GET /api/messages?limit=10`, groups consecutive messages by sender, renders last 5 groups as compact rows: sender name, channel badge (mono text), timestamp, content preview (120 char truncate); each group links to /messages; full-width below the two-column layout [owner:ui-engineer]
- [ ] [2.6] [P-2] Apply edge-to-edge layout -- override PageShell's `max-w-6xl mx-auto` for the home page by adding a wrapper that fills available width; use `grid grid-cols-1 lg:grid-cols-5 gap-6` with feed at `lg:col-span-3` and quick actions at `lg:col-span-2`; remove outer spacing gutters on the content grid [owner:ui-engineer]
- [ ] [2.7] [P-2] Preserve daemon disconnect behavior -- keep `useDaemonStatus()` check and apply `opacity-50` to the entire content area when `isDisconnected` is true; keep auto-refresh toggle disabled state when disconnected [owner:ui-engineer]

## Verify

- [ ] [3.1] `cd apps/dashboard && pnpm typecheck` passes -- zero TypeScript errors [owner:ui-engineer]
- [ ] [3.2] `cd apps/dashboard && pnpm build` passes -- production build succeeds [owner:ui-engineer]
- [ ] [3.3] [user] Manual test: dashboard home shows Priority Banner when obligations exist, hidden when none [owner:ui-engineer]
- [ ] [3.4] [user] Manual test: Activity Feed shows merged events from messages + obligations + diary tables [owner:ui-engineer]
- [ ] [3.5] [user] Manual test: "Create Obligation" inline input submits and creates an obligation [owner:ui-engineer]
- [ ] [3.6] [user] Manual test: Quick Actions panel shows fleet health status and briefing preview [owner:ui-engineer]
- [ ] [3.7] [user] Manual test: layout is edge-to-edge with no card gutters, monospace timestamps, dense rows [owner:ui-engineer]
