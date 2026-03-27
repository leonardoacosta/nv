# Implementation Tasks
<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] Add `content_preview` to briefing response in `apps/dashboard/app/api/automations/route.ts`. Select the `content` column from the latest briefing row (already queried). Strip markdown formatting, truncate to 200 chars, and return as `content_preview: string | null` in the `briefing` object. [owner:api-engineer]
- [x] [2.2] Create `PATCH /api/automations/watcher` endpoint at `apps/dashboard/app/api/automations/watcher/route.ts`. Accept partial body with optional fields: `enabled` (boolean), `interval_minutes` (number, 5-120), `quiet_start` (string, HH:MM), `quiet_end` (string, HH:MM). Validate input, update the watcher config source (env override or in-memory state), return the updated `AutomationWatcher` object. [owner:api-engineer]
- [x] [2.3] Update `AutomationBriefing` in `apps/dashboard/types/api.ts` -- add `content_preview: string | null` field. [owner:api-engineer]

## UI Batch

- [ ] [3.1] [P-1] Restructure page layout from vertical stack to 2-column grid. Top row: Watcher stat card (left) + Briefing stat card (right) using `grid grid-cols-1 md:grid-cols-2 gap-4`. Bottom row: full-width "Scheduled Automations" section with tab/segmented control switching between Reminders and Schedules views. Remove Active Sessions section entirely. Preserve existing loading skeleton and error state patterns. [owner:ui-engineer]
- [ ] [3.2] [P-1] Add inline documentation to all sections. Below each `SectionHeader`, add a `<p className="text-copy-13 text-ds-gray-700">` with contextual guidance text: Reminders tab ("Created via Telegram..."), Schedules tab ("Recurring jobs configured in daemon..."), Watcher card ("Monitors Telegram channels..."), Briefing card ("Generates a daily summary..."). [owner:ui-engineer]
- [ ] [3.3] [P-1] Convert Watcher card to editable controls. Replace read-only text with: (a) toggle switch for enabled/disabled calling `PATCH /api/automations/watcher`, (b) number stepper for interval_minutes (min 5, max 120, step 5) submitting on blur, (c) two time inputs for quiet_start/quiet_end submitting on blur. All edits are optimistic with revert on failure. [owner:ui-engineer]
- [ ] [3.4] [P-1] Add Briefing card controls. Add "Generate Now" button calling `POST /api/briefing/generate` with loading spinner. Show truncated `content_preview` (2-3 lines) from API response with click-through to `/briefing`. Add "View Briefing" link navigating to `/briefing`. [owner:ui-engineer]
- [ ] [3.5] [P-2] Improve empty states for Reminders and Schedules tabs. Reminders empty: "No active reminders. Tell Nova 'remind me to...' in Telegram, or create one via the API." Schedules empty: "No scheduled jobs. Schedules are configured in the daemon schedule-svc config." Use info icon + muted text styling. [owner:ui-engineer]
- [ ] [3.6] [P-2] Add Sessions cross-link. Replace removed Active Sessions section with an inline link in the page subtitle area: "N active sessions" or "No active sessions" linking to `/sessions`. Style as `text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000` with underline on hover. Use `active_sessions.length` from existing API response. [owner:ui-engineer]

## E2E Batch

- [ ] [4.1] Verify dashboard builds cleanly: `pnpm typecheck` passes with restructured page, new watcher route, updated types. [owner:e2e-engineer]
