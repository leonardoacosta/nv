# Spec: Next.js tRPC Wiring

## ADDED Requirements

### Requirement: tRPC catch-all route handler
The system SHALL create `apps/dashboard/app/api/trpc/[trpc]/route.ts` using `fetchRequestHandler` from `@trpc/server/adapters/fetch`. The handler MUST pass the `appRouter` and `createTRPCContext` from `@nova/api`, plus any dashboard-local routers (CC session, resolve) merged in.

#### Scenario: tRPC endpoint responds to queries
- GIVEN the tRPC catch-all handler is mounted
- WHEN a GET request hits `/api/trpc/obligation.list?input=...`
- THEN the obligation.list procedure executes and returns JSON

#### Scenario: tRPC endpoint responds to mutations
- GIVEN the tRPC catch-all handler is mounted
- WHEN a POST request hits `/api/trpc/obligation.create` with JSON body
- THEN the obligation.create procedure executes and returns the created entity

#### Scenario: Batch requests
- GIVEN multiple queries are batched by the client
- WHEN a single request hits `/api/trpc/obligation.list,contact.list`
- THEN both procedures execute and results are returned in a single response

### Requirement: Client-side tRPC provider
The system SHALL create `apps/dashboard/lib/trpc/client.ts` that exports a typed tRPC client using `createTRPCClient` with `httpBatchLink`. The link MUST inject the bearer token from the `dashboard_token` cookie into the `Authorization` header. The client SHALL integrate with the existing `QueryClientProvider` from `add-tanstack-query`.

#### Scenario: Client sends authenticated requests
- GIVEN a user is logged in with a `dashboard_token` cookie
- WHEN a tRPC query is made from a client component
- THEN the request includes `Authorization: Bearer <token>` header

#### Scenario: Client handles 401 responses
- GIVEN an expired or invalid token
- WHEN a tRPC request returns UNAUTHORIZED
- THEN the cookie is cleared and the user is redirected to `/login`

### Requirement: Server-side tRPC caller
The system SHALL create `apps/dashboard/lib/trpc/server.ts` that exports a server-side caller for use in React Server Components. The caller MUST read the auth cookie from `next/headers` and create a direct procedure call (no HTTP round-trip).

#### Scenario: RSC prefetch with server caller
- GIVEN a server component needs obligation data
- WHEN `trpc.obligation.list.prefetch()` is called in the RSC
- THEN the query is executed server-side and the result is dehydrated for client hydration

### Requirement: tRPC React integration
The system SHALL create `apps/dashboard/lib/trpc/react.tsx` that exports the tRPC React utilities including typed `trpc` proxy with `.queryOptions()`, `.mutationOptions()`, and `.queryKey()` methods. This MUST integrate with TanStack Query from `add-tanstack-query`.

#### Scenario: queryOptions usage
- GIVEN a client component needs to fetch obligations
- WHEN the component calls `useQuery(trpc.obligation.list.queryOptions({ status: "open" }))`
- THEN the query executes with full type safety and the result type is inferred from the procedure

#### Scenario: mutationOptions usage
- GIVEN a client component needs to create an obligation
- WHEN the component calls `useMutation(trpc.obligation.create.mutationOptions({ onSuccess: ... }))`
- THEN the mutation executes with typed input and the onSuccess callback receives the typed result

## MODIFIED Requirements

### Requirement: Middleware auth path update
The system SHALL update `apps/dashboard/middleware.ts` to recognize `/api/trpc/*` as an authenticated API path. The existing `/api/` auth block already covers this since `/api/trpc/` is a sub-path, but the auth exclusion list (`/api/auth/verify`, `/api/auth/logout`) MUST NOT interfere with tRPC's auth router.

#### Scenario: tRPC requests pass through middleware auth
- GIVEN auth is enabled via `DASHBOARD_TOKEN`
- WHEN a request hits `/api/trpc/obligation.list` with a valid bearer token
- THEN Next.js middleware passes the request through to the tRPC handler
