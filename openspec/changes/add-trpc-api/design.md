# Design: Add tRPC API Layer

## Architecture Overview

### Package Layout

```
packages/api/
  package.json              @nova/api
  tsconfig.json
  src/
    root.ts                 appRouter = mergeRouters(...)
    trpc.ts                 initTRPC, createTRPCContext, protectedProcedure, publicProcedure
    lib/
      auth.ts               Timing-safe token verification (standalone, no dashboard dep)
      fleet.ts              fleetFetch() helper for fleet HTTP calls
    routers/
      obligation.ts         9 procedures (list, getById, create, update, execute, activity, stats, approve, getRelated)
      contact.ts            9 procedures (list, getById, create, update, delete, getRelated, discovered, relationships, resolve)
      diary.ts              1 procedure (list)
      briefing.ts           3 procedures (latest, history, generate)
      message.ts            1 procedure (list)
      session.ts            5 procedures (list, getById, analytics, getEvents, ccSessions)
      automation.ts         8 procedures (getAll, listReminders, updateReminder, listSchedules, updateSchedule, getSettings, updateSettings, getWatcher)
      system.ts             6 procedures (health, latency, stats, fleetStatus, activityFeed, config)
      auth.ts               2 procedures (verify, logout)
      project.ts            4 procedures (list, getByCode, extract, getRelated)

apps/dashboard/
  app/api/trpc/[trpc]/route.ts    Catch-all handler (merges @nova/api + local routers)
  lib/trpc/
    client.ts               httpBatchLink + auth token injection
    server.ts               Server-side caller for RSC prefetch
    react.tsx               tRPC React proxy with queryOptions/mutationOptions
  lib/routers/
    cc-session.ts           Dashboard-local: Docker session manager procedures
    resolve.ts              Dashboard-local: Entity resolution procedures
```

### Provider Stack (post add-tanstack-query)

```
RootLayout (RSC)
  └─ TRPCProvider (NEW)
       └─ QueryClientProvider (from add-tanstack-query)
            └─ AppShell ("use client")
                 └─ DaemonEventProvider (existing)
                      └─ Sidebar + main content
                 └─ ReactQueryDevtools (dev only)
```

### Data Flow

```
Client Component
  │
  ├─ useQuery(trpc.obligation.list.queryOptions({ status: "open" }))
  │     │
  │     ▼
  │   httpBatchLink (Bearer token from cookie)
  │     │
  │     ▼
  │   /api/trpc/[trpc]/route.ts (fetchRequestHandler)
  │     │
  │     ▼
  │   createTRPCContext (extract token from header)
  │     │
  │     ▼
  │   protectedProcedure (validate token)
  │     │
  │     ▼
  │   obligation.list procedure
  │     │
  │     ▼
  │   db.select().from(obligations)... (Drizzle → Postgres)
  │
  └─ Result: typed, cached, auto-refetched
```

### Server-Side Caller (RSC Prefetch)

```
RSC Page Component
  │
  ├─ const caller = createCaller(await createContext(cookies()))
  ├─ void trpc.obligation.list.prefetch()
  │
  └─ <HydrationBoundary state={dehydrate(queryClient)}>
       <ObligationPage />  ← useQuery resolves from cache, no loading spinner
     </HydrationBoundary>
```

## Key Design Decisions

### Decision 1: Auth logic duplicated in packages/api, not imported from dashboard

The `verifyToken` function from `apps/dashboard/lib/auth.ts` uses Node.js `crypto.timingSafeEqual`. Rather than creating a cross-dependency from `packages/api/` to `apps/dashboard/`, the timing-safe comparison is replicated in `packages/api/src/lib/auth.ts`. Both implementations are <15 lines and share the same logic. This keeps `@nova/api` self-contained.

**Alternative considered:** Export auth from a shared package. Rejected because there is no `packages/shared/` and the auth logic is trivial.

### Decision 2: Dashboard-local routers for Docker/entity-resolution deps

CC session management (`sessionManager`) depends on Docker APIs. Entity resolution (`lib/entity-resolution/`) is dashboard-specific logic. Rather than pulling these dependencies into `packages/api/`, these procedures are defined as dashboard-local routers in `apps/dashboard/lib/routers/` and merged with the `@nova/api` appRouter at the catch-all handler:

```typescript
// apps/dashboard/app/api/trpc/[trpc]/route.ts
import { appRouter } from "@nova/api";
import { ccSessionRouter } from "@/lib/routers/cc-session";
import { resolveRouter } from "@/lib/routers/resolve";

const dashboardRouter = mergeRouters(appRouter, ccSessionRouter, resolveRouter);
```

This keeps `@nova/api` free of Docker SDK and dashboard-specific logic while still exposing all procedures through a single tRPC endpoint.

### Decision 3: Snake_case response shapes during migration

Current route handlers return snake_case fields (legacy from the Rust daemon era). The `types/api.ts` file and all client code expect snake_case. Changing to camelCase would require updating every client component simultaneously. Instead, tRPC procedures return the same snake_case shape during migration. After all clients are migrated and `types/api.ts` is deleted, a follow-up normalization pass can switch to camelCase with `RouterOutputs` providing the new types automatically.

**Alternative considered:** Switch to camelCase immediately. Rejected because it would block the phased migration approach -- every client file would need updating in a single batch.

### Decision 4: httpBatchLink for client transport

The dashboard makes 3-6 parallel API calls on most pages. `httpBatchLink` combines these into a single HTTP request, reducing connection overhead. The batch endpoint is `/api/trpc`.

**Alternative considered:** `httpLink` (one request per query). Rejected because the home page alone makes 6 parallel fetches -- batching cuts network round-trips from 6 to 1.

### Decision 5: Phased migration with coexistence

During migration, both old route handlers and tRPC procedures coexist. Pages are migrated one domain at a time (obligations first, then contacts, etc.). The `useApiQuery` bridge from `add-tanstack-query` allows pages to use either fetch path. After a domain's pages are all migrated, that domain's route handlers are deleted.

Migration order (by complexity and traffic):
1. obligation (9 procedures, high-traffic, CRUD -- validates the full pattern)
2. contact (9 procedures, CRUD)
3. session (5 procedures, includes CC session local router)
4. automation (8 procedures, complex aggregation)
5. message (1 procedure, pagination)
6. briefing (3 procedures)
7. diary (1 procedure)
8. system (6 procedures, fleet + health)
9. project (4 procedures)
10. auth (2 procedures, special: publicProcedure)
11. resolve (2 procedures, dashboard-local)

### Decision 6: Zod input schemas co-located with procedures

Each router file defines its own Zod input schemas inline (e.g., `z.object({ status: z.string().optional(), owner: z.string().optional() })` for obligation.list). Schemas that already exist in `@nova/db` (like `createProjectSchema`) are imported and reused. This avoids a separate "validators" layer and keeps procedures self-documenting.

## Fleet Fetch Configuration

| Env Var | Default | Service | Port |
|---------|---------|---------|------|
| `TOOL_ROUTER_URL` | `http://host.docker.internal:4100` | tool-router | 4100 |
| `MEMORY_SVC_URL` | `http://host.docker.internal:4101` | memory-svc | 4101 |
| `MESSAGES_SVC_URL` | `http://host.docker.internal:4102` | messages-svc | 4102 |
| `META_SVC_URL` | `http://host.docker.internal:4108` | meta-svc | 4108 |

Fleet services bind to `0.0.0.0`. The dashboard runs in Docker and reaches the host via `host.docker.internal` (already configured in `docker-compose.yml`).

## Trade-offs

| Decision | Alternative | Rationale |
|----------|-------------|-----------|
| Auth in packages/api (duplicated) | Shared package | No packages/shared exists; 15-line function not worth a new package |
| Dashboard-local routers | Everything in packages/api | Avoids Docker SDK + entity-resolution as api package deps |
| Snake_case during migration | Immediate camelCase | Enables phased migration without big-bang client changes |
| httpBatchLink | httpLink | Home page makes 6 parallel fetches; batching = 1 round-trip |
| Inline Zod schemas | Separate schema files | Co-location keeps procedures self-documenting |
| Phased domain-by-domain migration | Big-bang rewrite | Reduces risk; each domain is independently verifiable |
