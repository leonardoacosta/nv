# Implementation Tasks

<!-- beads:epic:nv-xefh -->

## DB Batch

- [x] [1.1] [P-1] Create `packages/validators/` with `package.json` (`@nova/validators`, deps: `zod`, `drizzle-zod`, peer: `@nova/db`), `tsconfig.json` (ESM strict, matching existing packages), and empty `src/index.ts` [owner:db-engineer] [beads:nv-84vf]
- [x] [1.2] [P-1] Create `src/common.ts` with shared schemas: `paginationSchema` (limit/offset with defaults), `cursorPaginationSchema`, `sortOrderSchema`, `dateRangeSchema` (coerced Date), `uuidParamSchema` [owner:db-engineer] [beads:nv-m0ye]
- [x] [1.3] [P-1] Create `src/messages.ts`: generate `insertMessageSchema`/`selectMessageSchema` via `drizzle-zod` with overrides for `embedding` (number array) and `metadata` (record), add `createMessageSchema` (omit id/createdAt), `messageFilterSchema` (channel, date range) [owner:db-engineer] [beads:nv-s7of]
- [x] [1.4] [P-1] Create `src/obligations.ts`: generate insert/select schemas, define `obligationStatusEnum`, add `createObligationSchema` (omit server fields, defaults for status/priority/owner/sourceChannel), `updateObligationSchema` (partial), `obligationFilterSchema` (status, owner) [owner:db-engineer] [beads:nv-hkbi]
- [x] [1.5] [P-1] Create `src/contacts.ts`: generate insert/select schemas with `channelIds` override (record type), add `createContactSchema` (name required, channelIds required), `updateContactSchema` (partial) [owner:db-engineer] [beads:nv-wug1]
- [x] [1.6] [P-1] Create `src/projects.ts`: migrate `projectCategoryEnum`, `projectStatusEnum`, `createProjectSchema`, `updateProjectSchema` from `packages/db/src/schema/projects.ts`, add `drizzle-zod` insert/select schemas [owner:db-engineer] [beads:nv-6j0h]
- [x] [1.7] [P-1] Create `src/memory.ts`: generate insert/select schemas with `embedding` override, add `createMemorySchema` (topic + content required), `updateMemorySchema` (content only) [owner:db-engineer] [beads:nv-cpob]
- [x] [1.8] [P-2] Create `src/reminders.ts`: generate insert/select schemas, add `createReminderSchema` (message, dueAt, channel required), `updateReminderSchema` (partial) [owner:db-engineer] [beads:nv-2dvr]
- [x] [1.9] [P-2] Create `src/schedules.ts`: generate insert/select schemas, add `createScheduleSchema` (name, cronExpr, action, channel required), `updateScheduleSchema` (partial) [owner:db-engineer] [beads:nv-m3z0]
- [x] [1.10] [P-2] Create `src/sessions.ts`: generate insert/select schemas, add `createSessionSchema` (project, command required), `sessionFilterSchema` (project, status, date range) [owner:db-engineer] [beads:nv-x8ce]
- [x] [1.11] [P-2] Create `src/session-events.ts`: generate insert/select schemas only (append-only table, no DTOs) [owner:db-engineer] [beads:nv-cmkx]
- [x] [1.12] [P-2] Create `src/briefings.ts`: generate insert/select schemas with `sourcesStatus`/`suggestedActions` overrides, add `createBriefingSchema` [owner:db-engineer] [beads:nv-jhtf]
- [x] [1.13] [P-2] Create `src/diary.ts`: generate insert/select schemas with `toolsUsed` override only (read-only in dashboard, no DTOs) [owner:db-engineer] [beads:nv-7522]
- [x] [1.14] [P-2] Create `src/settings.ts`: generate insert/select schemas, add `upsertSettingSchema` (key + value required) [owner:db-engineer] [beads:nv-qadu]
- [x] [1.15] [P-2] Create `src/index.ts` barrel: re-export all schemas and types from entity files and `common.ts` [owner:db-engineer] [beads:nv-eo1j]
- [x] [1.16] [P-2] Remove Zod schemas and types from `packages/db/src/schema/projects.ts`, update `packages/db/src/index.ts` to remove Zod re-exports, remove `zod` from `packages/db/package.json` dependencies [owner:db-engineer] [beads:nv-dpds]
- [x] [1.17] [P-2] Run `pnpm install` and verify `pnpm build` succeeds for `@nova/validators` and `@nova/db` with zero type errors [owner:db-engineer] [beads:nv-oy0z]

## E2E Batch

- [x] [4.1] Add unit tests in `packages/validators/src/__tests__/` for create/update DTOs: valid input passes, missing required fields rejected, partial updates accepted, pagination defaults applied [owner:e2e-engineer] [beads:nv-golh]
- [x] [4.2] Add unit tests for drizzle-zod overrides: vector fields accept number arrays, jsonb fields accept records, custom type fallbacks work [owner:e2e-engineer] [beads:nv-9en7]
