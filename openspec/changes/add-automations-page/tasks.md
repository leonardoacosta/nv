# Implementation Tasks

<!-- beads:epic:TBD -->

## DB Batch

(No schema changes -- reminders, schedules, sessions, and briefings tables already exist in @nova/db)

## API Batch

- [ ] [2.1] Add `GET /api/automations` handler in `crates/nv-daemon/src/http.rs`. Query `reminders` (non-cancelled, non-delivered, order by `due_at` asc), `schedules` (order by `name` asc), `sessions` (where `status = 'running'`, order by `started_at` desc), `briefings` (latest by `generated_at`, limit 1). Read `[proactive_watcher]` config from `HttpState`. Compute `next_run` for schedules using a cron parser crate (`cron` or `croner`). Compute reminder `status` as `"overdue"` if `due_at < now()`, else `"pending"`. Compute briefing `next_generation` as next 7:00 AM after `last_generated_at`. Return aggregated JSON matching `AutomationsGetResponse` shape. Register route in `build_router`. [owner:api-engineer]
- [ ] [2.2] Add `PATCH /api/automations/reminders/{id}` handler in `crates/nv-daemon/src/http.rs`. Accept `{ "action": "cancel" }` body, set `cancelled = true` on the reminders row, return updated reminder. Register route in `build_router`. [owner:api-engineer]
- [ ] [2.3] Add `PATCH /api/automations/schedules/{id}` handler in `crates/nv-daemon/src/http.rs`. Accept `{ "enabled": bool }` body, update `enabled` field on the schedules row, return updated schedule. Register route in `build_router`. [owner:api-engineer]

## UI Batch

- [ ] [3.1] Add automation response types to `dashboard/src/types/api.ts`: `AutomationReminder`, `AutomationSchedule`, `AutomationWatcher`, `AutomationBriefing`, `AutomationSession`, `AutomationsGetResponse`. [owner:ui-engineer]
- [ ] [3.2] Create `dashboard/src/pages/AutomationsPage.tsx` with 5 sections -- Active Reminders table (message, due, channel, status, cancel action), Scheduled Jobs table (name, human-readable cron, last run, next run, enabled status, pause/resume toggle), Proactive Watcher row (interval, quiet hours, enabled), Briefing Schedule row (last generated, next generation), Active Sessions table (project, command, status, started). Include inline cron-to-human-readable renderer for common patterns (`0 7 * * *` -> `"Every day at 7:00 AM"`, `*/30 * * * *` -> `"Every 30 minutes"`, fallback to raw). Loading skeleton, error state with retry, empty state per section. Auto-refresh every 30s via `setInterval` + re-fetch. Quick actions: cancel reminder via `PATCH /api/automations/reminders/:id`, toggle schedule via `PATCH /api/automations/schedules/:id`. [owner:ui-engineer]
- [ ] [3.3] Update `dashboard/src/components/Sidebar.tsx` -- add `Timer` to lucide-react imports, add `{ to: "/automations", label: "Automations", icon: Timer }` to `NAV_ITEMS` after the Settings entry. [owner:ui-engineer]
- [ ] [3.4] Update `dashboard/src/App.tsx` -- import `AutomationsPage`, add `<Route path="/automations" element={<AutomationsPage />} />`. [owner:ui-engineer]

## E2E Batch

- [ ] [4.1] Verify dashboard builds cleanly: `pnpm typecheck` passes with new page, types, sidebar update, and route. [owner:e2e-engineer]
- [ ] [4.2] Verify `GET /api/automations` returns 200 with valid JSON containing all 5 sections (reminders, schedules, watcher, briefing, active_sessions). [owner:e2e-engineer]
- [ ] [4.3] Verify `PATCH /api/automations/reminders/{id}` with `{ "action": "cancel" }` sets cancelled to true. Verify `PATCH /api/automations/schedules/{id}` with `{ "enabled": false }` sets enabled to false. [owner:e2e-engineer]
