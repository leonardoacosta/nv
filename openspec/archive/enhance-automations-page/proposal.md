# Proposal: Enhance Automations Page

## Change ID
`enhance-automations-page`

## Summary
Add editable custom prompts, configurable briefing schedule, cross-page navigation links, and UI-driven reminder creation to the Automations page. Rename the Telegram `/brief` command to `/snapshot` to disambiguate from the full daily briefing.

## Context
- Extends: `apps/dashboard/app/automations/page.tsx`, `apps/dashboard/app/api/automations/`, `apps/dashboard/types/api.ts`, `crates/nv-daemon/src/scheduler.rs`, `crates/nv-daemon/src/proactive_watcher.rs`, `crates/nv-core/src/config.rs`, `packages/daemon/src/telegram/commands/brief.ts`, `packages/daemon/src/channels/telegram.ts`, `packages/db/src/schema/`, `apps/dashboard/app/sessions/page.tsx`
- Related: `add-automations-page` (8/10 tasks, base page built), `add-schedule-svc` (schedule CRUD), `add-briefing-cron` (morning briefing pipeline)
- Existing `/status` Telegram command does daemon uptime + fleet health -- no collision with `/snapshot`

## Motivation
The Automations page (built in `add-automations-page`) provides visibility into Nova's autonomous processes but lacks configuration controls. Users cannot customize what the watcher monitors or what the briefing emphasizes, cannot change the hardcoded 7 AM briefing time, cannot create reminders from the dashboard, and must navigate blindly between related pages. Additionally, the Telegram `/brief` command name is overloaded with the full daily briefing concept, causing confusion.

## Requirements

### Req-1: Settings table for runtime-configurable automation settings
Add a `settings` table in `@nova/db` with key-value pairs for automation config. Store `watcher_prompt`, `briefing_prompt`, and `briefing_hour` as named settings. Expose via `GET /api/automations/settings` and `PUT /api/automations/settings`. The Rust daemon reads `briefing_hour` from the DB instead of the `MORNING_BRIEFING_HOUR` constant, falling back to 7 when no row exists.

### Req-2: Editable prompt textareas for watcher and briefing
Add collapsible prompt textarea sections to both the WatcherCard and BriefingCard components. Each textarea loads the current prompt from the settings API, auto-saves on blur via `PUT /api/automations/settings`, and shows a saving/saved indicator. Default placeholder text describes what each prompt customizes.

### Req-3: Configurable briefing schedule time picker
Add a time picker (hour selector, 0-23) to the BriefingCard that reads/writes `briefing_hour` from the settings table. Update the `GET /api/automations` response to compute `next_generation` from the DB setting instead of hardcoded 7. Update `crates/nv-daemon/src/scheduler.rs` to read `briefing_hour` from the Postgres settings table on each poll tick instead of the `MORNING_BRIEFING_HOUR` constant.

### Req-4: Cross-page navigation links
Add navigation buttons to the automations page: "View All Briefings" linking to `/briefing`, "View Watcher Sessions" linking to `/sessions?command=proactive-followup`, "View Briefing Sessions" linking to `/sessions?command=morning-briefing`. The sessions page uses `command` field values (not a `trigger` column) since sessions store command names like `"proactive-followup"` and `"morning-briefing"`.

### Req-5: Sessions page command filter support
Add `?command=<value>` query param parsing to the sessions page. When present, pre-filter the session list to only show sessions whose `command` field matches the param value. Integrate with the existing `statusFilter` and `projectFilter` UI. Show an active filter chip that can be dismissed.

### Req-6: Dashboard reminder creation
Add a "Create Reminder" button to the RemindersTab that opens an inline form with message input (text), date/time picker, and channel selector (default: "dashboard"). Submit via `POST /api/automations/reminders` which inserts into the `reminders` table. Refresh the reminders list on success.

### Req-7: Rename Telegram /brief command to /snapshot
Rename `packages/daemon/src/telegram/commands/brief.ts` to `snapshot.ts`. Update the exported function from `buildBriefReply` to `buildSnapshotReply`. Update all references in `packages/daemon/src/channels/telegram.ts` (import, switch case, onText regex). Update `help.ts` and `start.ts` command references. The command's behavior (calendar + mail + obligations quick view) remains identical.

## Scope
- **IN**: Settings table + API, prompt textareas, briefing hour picker, nav links, sessions command filter, reminder creation form, Telegram /brief -> /snapshot rename
- **OUT**: Real-time WebSocket updates, watcher prompt injection into the Rust daemon's LLM calls (future spec -- this only stores the prompt), briefing prompt injection into the synthesizer (future spec), schedule CRUD from dashboard (already covered by schedule-svc), reminder editing/snooze, session detail cross-linking

## Impact

| Area | Change |
|------|--------|
| `packages/db/src/schema/settings.ts` | New: `settings` table (key TEXT PK, value TEXT, updated_at TIMESTAMP) |
| `packages/db/drizzle/` | New migration for settings table |
| `apps/dashboard/app/api/automations/settings/route.ts` | New: GET + PUT for automation settings |
| `apps/dashboard/app/api/automations/reminders/route.ts` | New: POST for reminder creation |
| `apps/dashboard/app/api/automations/route.ts` | Modified: read briefing_hour from settings table |
| `apps/dashboard/app/automations/page.tsx` | Modified: prompt textareas, hour picker, nav links, reminder form |
| `apps/dashboard/types/api.ts` | Modified: add settings types, reminder creation types |
| `apps/dashboard/app/sessions/page.tsx` | Modified: command query param filter |
| `crates/nv-daemon/src/scheduler.rs` | Modified: read briefing_hour from DB instead of const |
| `packages/daemon/src/telegram/commands/brief.ts` | Renamed to `snapshot.ts`, function renamed |
| `packages/daemon/src/telegram/commands/help.ts` | Modified: /brief -> /snapshot |
| `packages/daemon/src/telegram/commands/start.ts` | Modified: callback_data cmd:brief -> cmd:snapshot |
| `packages/daemon/src/channels/telegram.ts` | Modified: import + switch case + regex for snapshot |

## Risks

| Risk | Mitigation |
|------|-----------|
| Rust daemon DB read on every scheduler tick adds latency | Query is a single-row SELECT by PK on a tiny table. Add a 60s in-memory cache in the scheduler to avoid per-tick queries. Fall back to 7 if query fails. |
| Settings table could grow unbounded with arbitrary keys | Constrain to known keys via validation in the API route. Only accept `watcher_prompt`, `briefing_prompt`, `briefing_hour`. |
| Telegram /brief -> /snapshot breaks muscle memory | One-time rename. Help text and start keyboard update simultaneously. Low user count (single user). |
| Reminder creation from dashboard bypasses NLP parsing | Dashboard form has explicit fields (message, date, channel) so NLP is unnecessary. Same DB insert path as the Rust daemon's set_reminder tool. |
| Sessions command filter may not match all watcher/briefing sessions | Command values are deterministic strings set in worker.rs (`"proactive-followup"`, `"morning-briefing"`). Filter uses exact match. |
