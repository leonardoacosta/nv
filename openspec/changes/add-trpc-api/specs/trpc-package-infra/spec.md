# Spec: tRPC Package Infrastructure

## ADDED Requirements

### Requirement: @nova/api workspace package
The system SHALL create `packages/api/` as a new workspace package with tRPC v11, exporting the root `AppRouter` type and `RouterOutputs` / `RouterInputs` inference helpers.

#### Scenario: Package installation and exports
- GIVEN the monorepo with `packages/*` in workspace config
- WHEN `packages/api/` is created with `@trpc/server`, `@trpc/client`, `@trpc/react-query`, `zod`, and `@nova/db` as dependencies
- THEN `pnpm install` succeeds and `@nova/api` is resolvable from `apps/dashboard/`
- AND the package exports `appRouter`, `AppRouter`, `createTRPCContext`, `RouterOutputs`, `RouterInputs`

#### Scenario: tRPC initialization and context
- GIVEN a request with an `Authorization: Bearer <token>` header
- WHEN `createTRPCContext` is called with the request
- THEN the context contains the extracted token string
- AND `protectedProcedure` validates the token using timing-safe comparison against `DASHBOARD_TOKEN` env var

#### Scenario: Dev-mode auth bypass
- GIVEN `DASHBOARD_TOKEN` env var is unset or empty
- WHEN a request hits a `protectedProcedure`
- THEN the procedure executes without auth validation (matching current middleware behavior)

### Requirement: Auth middleware in packages/api
The system MUST replicate the timing-safe token comparison from `apps/dashboard/lib/auth.ts` inside `packages/api/src/trpc.ts` so the API package has zero dependency on `apps/dashboard/`. The `protectedProcedure` middleware SHALL read the token from context and validate it.

#### Scenario: Invalid token returns UNAUTHORIZED
- GIVEN a request with an invalid bearer token
- WHEN the request hits a `protectedProcedure`
- THEN tRPC returns a `TRPCError` with code `UNAUTHORIZED`

#### Scenario: Missing Authorization header
- GIVEN a request with no `Authorization` header and no cookie
- WHEN the request hits a `protectedProcedure`
- THEN tRPC returns a `TRPCError` with code `UNAUTHORIZED`

### Requirement: Fleet fetch helper
The system SHALL create `packages/api/src/lib/fleet.ts` exporting a `fleetFetch(service, path, init?)` function that MUST resolve fleet service URLs from environment variables with `host.docker.internal` defaults and include a 5-second timeout.

#### Scenario: Fleet service URL resolution
- GIVEN `MEMORY_SVC_URL` is set to `http://custom-host:4101`
- WHEN `fleetFetch("memory-svc", "/read")` is called
- THEN the request is sent to `http://custom-host:4101/read`

#### Scenario: Default URL fallback
- GIVEN no fleet env vars are set
- WHEN `fleetFetch("tool-router", "/health")` is called
- THEN the request is sent to `http://host.docker.internal:4100/health`
