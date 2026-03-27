# Capability: Automations Enhancements

## ADDED Requirements

### Requirement: settings table stores key-value automation config
The `packages/db/src/schema/settings.ts` file SHALL export a `settings` pgTable with columns: `key` (text, primary key), `value` (text, not null), `updatedAt` (timestamp with timezone, not null, default now). A Drizzle migration MUST be generated via `pnpm drizzle-kit generate`. The table MUST be re-exported from `packages/db/src/index.ts`.

#### Scenario: Insert a new setting
Given no row exists for key `"briefing_hour"`
When a row is inserted with key `"briefing_hour"` and value `"9"`
Then the row is persisted and `updated_at` is set to the current timestamp

#### Scenario: Update an existing setting
Given a row exists for key `"watcher_prompt"` with value `"Check for overdue items"`
When the row is updated with value `"Focus on calendar conflicts"`
Then the value column reflects the new text and `updated_at` is refreshed

#### Scenario: Query a missing setting returns no rows
Given no row exists for key `"briefing_prompt"`
When a SELECT is executed for that key
Then zero rows are returned and the caller uses a hardcoded default

---

### Requirement: GET and PUT /api/automations/settings endpoints
`apps/dashboard/app/api/automations/settings/route.ts` SHALL export GET and PUT handlers. GET returns all settings as `{ settings: Record<string, string> }`. PUT accepts `{ key: string, value: string }` where key MUST be one of `"watcher_prompt"`, `"briefing_prompt"`, `"briefing_hour"`. PUT upserts the row (insert on conflict update) and returns the updated setting. Invalid keys return 400.

#### Scenario: GET returns all stored settings
Given settings rows exist for `watcher_prompt` and `briefing_hour`
When GET /api/automations/settings is called
Then the response is `{ settings: { watcher_prompt: "...", briefing_hour: "9" } }` with status 200

#### Scenario: PUT upserts a setting
Given no row exists for `briefing_prompt`
When PUT is called with `{ key: "briefing_prompt", value: "Emphasize calendar" }`
Then a new row is created and the response contains the upserted key-value with status 200

#### Scenario: PUT rejects unknown keys
Given a PUT request with `{ key: "unknown_key", value: "foo" }`
When the request is processed
Then the response is 400 with an error message listing valid keys

---

### Requirement: briefing hour is configurable from the settings table
The `GET /api/automations` route SHALL read the `briefing_hour` setting from the settings table instead of using hardcoded 7. When no setting exists, it defaults to 7. The `next_generation` field in the briefing response MUST use the configured hour. The `AutomationsGetResponse.briefing` type in `apps/dashboard/types/api.ts` SHALL include a `briefing_hour` field (number).

#### Scenario: Custom briefing hour affects next_generation
Given `briefing_hour` is set to `"9"` in the settings table
When GET /api/automations is called at 8:00 AM
Then `briefing.next_generation` is today at 9:00 AM and `briefing.briefing_hour` is 9

#### Scenario: Missing briefing_hour defaults to 7
Given no `briefing_hour` row exists in the settings table
When GET /api/automations is called
Then `briefing.briefing_hour` is 7 and `next_generation` uses 7:00 AM

---

### Requirement: Rust scheduler reads briefing_hour from Postgres
`crates/nv-daemon/src/scheduler.rs` SHALL read the `briefing_hour` value from the Postgres `settings` table on each morning-briefing poll tick (every 60 seconds). The `MORNING_BRIEFING_HOUR` constant becomes the fallback default. A failed DB query logs a warning and uses the fallback. The scheduler caches the value for 60 seconds to avoid redundant queries.

#### Scenario: Scheduler fires at configured hour
Given `briefing_hour` is `"9"` in the settings table
When the scheduler polls at 9:00 AM local time
Then the morning briefing trigger fires

#### Scenario: Scheduler uses fallback on DB error
Given the settings table query fails (connection error)
When the scheduler polls at 7:00 AM local time
Then the morning briefing trigger fires using the fallback hour 7

#### Scenario: Scheduler does not fire at old hardcoded hour
Given `briefing_hour` is `"9"` in the settings table
When the scheduler polls at 7:00 AM local time
Then no morning briefing trigger fires

---

### Requirement: editable prompt textareas in WatcherCard and BriefingCard
The WatcherCard component SHALL include a collapsible "Custom Prompt" section with a textarea that loads from `GET /api/automations/settings` (key: `watcher_prompt`) and saves on blur via `PUT /api/automations/settings`. The BriefingCard component SHALL include an equivalent textarea for `briefing_prompt`. Both show a saved/saving indicator and use placeholder text describing the prompt purpose.

#### Scenario: Prompt loads from settings on mount
Given `watcher_prompt` is `"Focus on overdue obligations"` in the settings table
When the automations page loads
Then the watcher prompt textarea displays `"Focus on overdue obligations"`

#### Scenario: Prompt saves on blur
Given the user types `"Track calendar conflicts"` in the briefing prompt textarea
When the textarea loses focus
Then PUT /api/automations/settings is called with key `"briefing_prompt"` and the textarea shows a "Saved" indicator

#### Scenario: Empty settings show placeholder
Given no `watcher_prompt` row exists in the settings table
When the automations page loads
Then the watcher prompt textarea is empty with placeholder text

---

### Requirement: briefing hour time picker in BriefingCard
The BriefingCard component SHALL include an hour selector (0-23, displayed in 12-hour format with AM/PM) that reads the current `briefing_hour` from the settings API and saves changes via PUT. The selector updates the displayed `next_generation` time optimistically.

#### Scenario: Hour picker displays current setting
Given `briefing_hour` is `"9"` in the settings table
When the automations page loads
Then the hour picker shows "9:00 AM"

#### Scenario: Changing the hour saves and updates next_generation
Given the user changes the hour picker from 7 to 14
When the change is confirmed
Then PUT saves `briefing_hour: "14"` and the next_generation display updates to the next 2:00 PM

---

### Requirement: cross-page navigation links on automations page
The automations page SHALL include: a "View All Briefings" button in the BriefingCard linking to `/briefing`, a "View Watcher Sessions" link below the WatcherCard linking to `/sessions?command=proactive-followup`, and a "View Briefing Sessions" link below the BriefingCard linking to `/sessions?command=morning-briefing`.

#### Scenario: View All Briefings navigates to briefing page
Given the user clicks "View All Briefings"
When navigation completes
Then the browser is at `/briefing`

#### Scenario: View Watcher Sessions navigates with filter
Given the user clicks "View Watcher Sessions"
When navigation completes
Then the browser is at `/sessions?command=proactive-followup` and the session list is filtered

---

### Requirement: sessions page supports command query param filter
`apps/dashboard/app/sessions/page.tsx` SHALL read a `command` query parameter from the URL. When present, the session list is pre-filtered to show only sessions whose `command` field matches the param value. A dismissible filter chip is shown above the session list. Dismissing the chip clears the URL param and shows all sessions.

#### Scenario: Command filter applied from URL
Given the URL is `/sessions?command=proactive-followup`
When the page loads
Then only sessions with command `"proactive-followup"` are displayed and a filter chip shows "command: proactive-followup"

#### Scenario: Dismissing the filter chip shows all sessions
Given a command filter chip is active
When the user clicks the dismiss button on the chip
Then the URL updates to `/sessions` and all sessions are displayed

#### Scenario: Command filter combines with existing filters
Given the URL is `/sessions?command=morning-briefing` and statusFilter is "active"
When both filters are applied
Then only active sessions with command `"morning-briefing"` are shown

---

### Requirement: POST /api/automations/reminders creates a reminder
`apps/dashboard/app/api/automations/reminders/route.ts` SHALL export a POST handler that accepts `{ message: string, due_at: string (ISO 8601), channel?: string }`. Channel defaults to `"dashboard"`. The handler inserts into the `reminders` table and returns the created reminder with status 201. Validation: message must be non-empty (max 500 chars), due_at must be a valid future date.

#### Scenario: Create a valid reminder
Given POST with `{ message: "Follow up on PR", due_at: "2026-03-28T14:00:00Z" }`
When the request is processed
Then a row is inserted into reminders with channel `"dashboard"` and status 201 is returned

#### Scenario: Reject empty message
Given POST with `{ message: "", due_at: "2026-03-28T14:00:00Z" }`
When the request is processed
Then status 400 is returned with an error about message being required

#### Scenario: Reject past due date
Given POST with `{ message: "Test", due_at: "2020-01-01T00:00:00Z" }`
When the request is processed
Then status 400 is returned with an error about due_at being in the past

---

### Requirement: Create Reminder UI form in RemindersTab
The RemindersTab component SHALL include a "Create Reminder" button that toggles an inline creation form. The form has: message textarea (required, max 500 chars), date/time input (required, must be future), channel text input (optional, defaults to "dashboard"). Submit calls POST /api/automations/reminders. On success, the form closes and the reminders list refreshes. On error, an inline error message is shown.

#### Scenario: User creates a reminder via the form
Given the user clicks "Create Reminder" and fills in message and date
When the form is submitted
Then the reminder appears in the table and the form closes

#### Scenario: Validation prevents invalid submission
Given the user submits with an empty message
When the form validates
Then a client-side error appears and no API call is made

---

### Requirement: Telegram /brief command renamed to /snapshot
`packages/daemon/src/telegram/commands/brief.ts` SHALL be renamed to `snapshot.ts`. The exported function SHALL be renamed from `buildBriefReply` to `buildSnapshotReply`. The JSDoc SHALL be updated to reference `/snapshot`. In `packages/daemon/src/channels/telegram.ts`, the import SHALL change to `snapshot.js`, the switch case SHALL match `"snapshot"`, and the onText regex SHALL match `/^\/snapshot(@\S+)?$/`. In `help.ts`, the help text SHALL show `/snapshot` instead of `/brief`. In `start.ts`, the callback_data SHALL change from `"cmd:brief"` to `"cmd:snapshot"`.

#### Scenario: /snapshot returns the same output as old /brief
Given the user sends `/snapshot` in Telegram
When the command is processed
Then the response contains Calendar, Mail, and Obligations sections identical to the old /brief output

#### Scenario: /brief no longer responds
Given the user sends `/brief` in Telegram
When the command is processed
Then it falls through to the default NLP handler (no direct command match)

#### Scenario: Start keyboard uses /snapshot
Given the user sends `/start` in Telegram
When the keyboard is displayed
Then the "Briefing" button triggers `cmd:snapshot`

## MODIFIED Requirements

### Requirement: AutomationsGetResponse includes briefing_hour and content_preview
The existing `AutomationBriefing` type in `apps/dashboard/types/api.ts` SHALL be extended with a `briefing_hour: number` field. The `GET /api/automations` handler SHALL populate this from the settings table (default: 7).

#### Scenario: Briefing section includes configured hour
Given `briefing_hour` is 9 in settings
When GET /api/automations returns
Then `briefing.briefing_hour` is 9
