# Spec: tRPC Router Procedures

## ADDED Requirements

### Requirement: Obligation router
The system SHALL create `packages/api/src/routers/obligation.ts` with procedures that replace the 9 obligation route handlers. All procedures MUST use `protectedProcedure` and import `db` + `obligations` schema from `@nova/db`.

#### Scenario: List obligations with filters
- GIVEN obligations exist in the database
- WHEN `obligation.list` is called with optional `status` and `owner` input params
- THEN it returns `{ obligations: [...] }` with snake_case fields matching `ObligationsGetResponse`
- AND results are ordered by `created_at` desc

#### Scenario: Create obligation with validation
- GIVEN valid input with `detected_action` (required, non-empty string)
- WHEN `obligation.create` is called
- THEN a new obligation is inserted and `{ obligation: { id } }` is returned with status 201 semantics
- AND missing optional fields default to: owner="nova", status="open", priority=2, source_channel="dashboard"

#### Scenario: Execute obligation
- GIVEN an obligation with status "open"
- WHEN `obligation.execute` is called with `{ id }`
- THEN the obligation status is updated to "in_progress" and `lastAttemptAt` is set to now

### Requirement: Contact router
The system SHALL create `packages/api/src/routers/contact.ts` with procedures replacing the 9 contact route handlers: `list`, `getById`, `create`, `update`, `delete`, `getRelated`, `discovered`, `relationships`, `resolve`.

#### Scenario: Full CRUD lifecycle
- GIVEN the contacts table
- WHEN `contact.create` is called with `{ name, channel_ids?, relationship_type?, notes? }`
- THEN a new contact is inserted and returned in snake_case
- AND `contact.getById({ id })` returns the created contact
- AND `contact.update({ id, ...fields })` updates and returns the contact
- AND `contact.delete({ id })` removes the contact

#### Scenario: Search contacts
- GIVEN contacts exist with various names
- WHEN `contact.list` is called with `{ q: "search term" }`
- THEN contacts with names matching the search term (LIKE) are returned

### Requirement: Diary router
The system SHALL create `packages/api/src/routers/diary.ts` with a single `list` query.

#### Scenario: List diary entries
- GIVEN diary entries exist
- WHEN `diary.list` is called with optional `date` and `limit` params
- THEN entries are returned ordered by `created_at` desc, filtered by date if provided

### Requirement: Briefing router
The system SHALL create `packages/api/src/routers/briefing.ts` with `latest`, `history`, and `generate` procedures.

#### Scenario: Get latest briefing
- GIVEN briefings exist
- WHEN `briefing.latest` is called
- THEN the most recent briefing (by `generated_at`) is returned

#### Scenario: List briefing history
- GIVEN briefings exist
- WHEN `briefing.history` is called with optional `limit`
- THEN briefings are returned ordered by `generated_at` desc

### Requirement: Message router
The system SHALL create `packages/api/src/routers/message.ts` with a `list` query replacing the messages route handler.

#### Scenario: List messages with pagination
- GIVEN messages exist
- WHEN `message.list` is called with `{ channel?, direction?, sort?, type?, limit?, offset? }`
- THEN messages are returned with `{ messages, total, limit, offset }` shape
- AND results respect all filter conditions and pagination

### Requirement: Session router
The system SHALL create `packages/api/src/routers/session.ts` with procedures replacing 7 session route handlers.

#### Scenario: List sessions
- GIVEN sessions exist
- WHEN `session.list` is called
- THEN all sessions are returned ordered by `started_at` desc

#### Scenario: Get session events
- GIVEN a session with events
- WHEN `session.getEvents({ id })` is called
- THEN session events for that session are returned ordered by timestamp

#### Scenario: Session analytics
- GIVEN session data exists
- WHEN `session.analytics` is called
- THEN aggregated session statistics are returned

### Requirement: Automation router
The system SHALL create `packages/api/src/routers/automation.ts` with procedures replacing the 6 automation route handlers.

#### Scenario: Get all automations
- GIVEN reminders, schedules, briefings, and settings exist
- WHEN `automation.getAll` is called
- THEN the response matches `AutomationsGetResponse` shape with reminders, schedules, watcher, briefing, active_sessions

#### Scenario: Update schedule
- GIVEN a schedule exists
- WHEN `automation.updateSchedule({ id, enabled?, cron_expr?, action? })` is called
- THEN the schedule is updated and returned

### Requirement: System router
The system SHALL create `packages/api/src/routers/system.ts` with procedures replacing system/infrastructure route handlers.

#### Scenario: Health check
- GIVEN the database is reachable
- WHEN `system.health` is called
- THEN `{ status: "healthy" }` is returned with database connectivity info

#### Scenario: Fleet status
- GIVEN the static fleet service registry
- WHEN `system.fleetStatus` is called
- THEN all 10 fleet services are returned with their configured URLs and "unknown" status

#### Scenario: Activity feed
- GIVEN recent obligations and messages
- WHEN `system.activityFeed` is called
- THEN a merged timeline of recent events is returned

### Requirement: Auth router
The system SHALL create `packages/api/src/routers/auth.ts` with `verify` and `logout` procedures using `publicProcedure` (no auth required).

#### Scenario: Verify valid token
- GIVEN a valid `DASHBOARD_TOKEN` is configured
- WHEN `auth.verify` is called with `{ token: "valid-token" }`
- THEN `{ ok: true }` is returned

#### Scenario: Logout
- WHEN `auth.logout` is called
- THEN `{ ok: true }` is returned (cookie clearing is handled by the client)

### Requirement: Project router
The system SHALL create `packages/api/src/routers/project.ts` with `list`, `getByCode`, `extract`, and `getRelated` procedures.

#### Scenario: List projects
- GIVEN projects exist in the database
- WHEN `project.list` is called
- THEN all projects are returned ordered by name

#### Scenario: Get project by code
- GIVEN a project with code "nv" exists
- WHEN `project.getByCode({ code: "nv" })` is called
- THEN the project details are returned
- AND if the project does not exist, a `NOT_FOUND` TRPCError is thrown

### Requirement: Resolve router
The system SHALL create entity resolution procedures for `resolve/senders` and `contacts/resolve` endpoints. Since entity resolution logic lives in `apps/dashboard/lib/entity-resolution/`, these procedures MUST be defined in a dashboard-local router file that merges with the `@nova/api` root router.

#### Scenario: Resolve senders
- GIVEN message senders that may map to known contacts
- WHEN `resolve.senders` is called
- THEN sender identifiers are resolved to contact entities where possible

## MODIFIED Requirements

### Requirement: Response shape compatibility
All tRPC procedures MUST return the exact same JSON shape as the current route handlers during the migration period. This means snake_case field names (via `toSnakeCase` mapping) and the same nesting structure. After all clients are migrated, a follow-up spec can normalize to camelCase.

#### Scenario: Obligation response shape matches
- GIVEN the current `/api/obligations` returns `{ obligations: [{ id, source_channel, detected_action, ... }] }`
- WHEN `obligation.list` is called via tRPC
- THEN the response has the identical shape with snake_case fields
