# Proposal: Add tRPC API Layer

## Change ID
`add-trpc-api`

## Summary
Create a `packages/api/` workspace package with tRPC v11 routers that replace all 49 Next.js route handlers in `apps/dashboard/app/api/`, wiring type-safe procedures through a Next.js App Router handler and consuming them on the client via `queryOptions()` / `mutationOptions()` with TanStack Query.

## Context
- Depends on: `add-tanstack-query` (must land first -- provides `QueryClientProvider`, TanStack Query deps, and reusable state components)
- Extends: `packages/db/` (Drizzle schemas + db client), `apps/dashboard/middleware.ts` (auth), `apps/dashboard/lib/auth.ts` (token verification)
- Related: `rewire-dashboard-api` (88% complete -- rewired routes from daemon to Drizzle/fleet; this spec replaces those route handlers with tRPC procedures)
- Conflicts with: none (route handlers are deleted after tRPC equivalents are verified)

## Motivation
The dashboard has 49 Next.js route handlers with direct Drizzle queries and manual `NextResponse.json()` wrapping. Every handler independently implements error handling, input parsing, and response mapping. The client uses a custom `apiFetch()` wrapper with raw `fetch` -- no type safety across the network boundary. Adding a new endpoint requires creating a route file, defining request/response types in `types/api.ts`, and manually wiring the client fetch call. tRPC eliminates all of this: procedures are type-safe end-to-end, input validation uses Zod schemas already present in `@nova/db`, and the client gets full autocompletion and type inference without code generation. This is the single biggest T3 gap in the project.

The 49 route handlers break down into 8 natural domain routers (obligation, contact, diary, briefing, message, session, automation, system) plus 2 utility routers (auth, project). Of these, 37 are DB-backed (direct Drizzle queries), 6 are fleet-backed (HTTP proxy to Hono microservices on the host), 4 are CC session management (Docker), and 2 are auth. The Hono tool fleet (`packages/tools/`) stays as Hono HTTP + MCP -- tRPC is dashboard-only.

## Requirements

### Req-1: Create packages/api/ workspace package
Create `packages/api/` as a new workspace package (`@nova/api`) with tRPC v11, `@trpc/server`, `@trpc/client`, `@trpc/next`, `@trpc/react-query`, Zod, and a dependency on `@nova/db`. Export the root `appRouter` type (`AppRouter`) and the router itself. The package structure follows T3 Turbo conventions:

```
packages/api/
  src/
    root.ts          -- mergeRouters into appRouter, export AppRouter type
    trpc.ts          -- initTRPC, context factory, auth middleware
    routers/
      obligation.ts
      contact.ts
      diary.ts
      briefing.ts
      message.ts
      session.ts
      automation.ts
      system.ts
      auth.ts
      project.ts
```

### Req-2: tRPC context and auth middleware
Create a `createTRPCContext` factory that extracts the bearer token from the `Authorization` header (or cookie for server-side callers). Create `protectedProcedure` middleware that validates the token using the existing `verifyToken()` from `apps/dashboard/lib/auth.ts` -- or more precisely, replicate the timing-safe comparison logic in `packages/api/` so it has no dependency on `apps/dashboard/`. The `publicProcedure` is used only for `auth.verify` and `auth.logout`. When `DASHBOARD_TOKEN` is unset, auth is disabled (dev-mode fallback, matching current behavior).

### Req-3: Migrate DB-backed routes to tRPC procedures
Convert the 37 DB-backed route handlers into tRPC query/mutation procedures with Zod input validation. Each procedure imports `db` from `@nova/db` directly (not from `apps/dashboard/lib/db.ts`). Response shapes must match the current API contract (snake_case field names via `toSnakeCase` mapping, same JSON structure) to avoid breaking client code during the phased migration.

Domain grouping:

| Router | Procedures | Source |
|--------|-----------|--------|
| `obligation` | `list`, `getById`, `create`, `update`, `execute`, `activity`, `stats`, `approve`, `getRelated` | obligations table |
| `contact` | `list`, `getById`, `create`, `update`, `delete`, `getRelated`, `discovered`, `relationships`, `resolve` | contacts table |
| `diary` | `list` | diary table |
| `briefing` | `latest`, `history`, `generate` | briefings table |
| `message` | `list` | messages table |
| `session` | `list`, `getById`, `analytics`, `getEvents`, `ccSessions` | sessions + session_events tables |
| `automation` | `getAll`, `listReminders`, `updateReminder`, `listSchedules`, `updateSchedule`, `getSettings`, `updateSettings`, `getWatcher` | reminders + schedules + settings tables |
| `project` | `list`, `getByCode`, `extract`, `getRelated` | projects table |

### Req-4: Migrate fleet-backed routes to tRPC procedures
Convert the 6 fleet-backed route handlers into tRPC procedures that call fleet services via HTTP. Create a `fleetFetch` helper in `packages/api/src/lib/fleet.ts` (same pattern as the rewire spec planned but never created). Fleet service URLs come from environment variables with `host.docker.internal` defaults.

| Router | Procedure | Fleet Target |
|--------|----------|-------------|
| `system` | `health` | tool-router :4100 `/health` |
| `system` | `latency` | meta-svc :4108 `/health` |
| `system` | `stats` | meta-svc :4108 `/services` |
| `system` | `fleetStatus` | Static registry (no HTTP call) |
| `system` | `activityFeed` | DB query on obligations + messages |
| `system` | `config` | Env vars / static |

### Req-5: Migrate CC session routes to tRPC procedures
Convert the 4 CC session management routes (`session/control`, `session/logs`, `session/message`, `session/status`) into a `ccSession` sub-router. These call the Docker-based `sessionManager` (existing `lib/session-manager.ts`). The session manager stays in `apps/dashboard/` since it depends on Docker APIs -- the tRPC procedures in `packages/api/` call it via a context-injected function or the procedures live in a dashboard-local router that merges with the package router.

### Req-6: Wire tRPC into Next.js App Router
Create a catch-all API route at `apps/dashboard/app/api/trpc/[trpc]/route.ts` using `fetchRequestHandler` from `@trpc/server/adapters/fetch`. Create a server-side caller (`packages/api/src/server.ts` or `apps/dashboard/lib/trpc/server.ts`) for RSC prefetching. Create a client-side tRPC provider (`apps/dashboard/lib/trpc/client.ts`) that uses `httpBatchLink` and injects the bearer token from the cookie. The client provider integrates with the `QueryClientProvider` from `add-tanstack-query`.

### Req-7: Client migration to queryOptions/mutationOptions pattern
Replace all `useApiQuery` / `apiFetch` calls across 29 client files with tRPC's `queryOptions()` / `mutationOptions()` pattern consumed via `useQuery` / `useMutation` from TanStack Query. Never use direct tRPC hooks (`trpc.*.useQuery()`). Query invalidation uses `trpc.*.queryKey()` instead of manual key strings. This is phased -- route handlers are kept alive during migration and deleted only after the tRPC equivalent is verified.

### Req-8: Export RouterOutputs / RouterInputs type inference
Export `RouterOutputs` and `RouterInputs` helper types from `packages/api/` so the dashboard can infer procedure return types without manually maintaining `types/api.ts`. After migration is complete, the 733-line `types/api.ts` file is deleted and all types are inferred from the router.

### Req-9: Delete route handlers and types/api.ts
After all client code is migrated to tRPC, delete the 49 route handler files under `apps/dashboard/app/api/` (except `api/trpc/[trpc]/route.ts`), delete `apps/dashboard/lib/api-client.ts`, and delete `apps/dashboard/types/api.ts`. Update `apps/dashboard/middleware.ts` to handle `/api/trpc/*` auth (or remove the `/api/` auth block if tRPC middleware handles auth).

### Req-10: Resolve endpoint (entity resolution)
The `resolve/senders` and `contacts/resolve` routes use the entity resolution system in `apps/dashboard/lib/entity-resolution/`. Migrate these to a `resolve` sub-router. Since entity resolution logic lives in `apps/dashboard/`, these procedures can be defined in a dashboard-local router merged with the package router (same pattern as CC session routes).

## Scope
- **IN**: Create `packages/api/` with tRPC v11 routers, context, auth middleware; wire into Next.js App Router; migrate all 49 route handlers to tRPC procedures; migrate all 29 client files from `apiFetch` to `queryOptions()`/`mutationOptions()`; delete route handlers + `types/api.ts` + `api-client.ts` after migration; export `RouterOutputs`/`RouterInputs`
- **OUT**: Hono tool fleet changes (stays as Hono HTTP + MCP), new features or UI changes, database schema changes, Turborepo setup (separate spec), shadcn/ui migration (separate spec), real-time/WebSocket changes (SSE from fleet is a separate concern)

## Impact
| Area | Change |
|------|--------|
| `packages/api/` | NEW -- tRPC router package with 10 domain routers |
| `packages/api/package.json` | NEW -- `@nova/api` with tRPC v11, Zod, `@nova/db` dep |
| `apps/dashboard/package.json` | Add `@nova/api`, `@trpc/client`, `@trpc/react-query`, `@trpc/next` |
| `apps/dashboard/app/api/trpc/[trpc]/route.ts` | NEW -- catch-all tRPC handler |
| `apps/dashboard/lib/trpc/client.ts` | NEW -- tRPC client with httpBatchLink |
| `apps/dashboard/lib/trpc/server.ts` | NEW -- server-side caller for RSC prefetch |
| `apps/dashboard/app/api/**/*.ts` (49 files) | DELETE -- replaced by tRPC procedures |
| `apps/dashboard/lib/api-client.ts` | DELETE -- replaced by tRPC client |
| `apps/dashboard/types/api.ts` | DELETE -- replaced by RouterOutputs inference |
| `apps/dashboard/lib/case.ts` | DELETE -- snake_case mapping no longer needed once types align |
| `apps/dashboard/app/*/page.tsx` (18 pages) | Migrate from apiFetch to trpc.*.queryOptions() |
| `apps/dashboard/components/*.tsx` (11 files) | Migrate from apiFetch to trpc.*.queryOptions() |
| `apps/dashboard/middleware.ts` | Update `/api/` auth matcher for tRPC endpoint |
| `package.json` (root) | No change -- workspaces already includes `packages/*` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Response shape mismatch breaks existing client code | Procedures return the exact same JSON shape as current route handlers (snake_case fields, same nesting). Response shapes are verified by TypeScript -- `RouterOutputs` must satisfy the same interfaces before deleting `types/api.ts`. |
| Bundle size increase from tRPC client | tRPC client is ~3KB gzip. Combined with the TanStack Query dep from `add-tanstack-query`, total overhead is ~16KB gzip -- acceptable for a dashboard app not targeting mobile. |
| Auth token not available in tRPC context | The `createTRPCContext` reads `Authorization` header (client requests) and cookies (server-side prefetch). Both paths are tested. Dev-mode fallback (no `DASHBOARD_TOKEN`) disables auth checks, matching current behavior. |
| Fleet-backed procedures depend on host network | Same risk as current route handlers -- fleet services must be reachable from the Docker container via `host.docker.internal`. No new network requirements. |
| CC session manager has Docker dependency | Session manager stays in `apps/dashboard/`. CC session procedures are defined in a dashboard-local router file that merges with the `@nova/api` router at the catch-all handler, avoiding a Docker SDK dependency in `packages/api/`. |
| Phased migration creates dual-path complexity | Migration is batched per domain (obligation router first, then contacts, etc.). Each batch deletes the old route handlers only after verifying the tRPC equivalent works. `useApiQuery` from `add-tanstack-query` serves as the bridge -- pages can consume either path during transition. |
| `add-tanstack-query` dependency not yet landed | This spec explicitly depends on `add-tanstack-query`. If that spec is not yet applied, the UI batch tasks (client migration) cannot proceed. DB and API batches (package creation, router definitions) can proceed independently. |
