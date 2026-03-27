# Proposal: Add Automations Page

## Change ID
`add-automations-page`

## Summary

Add a `/automations` page to the dashboard showing everything Nova does autonomously -- active
reminders, scheduled jobs, proactive watcher status, briefing schedule, and active sessions. A new
`GET /api/automations` endpoint on the Rust daemon aggregates data from the reminders, schedules,
and sessions DB tables plus watcher/briefing config from `nv.toml`.

## Context
- Phase: Dashboard feature (post-Wave 1 schema)
- Dependencies: reminders, schedules, sessions tables in `@nova/db` (created in Wave 1)
- Key sources:
  - `packages/db/src/schema/reminders.ts` -- `reminders` table (id, message, dueAt, channel, deliveredAt, cancelled)
  - `packages/db/src/schema/schedules.ts` -- `schedules` table (id, name, cronExpr, action, channel, enabled, lastRunAt)
  - `packages/db/src/schema/sessions.ts` -- `sessions` table (id, project, command, status, startedAt, stoppedAt)
  - `packages/db/src/schema/briefings.ts` -- `briefings` table (id, generatedAt, content, sourcesStatus, suggestedActions)
  - `config/nv.toml` -- `[proactive_watcher]` section (interval_minutes, quiet_start, quiet_end, enabled)
  - `crates/nv-daemon/src/http.rs` -- Axum router with existing `/api/*` pattern
  - `dashboard/src/components/Sidebar.tsx` -- flat `NAV_ITEMS` array, lucide-react icons
  - `dashboard/src/App.tsx` -- React Router routes
  - `dashboard/vite.config.ts` -- Vite proxy `/api` to daemon at `127.0.0.1:3443`

## Motivation

Nova runs multiple autonomous processes -- reminders, cron-scheduled jobs, a proactive watcher,
morning briefings, and work sessions. Today there is no single view showing what is running
automatically. To understand what Nova is doing, Leo must dig through Telegram logs or check
individual pages. An automations page provides:

1. **Visibility** -- see all autonomous activity in one place without leaving the dashboard.
2. **Control** -- quick actions (pause/resume/cancel) on reminders and schedules without issuing
   Telegram commands.
3. **Debugging** -- last-run timestamps and next-run projections surface stale or stuck automations.

## Requirements

### Req-1: GET /api/automations Endpoint

Add a new Axum handler in `crates/nv-daemon/src/http.rs`:

- Query `reminders` table: select all non-cancelled, non-delivered reminders ordered by `due_at` asc.
- Query `schedules` table: select all schedules ordered by `name` asc.
- Query `sessions` table: select where `status = 'running'` ordered by `started_at` desc.
- Query `briefings` table: select latest entry (order by `generated_at` desc, limit 1) to derive
  last generated time.
- Read `[proactive_watcher]` config from the loaded config (already available in `HttpState`) to
  derive interval, quiet hours, and enabled status.
- Compute `next_run` for schedules from `cron_expr` + `last_run_at` using a cron parser crate.
- Compute `next_run` for briefing from config (7:00 AM daily, skip if last generated today).

Response shape:

```json
{
  "reminders": [
    {
      "id": "uuid",
      "message": "Follow up on PR review",
      "due_at": "2026-03-26T14:00:00Z",
      "channel": "telegram",
      "created_at": "2026-03-26T10:00:00Z",
      "status": "pending"
    }
  ],
  "schedules": [
    {
      "id": "uuid",
      "name": "morning-briefing",
      "cron_expr": "0 7 * * *",
      "action": "generate_briefing",
      "channel": "telegram",
      "enabled": true,
      "last_run_at": "2026-03-26T07:00:00Z",
      "next_run": "2026-03-27T07:00:00Z"
    }
  ],
  "watcher": {
    "enabled": true,
    "interval_minutes": 30,
    "quiet_start": "22:00",
    "quiet_end": "07:00",
    "last_run_at": null
  },
  "briefing": {
    "last_generated_at": "2026-03-26T07:00:00Z",
    "next_generation": "2026-03-27T07:00:00Z"
  },
  "active_sessions": [
    {
      "id": "uuid",
      "project": "oo",
      "command": "pnpm dev",
      "status": "running",
      "started_at": "2026-03-26T09:00:00Z"
    }
  ]
}
```

### Req-2: PATCH /api/automations/reminders/:id Endpoint

Add endpoint to update reminder status:
- `PATCH /api/automations/reminders/{id}` with body `{ "action": "cancel" }`.
- Sets `cancelled = true` on the reminder row.
- Returns the updated reminder.

### Req-3: PATCH /api/automations/schedules/:id Endpoint

Add endpoint to toggle schedule enabled/disabled:
- `PATCH /api/automations/schedules/{id}` with body `{ "enabled": true|false }`.
- Updates the `enabled` field on the schedule row.
- Returns the updated schedule.

### Req-4: Dashboard TypeScript Types

Add to `dashboard/src/types/api.ts`:

```typescript
// -- GET /api/automations
export interface AutomationReminder {
  id: string;
  message: string;
  due_at: string;
  channel: string;
  created_at: string;
  status: "pending" | "overdue";
}

export interface AutomationSchedule {
  id: string;
  name: string;
  cron_expr: string;
  action: string;
  channel: string;
  enabled: boolean;
  last_run_at: string | null;
  next_run: string | null;
}

export interface AutomationWatcher {
  enabled: boolean;
  interval_minutes: number;
  quiet_start: string;
  quiet_end: string;
  last_run_at: string | null;
}

export interface AutomationBriefing {
  last_generated_at: string | null;
  next_generation: string | null;
}

export interface AutomationSession {
  id: string;
  project: string;
  command: string;
  status: string;
  started_at: string;
}

export interface AutomationsGetResponse {
  reminders: AutomationReminder[];
  schedules: AutomationSchedule[];
  watcher: AutomationWatcher;
  briefing: AutomationBriefing;
  active_sessions: AutomationSession[];
}
```

### Req-5: Automations Page Component

New file: `dashboard/src/pages/AutomationsPage.tsx`:

- On mount: fetch `GET /api/automations` and populate state.
- Layout: dense table/list format, no cards. Sections stacked vertically:
  1. **Active Reminders** -- table with columns: Message, Due, Channel, Status, Actions (cancel button).
  2. **Scheduled Jobs** -- table with columns: Name, Schedule (cron rendered human-readable), Last Run,
     Next Run, Status (enabled/disabled), Actions (pause/resume toggle).
  3. **Proactive Watcher** -- single row showing interval, quiet hours, enabled status.
  4. **Briefing Schedule** -- single row showing next generation time and last generated.
  5. **Active Sessions** -- table with columns: Project, Command, Status, Started.
- Cron rendering: convert cron expressions to human-readable strings inline (e.g., `"0 7 * * *"` ->
  `"Every day at 7:00 AM"`, `"*/30 * * * *"` -> `"Every 30 minutes"`). Simple switch/regex for
  common patterns, fallback to raw expression.
- Quick actions:
  - Cancel reminder: `PATCH /api/automations/reminders/:id` with `{ "action": "cancel" }`.
  - Pause/resume schedule: `PATCH /api/automations/schedules/:id` with `{ "enabled": false|true }`.
- Loading state: skeleton rows matching the table layout.
- Error state: retry-able error banner.
- Empty state per section: "No active reminders", "No scheduled jobs", etc.
- Auto-refresh every 30 seconds.

### Req-6: Sidebar Navigation Update

In `dashboard/src/components/Sidebar.tsx`:
- Add `Timer` to lucide-react imports.
- Add `{ to: "/automations", label: "Automations", icon: Timer }` to `NAV_ITEMS`, positioned after
  Settings (last item, system-level page).

### Req-7: Router Update

In `dashboard/src/App.tsx`:
- Import `AutomationsPage`.
- Add `<Route path="/automations" element={<AutomationsPage />} />`.

## Scope

**IN**: Automations page component, GET /api/automations endpoint, PATCH endpoints for
reminders and schedules, sidebar nav entry, router entry, TypeScript types, cron expression
human-readable display.

**OUT**: Creating/editing automations from the dashboard (use Telegram commands), watcher
configuration UI (edit quiet hours/interval), session start/stop controls, briefing
regeneration trigger, real-time WebSocket updates.

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/http.rs` | Add `GET /api/automations`, `PATCH /api/automations/reminders/{id}`, `PATCH /api/automations/schedules/{id}` routes + handlers |
| `dashboard/src/pages/AutomationsPage.tsx` | New: automations page with 5 sections |
| `dashboard/src/types/api.ts` | Add automation response types |
| `dashboard/src/components/Sidebar.tsx` | Add "Automations" nav entry with Timer icon |
| `dashboard/src/App.tsx` | Add `/automations` route |

## Risks

| Risk | Mitigation |
|------|-----------|
| Cron next-run computation requires a crate dependency | Use `cron` or `croner` crate which is lightweight and well-maintained. Only needed for `next_run` calculation -- if too heavy, return `null` for `next_run` and compute client-side. |
| Watcher `last_run_at` not persisted in DB | The watcher runs in the daemon process. Expose `last_run_at` via the loaded daemon state (in-memory). If the daemon restarts, this resets to null -- acceptable for v1. |
| Stale data with 30s polling interval | Automations are slow-changing (minutes/hours). 30s refresh is sufficient. Real-time updates deferred to a future WebSocket/SSE implementation. |
| Schedule toggle could conflict with daemon's schedule runner | The daemon reads `enabled` from DB before each run. Toggling `enabled` via PATCH takes effect on the next scheduled check. No race condition -- the DB is the source of truth. |
