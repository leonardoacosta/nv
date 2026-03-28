# Tasks: materialize-contacts-projects-from-memory

## DB Batch

_No schema changes required -- uses existing `contacts`, `projects`, and `memory` tables._

## API Batch

- [x] **T1** `api-engineer` — Move people-parser to API package
  Create `packages/api/src/lib/people-parser.ts` with the `parsePeopleMemory()` function and `PersonProfile` interface copied from `apps/dashboard/lib/entity-resolution/people-parser.ts`. Update the dashboard file to re-export from `@nova/api` or keep as-is if cross-package import is not feasible.

- [x] **T2** `api-engineer` — Implement contact materialization logic [depends: T1]
  Create `packages/api/src/lib/materialize-contacts.ts`. Read "people" memory topic from Postgres, parse via `parsePeopleMemory()`, match each profile to existing contacts by channel ID values (iterate contacts, check jsonb value overlap) then by case-insensitive name. Upsert: merge channelIds (union), update notes/relationshipType if enriched. Return `{ created, updated, unchanged }`.

- [x] **T3** `api-engineer` — Implement project materialization logic
  Create `packages/api/src/lib/materialize-projects.ts`. Fetch daemon project registry via `GET http://localhost:8400/api/projects` (with fetch timeout + fallback to `NV_PROJECTS` env var). Read all `projects-*` memory topics. Deduplicate by project code. Match to existing projects by code. Upsert: set path from daemon if null, set description from memory content (first 500 chars) if null. Insert new projects with code, name, category "work", status "active". Return `{ created, updated, unchanged }`.

- [x] **T4** `api-engineer` — Add contact.materialize tRPC mutation [depends: T2]
  Add `materialize` mutation to `packages/api/src/routers/contact.ts` calling `materializeContacts()`. Return the result object.

- [x] **T5** `api-engineer` — Add project.materialize tRPC mutation [depends: T3]
  Add `materialize` mutation to `packages/api/src/routers/project.ts` calling `materializeProjects()`. Return the result object.

## UI Batch

- [ ] **T6** `ui-engineer` — Rewire contacts page Refresh button [depends: T4]
  Modify `apps/dashboard/app/contacts/page.tsx`: add `useMutation(trpc.contact.materialize.mutationOptions(...))`. On Refresh click, call materialize mutation, then invalidate `contact.discovered` and `contact.list` query keys. Show toast with results (created/updated counts). Keep the RefreshCw spinner during the mutation.

- [ ] **T7** `ui-engineer` — Rewire projects page Refresh button [depends: T5]
  Modify `apps/dashboard/app/projects/page.tsx`: add `useMutation(trpc.project.materialize.mutationOptions(...))`. Change `handleRefresh` to call materialize first, then extract, then re-fetch. Show toast with materialization results. Keep the RefreshCw spinner across both mutations.

## Daemon Batch

- [ ] **T8** `api-engineer` — Add materialize scheduler to daemon
  Create `packages/daemon/src/features/materialize/scheduler.ts` following DreamScheduler pattern. Add `MaterializeConfig` to `packages/daemon/src/config.ts` with `enabled: boolean` and `cronHour: number` (default 4). Parse from `[materialize]` section in nv.toml. On cron tick, POST to `http://localhost:3000/api/trpc/contact.materialize` and `http://localhost:3000/api/trpc/project.materialize` via fetch. Log results. Create barrel export at `packages/daemon/src/features/materialize/index.ts`. Wire start/stop in `packages/daemon/src/index.ts`. Add `[materialize]` section to `config/nv.toml`.
