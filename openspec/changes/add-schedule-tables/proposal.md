# Proposal: Add Schedule Tables

## Change ID
`add-schedule-tables`

## Summary

Add three new Drizzle schema files to `packages/db/` for the schedule-svc domain: `reminders`, `schedules`, and `sessions`. These tables port the existing Rust SQLite schemas to Postgres and add a new `sessions` table for tracking CC session lifecycle. Export all schemas and types from `packages/db/src/index.ts` and register them in the Drizzle client.

## Context
- Extends: `packages/db/src/schema/` (existing schema directory with 6 tables), `packages/db/src/index.ts` (barrel exports), `packages/db/src/client.ts` (Drizzle client with schema registration)
- Related: `openspec/specs/db-schema/spec.md` (original DB schema spec — established the pattern), Rust `crates/nv-daemon/src/reminders.rs` (SQLite reminders schema being ported), Rust `crates/nv-daemon/src/tools/schedule.rs` (SQLite schedules schema being ported)
- Depends on: nothing — Wave 1, no dependencies
- Depended on by: `add-schedule-svc` (Wave 5 — the Hono+MCP service that reads/writes these tables)

## Motivation

The v10 tool fleet architecture decomposes Nova's tools into standalone Hono+MCP microservices backed by a shared Postgres instance. The schedule-svc (:4006) owns 9 tools across three domains:

1. **Reminders** (set_reminder, cancel_reminder, list_reminders) — one-shot timers. Currently in SQLite (`reminders.db`) via the Rust daemon. Moving to Postgres gives us proper UUID PKs, timestamptz handling, and shared access from both the daemon and the new microservice during migration.

2. **Schedules** (add_schedule, modify_schedule, remove_schedule, list_schedules) — user-defined recurring cron jobs. Currently in SQLite (`schedules.db`). Same migration rationale as reminders.

3. **Sessions** (start_session, stop_session) — CC session lifecycle tracking. Currently in-memory only (NexusBackend). Adding a Postgres table gives the schedule-svc a persistent record of session starts/stops for auditability and state recovery after restarts.

This spec creates only the Drizzle schemas and migration. The schedule-svc service itself is a separate spec (`add-schedule-svc`, Wave 5).

## Requirements

### Req-1: Reminders Schema (`packages/db/src/schema/reminders.ts`)

Define a `reminders` table with columns matching the Rust SQLite schema, adapted to Postgres conventions:

| Column | Type | Constraints |
|--------|------|-------------|
| `id` | uuid | PK, defaultRandom |
| `message` | text | NOT NULL |
| `due_at` | timestamp with timezone | NOT NULL |
| `channel` | text | NOT NULL |
| `created_at` | timestamp with timezone | NOT NULL, defaultNow |
| `delivered_at` | timestamp with timezone | nullable |
| `cancelled` | boolean | NOT NULL, default false |
| `obligation_id` | uuid | nullable (FK to obligations.id) |

Changes from Rust SQLite:
- `id` upgrades from INTEGER AUTOINCREMENT to UUID (matches all other Nova tables)
- `due_at`, `created_at`, `delivered_at` upgrade from TEXT (ISO 8601 strings) to `timestamptz`
- `cancelled` upgrades from INTEGER (0/1) to boolean
- `obligation_id` upgrades from TEXT to uuid

Export `Reminder` (select type) and `NewReminder` (insert type).

### Req-2: Schedules Schema (`packages/db/src/schema/schedules.ts`)

Define a `schedules` table:

| Column | Type | Constraints |
|--------|------|-------------|
| `id` | uuid | PK, defaultRandom |
| `name` | text | NOT NULL, unique |
| `cron_expr` | text | NOT NULL |
| `action` | text | NOT NULL |
| `channel` | text | NOT NULL |
| `enabled` | boolean | NOT NULL, default true |
| `created_at` | timestamp with timezone | NOT NULL, defaultNow |
| `last_run_at` | timestamp with timezone | nullable |

Changes from Rust SQLite:
- `id` upgrades from TEXT (UUID string) to native uuid
- `enabled` upgrades from INTEGER (0/1) to boolean
- Timestamps upgrade from TEXT to `timestamptz`

Export `Schedule` (select type) and `NewSchedule` (insert type).

### Req-3: Sessions Schema (`packages/db/src/schema/sessions.ts`)

Define a `sessions` table for tracking CC session lifecycle:

| Column | Type | Constraints |
|--------|------|-------------|
| `id` | uuid | PK, defaultRandom |
| `project` | text | NOT NULL |
| `command` | text | NOT NULL |
| `status` | text | NOT NULL, default 'running' |
| `started_at` | timestamp with timezone | NOT NULL, defaultNow |
| `stopped_at` | timestamp with timezone | nullable |

The `status` column holds one of: `running`, `stopped`, `failed`. This is a new table with no Rust equivalent (sessions were previously in-memory only).

Export `Session` (select type) and `NewSession` (insert type).

### Req-4: Barrel Exports and Client Registration

- Add exports for all three schemas and their types to `packages/db/src/index.ts`
- Register all three tables in the `schema` object in `packages/db/src/client.ts`

### Req-5: Migration Generation

Run `pnpm db:generate` from `packages/db/` to produce the Drizzle migration SQL for all three new tables.

## Scope
- **IN**: Three new schema files (`reminders.ts`, `schedules.ts`, `sessions.ts`), barrel exports in `index.ts`, client registration in `client.ts`, generated Drizzle migration
- **OUT**: schedule-svc service code, tool implementations, reminder scheduler background task, cron validation, session dispatch logic, data migration from SQLite to Postgres

## Impact
| Area | Change |
|------|--------|
| `packages/db/src/schema/reminders.ts` | New: `reminders` pgTable with 8 columns |
| `packages/db/src/schema/schedules.ts` | New: `schedules` pgTable with 8 columns |
| `packages/db/src/schema/sessions.ts` | New: `sessions` pgTable with 6 columns |
| `packages/db/src/index.ts` | Modified: add exports for 3 tables + 6 types |
| `packages/db/src/client.ts` | Modified: add 3 tables to schema object |
| `packages/db/drizzle/` | New: generated migration SQL |

## Risks
| Risk | Mitigation |
|------|-----------|
| obligation_id FK to obligations table may cause issues if obligations table doesn't exist yet | obligations table already exists (created in original db-schema spec); FK is safe |
| Session status as text column instead of Postgres enum | Text is simpler and matches the project convention (obligations.status is also text); enum can be added later if needed |
| Migration generation may conflict with pending migrations | Wave 1 has no dependencies; run generation on clean state |
