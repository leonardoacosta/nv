# Implementation Tasks

<!-- beads:epic:nv-tzc2 -->

## DB Batch

- [x] [1.1] [P-1] [beads:nv-y5ot] Create `packages/db/src/schema/settings.ts` with `settings` pgTable (key TEXT PK, value TEXT NOT NULL, updatedAt TIMESTAMP WITH TZ NOT NULL DEFAULT NOW). Export `settings`, `Setting`, `NewSetting` types. Re-export from `packages/db/src/index.ts`. Run `pnpm drizzle-kit generate` to produce the migration. [owner:db-engineer]

## API Batch

- [x] [2.1] [P-1] [beads:nv-iw5a] Create `apps/dashboard/app/api/automations/settings/route.ts` with GET (return all settings as `{ settings: Record<string, string> }`) and PUT (accept `{ key, value }`, validate key is one of `watcher_prompt`, `briefing_prompt`, `briefing_hour`, upsert via INSERT ON CONFLICT UPDATE, return updated row). Invalid keys return 400. [owner:api-engineer]
- [x] [2.2] [P-1] [beads:nv-qwoi] Create `apps/dashboard/app/api/automations/reminders/route.ts` with POST handler. Accept `{ message: string, due_at: string, channel?: string }`. Validate message non-empty (max 500), due_at is valid future ISO 8601. Default channel to `"dashboard"`. Insert into reminders table. Return created reminder with status 201. [owner:api-engineer]
- [x] [2.3] [P-2] [beads:nv-4vuv] Update `apps/dashboard/app/api/automations/route.ts` to read `briefing_hour` from the settings table (SELECT where key = `briefing_hour`, parse as int, default 7). Use the configured hour to compute `next_generation` instead of hardcoded 7. Add `briefing_hour` to the briefing section of the response. [owner:api-engineer]
- [x] [2.4] [P-2] [beads:nv-ti1x] Update `apps/dashboard/types/api.ts`: extend `AutomationBriefing` with `briefing_hour: number`. Add `AutomationSettingsResponse` (`{ settings: Record<string, string> }`), `PutSettingRequest` (`{ key: string, value: string }`), and `CreateReminderRequest` (`{ message: string, due_at: string, channel?: string }`). [owner:api-engineer]

## UI Batch

- [x] [3.1] [P-1] [beads:nv-9fow] Add collapsible "Custom Prompt" textarea to WatcherCard. On mount, fetch `GET /api/automations/settings` and populate from `watcher_prompt` key. On blur, save via `PUT /api/automations/settings` with saving/saved indicator. Use placeholder: "Describe what the watcher should look for (e.g., overdue obligations, calendar conflicts)...". [owner:ui-engineer]
- [x] [3.2] [P-1] [beads:nv-trka] Add collapsible "Custom Prompt" textarea to BriefingCard. Same pattern as 3.1 but for `briefing_prompt` key. Placeholder: "Describe what the briefing should emphasize (e.g., today's meetings, urgent tasks)...". [owner:ui-engineer]
- [x] [3.3] [P-1] [beads:nv-afyb] Add hour picker (select, 0-23 displayed as 12h AM/PM) to BriefingCard. Load from settings API `briefing_hour` key (default 7). On change, save via PUT and optimistically update the displayed `next_generation` time. [owner:ui-engineer]
- [x] [3.4] [P-1] [beads:nv-a8m1] Add navigation links: "View All Briefings" button in BriefingCard linking to `/briefing` (next to existing "View Briefing" link), "View Watcher Sessions" link below WatcherCard linking to `/sessions?command=proactive-followup`, "View Briefing Sessions" link below BriefingCard linking to `/sessions?command=morning-briefing`. Use lucide ExternalLink icon. [owner:ui-engineer]
- [x] [3.5] [P-1] [beads:nv-4qpn] Add `?command=` query param filter to `apps/dashboard/app/sessions/page.tsx`. Read via `useSearchParams().get("command")`. When present, filter sessions by `agent_name` or `command` field match. Show a dismissible filter chip (X button) above the session list. Dismissing clears the URL param via `router.replace`. Combine with existing statusFilter and projectFilter. [owner:ui-engineer]
- [x] [3.6] [P-2] [beads:nv-wgl8] Add "Create Reminder" button + inline form to RemindersTab. Form fields: message textarea (required, max 500 chars, client validation), datetime-local input (required, must be future), channel text input (optional, default "dashboard"). Submit via POST /api/automations/reminders. On success, close form and trigger parent refetch. On error, show inline error. [owner:ui-engineer]

## Daemon Batch

- [x] [4.1] [P-1] [beads:nv-fu7a] Rename `packages/daemon/src/telegram/commands/brief.ts` to `snapshot.ts`. Rename export `buildBriefReply` to `buildSnapshotReply`. Update JSDoc to reference `/snapshot`. [owner:api-engineer]
- [x] [4.2] [P-1] [beads:nv-8tm3] Update `packages/daemon/src/channels/telegram.ts`: change import from `brief.js` to `snapshot.js`, rename `buildBriefReply` to `buildSnapshotReply`, update switch case from `"brief"` to `"snapshot"`, update onText regex from `/^\/brief(@\S+)?$/` to `/^\/snapshot(@\S+)?$/`. [owner:api-engineer]
- [x] [4.3] [P-1] [beads:nv-hcyr] Update `packages/daemon/src/telegram/commands/help.ts`: change `/brief` to `/snapshot` in help text. Update `packages/daemon/src/telegram/commands/start.ts`: change `callback_data` from `"cmd:brief"` to `"cmd:snapshot"`. [owner:api-engineer]
- [x] [4.4] [P-2] [beads:nv-h7cr] Update `crates/nv-daemon/src/scheduler.rs`: add a function `read_briefing_hour` that queries the Postgres settings table for key `"briefing_hour"`, parses as u32, caches for 60s, falls back to 7 on error. Replace `MORNING_BRIEFING_HOUR` usage in the morning briefing poll branch with the cached value. Keep the constant as the documented default. [owner:api-engineer]

## E2E Batch

- [x] [5.1] [beads:nv-4i1k] Verify dashboard builds cleanly: `pnpm typecheck` passes with all new/modified files. [owner:e2e-engineer]
- [ ] [5.2] [beads:nv-os6q] Verify `GET /api/automations/settings` returns 200. Verify `PUT /api/automations/settings` with valid key returns 200, with invalid key returns 400. [owner:e2e-engineer] [user]
- [ ] [5.3] [beads:nv-n56f] Verify `POST /api/automations/reminders` with valid body returns 201, with empty message returns 400, with past date returns 400. [owner:e2e-engineer] [user]
- [ ] [5.4] [beads:nv-iowp] Verify `/sessions?command=proactive-followup` filters session list and shows dismissible chip. [owner:e2e-engineer] [user]
- [ ] [5.5] [beads:nv-guxp] Verify Telegram `/snapshot` returns calendar + mail + obligations output. Verify `/brief` no longer matches as a direct command. [owner:e2e-engineer] [user]
