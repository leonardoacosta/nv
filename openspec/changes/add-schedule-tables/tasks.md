# Implementation Tasks

## DB Batch

- [x] [1.1] [P-1] Create `packages/db/src/schema/reminders.ts` — define `reminders` pgTable with columns: id (uuid PK defaultRandom), message (text NOT NULL), dueAt (timestamp with timezone NOT NULL), channel (text NOT NULL), createdAt (timestamp with timezone NOT NULL defaultNow), deliveredAt (timestamp with timezone nullable), cancelled (boolean NOT NULL default false), obligationId (uuid nullable); export `Reminder` and `NewReminder` inferred types [owner:db-engineer]
- [x] [1.2] [P-1] Create `packages/db/src/schema/schedules.ts` — define `schedules` pgTable with columns: id (uuid PK defaultRandom), name (text NOT NULL unique), cronExpr (text NOT NULL), action (text NOT NULL), channel (text NOT NULL), enabled (boolean NOT NULL default true), createdAt (timestamp with timezone NOT NULL defaultNow), lastRunAt (timestamp with timezone nullable); export `Schedule` and `NewSchedule` inferred types [owner:db-engineer]
- [x] [1.3] [P-1] Create `packages/db/src/schema/sessions.ts` — define `sessions` pgTable with columns: id (uuid PK defaultRandom), project (text NOT NULL), command (text NOT NULL), status (text NOT NULL default 'running'), startedAt (timestamp with timezone NOT NULL defaultNow), stoppedAt (timestamp with timezone nullable); export `Session` and `NewSession` inferred types [owner:db-engineer]
- [x] [1.4] [P-1] Update `packages/db/src/index.ts` — add barrel exports for reminders, schedules, sessions tables and their Reminder, NewReminder, Schedule, NewSchedule, Session, NewSession types [owner:db-engineer]
- [x] [1.5] [P-1] Update `packages/db/src/client.ts` — import reminders, schedules, sessions schemas and add them to the schema object passed to drizzle() [owner:db-engineer]
- [x] [1.6] [P-2] Run `pnpm db:generate` in `packages/db/` to produce migration SQL for all three new tables — verify migration file is created in `packages/db/drizzle/` and exit code is 0 [owner:db-engineer]
