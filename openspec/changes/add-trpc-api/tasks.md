# Implementation Tasks

<!-- beads:epic:nv-kyzh -->

## DB Batch

(No schema changes -- all tables already exist in @nova/db)

## API Batch

- [x] [2.1] [P-1] Create `packages/api/package.json` with `@trpc/server`, `@trpc/client`, `@trpc/react-query`, `zod`, `@nova/db` deps. Create `tsconfig.json`. Run `pnpm install` to register workspace package `@nova/api`. [owner:api-engineer] [beads:nv-emls]
- [x] [2.2] [P-1] Create `packages/api/src/trpc.ts` -- `initTRPC` with context type, `createTRPCContext` factory (extracts bearer token from Authorization header), `publicProcedure`, `protectedProcedure` (validates token via timing-safe comparison against `DASHBOARD_TOKEN` env var, dev-mode bypass when unset). [owner:api-engineer] [beads:nv-utbm]
- [x] [2.3] [P-1] Create `packages/api/src/lib/auth.ts` -- standalone timing-safe token verification (replicates `apps/dashboard/lib/auth.ts` logic, no dashboard dependency). [owner:api-engineer] [beads:nv-se5t]
- [x] [2.4] [P-1] Create `packages/api/src/lib/fleet.ts` -- `fleetFetch(service, path, init?)` helper resolving URLs from env vars (`TOOL_ROUTER_URL`, `MEMORY_SVC_URL`, `MESSAGES_SVC_URL`, `META_SVC_URL`) with `host.docker.internal` defaults and 5s timeout. [owner:api-engineer] [beads:nv-c3ga]
- [x] [2.5] [P-2] Create `packages/api/src/routers/obligation.ts` -- 9 procedures: `list` (GET with status/owner filters), `getById` (GET by ID), `create` (POST with Zod validation), `update` (PATCH by ID), `execute` (POST set status to in_progress), `activity` (GET recent updates), `stats` (GET aggregated counts), `approve` (POST set status to proposed_done), `getRelated` (GET related entities by obligation ID). All use `protectedProcedure` + Drizzle. Return snake_case shapes matching current API. [owner:api-engineer] [beads:nv-zkzf]
- [x] [2.6] [P-2] Create `packages/api/src/routers/contact.ts` -- 9 procedures: `list` (GET with relationship/q filters), `getById`, `create`, `update`, `delete`, `getRelated`, `discovered`, `relationships`, `resolve`. All use `protectedProcedure` + Drizzle. [owner:api-engineer] [beads:nv-450s]
- [x] [2.7] [P-2] Create `packages/api/src/routers/diary.ts` -- 1 procedure: `list` (GET with optional date/limit). [owner:api-engineer] [beads:nv-vyhh]
- [x] [2.8] [P-2] Create `packages/api/src/routers/briefing.ts` -- 3 procedures: `latest` (GET newest), `history` (GET with limit), `generate` (POST trigger generation). [owner:api-engineer] [beads:nv-jm1i]
- [x] [2.9] [P-2] Create `packages/api/src/routers/message.ts` -- 1 procedure: `list` (GET with channel/direction/sort/type/limit/offset filters, returns `{ messages, total, limit, offset }`). [owner:api-engineer] [beads:nv-s8au]
- [x] [2.10] [P-2] Create `packages/api/src/routers/session.ts` -- 5 procedures: `list` (GET all), `getById` (GET by ID), `analytics` (GET aggregated stats), `getEvents` (GET events for session ID), `ccSessions` (GET CC-type sessions). [owner:api-engineer] [beads:nv-y6p4]
- [x] [2.11] [P-2] Create `packages/api/src/routers/automation.ts` -- 8 procedures: `getAll` (GET full automations response), `listReminders`, `updateReminder`, `listSchedules`, `updateSchedule`, `getSettings`, `updateSettings`, `getWatcher`. [owner:api-engineer] [beads:nv-hfdu]
- [x] [2.12] [P-2] Create `packages/api/src/routers/system.ts` -- 6 procedures: `health` (DB ping), `latency` (fleet meta-svc health), `stats` (fleet meta-svc services), `fleetStatus` (static registry), `activityFeed` (merged timeline from DB), `config` (env vars/static). [owner:api-engineer] [beads:nv-xghi]
- [x] [2.13] [P-2] Create `packages/api/src/routers/auth.ts` -- 2 procedures using `publicProcedure`: `verify` (validate token input), `logout` (return ok). [owner:api-engineer] [beads:nv-0xek]
- [x] [2.14] [P-2] Create `packages/api/src/routers/project.ts` -- 4 procedures: `list` (GET all), `getByCode` (GET by code, NOT_FOUND on miss), `extract` (POST extract project from text), `getRelated` (GET related entities). [owner:api-engineer] [beads:nv-8a68]
- [x] [2.15] [P-3] Create `packages/api/src/root.ts` -- merge all 10 routers into `appRouter`, export `AppRouter` type, `RouterOutputs`, `RouterInputs`. [owner:api-engineer] [beads:nv-zwig]
- [x] [2.16] [P-3] Create `packages/api/src/index.ts` -- barrel exports: `appRouter`, `AppRouter`, `createTRPCContext`, `RouterOutputs`, `RouterInputs`. [owner:api-engineer] [beads:nv-xqld]
- [x] [2.17] [P-3] Add `@nova/api` as dependency in `apps/dashboard/package.json`. Add `@trpc/client`, `@trpc/react-query`, `@trpc/next` to dashboard deps. [owner:api-engineer] [beads:nv-wg24]
- [x] [2.18] [P-3] Create `apps/dashboard/app/api/trpc/[trpc]/route.ts` -- catch-all handler using `fetchRequestHandler`. Merge `@nova/api` appRouter with dashboard-local routers (cc-session, resolve). [owner:api-engineer] [beads:nv-opos]
- [x] [2.19] [P-3] Create `apps/dashboard/lib/trpc/client.ts` -- typed tRPC client with `httpBatchLink`, bearer token injection from `dashboard_token` cookie, 401 redirect handling. [owner:api-engineer] [beads:nv-qbqv]
- [x] [2.20] [P-3] Create `apps/dashboard/lib/trpc/server.ts` -- server-side caller for RSC prefetch, reads auth cookie from `next/headers`. [owner:api-engineer] [beads:nv-1z4v]
- [x] [2.21] [P-3] Create `apps/dashboard/lib/trpc/react.tsx` -- tRPC React proxy with `queryOptions()`, `mutationOptions()`, `queryKey()` methods. [owner:api-engineer] [beads:nv-fom6]
- [x] [2.22] [P-3] Create `apps/dashboard/lib/routers/cc-session.ts` -- dashboard-local tRPC router for 4 CC session procedures (control, logs, message, status) wrapping existing `sessionManager`. [owner:api-engineer] [beads:nv-gc58]
- [x] [2.23] [P-3] Create `apps/dashboard/lib/routers/resolve.ts` -- dashboard-local tRPC router for entity resolution procedures (resolve senders, resolve contacts) wrapping existing `lib/entity-resolution/`. [owner:api-engineer] [beads:nv-z4h9]

## UI Batch

- [x] [3.1] [P-1] Migrate `apps/dashboard/app/obligations/page.tsx` + `components/obligations/KanbanBoard.tsx` + `KanbanCard.tsx` + `InlineCreate.tsx` -- replace apiFetch with `trpc.obligation.*.queryOptions()` / `mutationOptions()`. Invalidate `trpc.obligation.list.queryKey()` on mutations. [owner:ui-engineer] [beads:nv-ji1x]
- [x] [3.2] [P-1] Migrate `apps/dashboard/app/contacts/page.tsx` -- replace apiFetch with `trpc.contact.*.queryOptions()` / `mutationOptions()`. [owner:ui-engineer] [beads:nv-g6qq]
- [x] [3.3] [P-1] Migrate `apps/dashboard/app/sessions/page.tsx` + `sessions/[id]/page.tsx` + `components/SessionDashboard.tsx` + `SessionWidget.tsx` + `CCSessionPanel.tsx` -- replace apiFetch with `trpc.session.*.queryOptions()`. [owner:ui-engineer] [beads:nv-ru1f]
- [x] [3.4] [P-2] Migrate `apps/dashboard/app/automations/page.tsx` -- replace apiFetch with `trpc.automation.*.queryOptions()` / `mutationOptions()`. [owner:ui-engineer] [beads:nv-6vxc]
- [x] [3.5] [P-2] Migrate `apps/dashboard/app/messages/page.tsx` -- replace apiFetch with `trpc.message.list.queryOptions()`. [owner:ui-engineer] [beads:nv-m4dg]
- [x] [3.6] [P-2] Migrate `apps/dashboard/app/briefing/page.tsx` -- replace apiFetch with `trpc.briefing.*.queryOptions()`. [owner:ui-engineer] [beads:nv-iaik]
- [x] [3.7] [P-2] Migrate `apps/dashboard/app/diary/page.tsx` -- replace apiFetch with `trpc.diary.list.queryOptions()`. [owner:ui-engineer] [beads:nv-p98u]
- [x] [3.8] [P-2] Migrate `apps/dashboard/app/memory/page.tsx` -- replace apiFetch with `trpc.system.*.queryOptions()`. [owner:ui-engineer] [beads:nv-6bvu]
- [x] [3.9] [P-2] Migrate `apps/dashboard/app/page.tsx` (home) + `components/ActivityFeed.tsx` + `LatencyChart.tsx` + `UsageSparkline.tsx` -- replace 6 parallel apiFetch calls with independent `useQuery(trpc.*.queryOptions())` calls. [owner:ui-engineer] [beads:nv-p9w2]
- [x] [3.10] [P-2] Migrate `apps/dashboard/app/projects/page.tsx` + `components/CreateProjectDialog.tsx` + `ProjectDetailPanel.tsx` -- replace apiFetch with `trpc.project.*.queryOptions()` / `mutationOptions()`. [owner:ui-engineer] [beads:nv-yt0m]
- [x] [3.11] [P-3] Migrate `apps/dashboard/app/usage/page.tsx` + `components/ColdStartsPanel.tsx` -- replace apiFetch with tRPC queries. [owner:ui-engineer] [beads:nv-yjai]
- [x] [3.12] [P-3] Migrate `apps/dashboard/app/integrations/page.tsx` -- replace apiFetch with `trpc.system.fleetStatus.queryOptions()`. [owner:ui-engineer] [beads:nv-8tni]
- [x] [3.13] [P-3] Migrate `apps/dashboard/app/settings/page.tsx` -- replace apiFetch with `trpc.system.config.queryOptions()` / `mutationOptions()`. [owner:ui-engineer] [beads:nv-qmk0]
- [x] [3.14] [P-3] Migrate `apps/dashboard/app/chat/page.tsx` + `components/Sidebar.tsx` -- replace apiFetch with tRPC queries for chat history and send. [owner:ui-engineer] [beads:nv-1pvw]
- [x] [3.15] [P-3] Migrate `apps/dashboard/app/nexus/page.tsx` -- replace apiFetch with tRPC queries. [owner:ui-engineer] [beads:nv-waha]
- [x] [3.16] [P-4] Delete all 49 route handler files under `apps/dashboard/app/api/` except `api/trpc/[trpc]/route.ts`. Verify no imports reference deleted files. [owner:ui-engineer] [beads:nv-q8o5]
- [x] [3.17] [P-4] Delete `apps/dashboard/lib/api-client.ts`, `apps/dashboard/types/api.ts`, `apps/dashboard/lib/case.ts`. Replace any remaining type imports with `RouterOutputs` from `@nova/api`. [owner:ui-engineer] [beads:nv-dddv]
- [x] [3.18] [P-4] Update `apps/dashboard/middleware.ts` -- simplify `/api/` auth block to only cover `/api/trpc/*`. Remove WebSocket auth block if not needed. Update auth exclusion to use tRPC auth router path. [owner:ui-engineer] [beads:nv-v5ml]

## E2E Batch

- [ ] [4.1] Verify `packages/api/` builds cleanly: `pnpm --filter @nova/api build` succeeds with zero errors. [owner:e2e-engineer] [beads:nv-xiiz]
- [ ] [4.2] Verify dashboard builds cleanly: `pnpm --filter nova-dashboard typecheck` passes. No imports of deleted files (`api-client.ts`, `types/api.ts`, old route handlers). [owner:e2e-engineer] [beads:nv-tiqw]
- [ ] [4.3] Verify tRPC endpoint responds: curl `/api/trpc/system.health` from inside Docker container returns `{ result: { data: { status: "healthy" } } }`. [owner:e2e-engineer] [beads:nv-02kx]
- [ ] [4.4] Verify auth enforcement: curl `/api/trpc/obligation.list` without bearer token returns UNAUTHORIZED error. With valid token returns obligation data. [owner:e2e-engineer] [beads:nv-tael]
- [ ] [4.5] Verify no remaining apiFetch imports: `grep -r "apiFetch" apps/dashboard/` returns zero matches (excluding node_modules). [owner:e2e-engineer] [beads:nv-piuw]
