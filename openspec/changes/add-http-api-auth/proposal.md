# Proposal: Add HTTP API Auth and Rate Limiting

## Change ID
`add-http-api-auth`

## Summary

Secure the Nova daemon HTTP API (`packages/daemon/src/http.ts`) with bearer token
authentication, per-endpoint rate limiting, and a restricted CORS origin. Currently
`cors({ origin: "*" })` is applied with no auth, allowing any host on the network to
trigger Agent SDK sessions, briefing generation, and dream cycles.

## Context

- Phase: Hardening | Wave: current
- Stack: TypeScript (Hono v4, `@hono/node-server`)
- Modifies: `packages/daemon/src/http.ts`, `packages/daemon/src/config.ts`
- Depends on: existing HTTP API (`add-http-api` — archived, fully applied)
- Related: dashboard CORS origin is `http://localhost:3101` (dev) /
  `https://nova.leonardoacosta.dev` (prod); `NV_API_TOKEN` joins the existing env var set

## Motivation

Three unauthenticated endpoints create two distinct risks:

1. **Unauthorized spend** — `POST /chat`, `POST /briefing/generate`, and `POST /dream`
   each invoke Claude API calls. Anyone on the same network segment (Tailscale or LAN)
   can exhaust the Claude API budget with no credential requirement.
2. **Runaway client loops** — a buggy dashboard reload loop or a misconfigured automation
   can fire hundreds of requests per minute. Without rate limiting there is no circuit breaker.

The fix is minimal: bearer token middleware guards all non-health routes, an in-memory
sliding-window rate limiter caps the three expensive endpoints at 10 req/min, and CORS is
narrowed to the known dashboard origin.

## Requirements

### Req-1: Bearer Token Authentication Middleware

Create `packages/daemon/src/middleware/auth.ts` exporting a single Hono middleware factory
`bearerAuth(token: string)`.

Behaviour:
- Read the `Authorization` header on each request.
- If the header is absent or does not match `Bearer <token>` exactly, return
  `{ error: "Unauthorized" }` with HTTP 401.
- If the header matches, call `next()`.

The token value is sourced at startup from the `NV_API_TOKEN` environment variable. If the
variable is not set the daemon MUST throw at startup with a descriptive error message
(fail-fast, do not silently skip auth).

Registration in `http.ts`:
- Apply `bearerAuth` after `cors` and `secureHeaders` but before any route handlers.
- Apply it on the pattern `"/chat"`, `"/briefing/*"`, `"/dream"`, and `"/dream/status"`.
- `GET /health` remains public (no auth check).

#### Scenario: Missing Authorization header

Given a request to `POST /chat` with no `Authorization` header,
when the middleware runs,
then it returns HTTP 401 with `{ error: "Unauthorized" }` and the route handler is never
called.

#### Scenario: Wrong token

Given a request to `POST /dream` with `Authorization: Bearer wrong-token`,
when the middleware runs,
then it returns HTTP 401 and the route handler is never called.

#### Scenario: Correct token

Given a request to `POST /briefing/generate` with `Authorization: Bearer <NV_API_TOKEN>`,
when the middleware runs,
then it calls `next()` and the route handler executes normally.

#### Scenario: Health endpoint is public

Given a request to `GET /health` with no `Authorization` header,
when the request is processed,
then it returns HTTP 200 with the health payload. No auth check runs.

#### Scenario: Missing NV_API_TOKEN at startup

Given `NV_API_TOKEN` is not set in the environment,
when `createHttpApp` is called,
then it throws an `Error` before the Hono app is returned.

### Req-2: In-Memory Rate Limiter

Implement a simple sliding-window rate limiter in
`packages/daemon/src/middleware/rate-limit.ts`.

No external package is needed. Use a `Map<string, number[]>` keyed by endpoint path (or
`routeKey` string) mapping to an array of recent request timestamps. On each request:

1. Prune timestamps older than 60 seconds from the entry.
2. If the entry now has `>= limit` timestamps, return HTTP 429 with
   `{ error: "Rate limit exceeded. Max <limit> requests per minute." }`.
3. Otherwise push `Date.now()` and call `next()`.

Export `rateLimiter(routeKey: string, limit: number)` — a factory that returns a Hono
middleware bound to the given key and limit.

The key is the string used as the Map key; callers pass the route path literal so each
endpoint has its own independent bucket.

Apply in `http.ts` to the three expensive endpoints with `limit: 10`:

```
POST /chat               → rateLimiter("/chat", 10)
POST /briefing/generate  → rateLimiter("/briefing/generate", 10)
POST /dream              → rateLimiter("/dream", 10)
```

`GET /dream/status` does not need rate limiting (read-only, cheap).

#### Scenario: Under the limit

Given 9 requests to `POST /dream` within the last 60 seconds,
when the 10th request arrives,
then it is allowed through (count is exactly at limit, not over).

#### Scenario: Over the limit

Given 10 requests to `POST /dream` within the last 60 seconds,
when the 11th request arrives,
then the rate limiter returns HTTP 429 with the error message and the route handler is never
called.

#### Scenario: Window slides

Given 10 requests to `POST /chat` made between t=0s and t=10s,
when a new request arrives at t=61s (all prior timestamps are now older than 60 seconds),
then the window is empty, the counter resets, and the request is allowed.

#### Scenario: Independent buckets

Given 10 requests to `POST /chat` within 60 seconds,
when a request arrives for `POST /dream`,
then the `/dream` bucket is unaffected and the request is allowed.

### Req-3: CORS Origin Restriction

Replace the wildcard CORS configuration in `http.ts`:

```typescript
// Before
app.use("*", cors({ origin: "*" }));

// After
const allowedOrigin =
  process.env["NV_DASHBOARD_ORIGIN"] ?? "http://localhost:3101";
app.use("*", cors({ origin: allowedOrigin, credentials: true }));
```

The `NV_DASHBOARD_ORIGIN` env var allows overriding the default for production deployments
without a code change. `credentials: true` is required because the dashboard will send the
`Authorization` header with cross-origin requests.

No new config struct field is required. Read the env var inline in `createHttpApp`.

### Req-4: Config Extension — NV_API_TOKEN

Extend `packages/daemon/src/config.ts`:

- Add `apiToken: string` to the `Config` interface (required, not optional).
- In `loadConfig`, read `process.env["NV_API_TOKEN"]`. If absent, throw:
  ```
  Error: "NV_API_TOKEN environment variable is required but not set."
  ```
- Pass `config.apiToken` into `createHttpApp` via the existing `HttpServerDeps` interface:
  add `apiToken: string` to `HttpServerDeps`.

This keeps the token out of env-var reading inside middleware and makes it testable.

### Req-5: Middleware Wiring in http.ts

Final middleware order in `createHttpApp`:

```
1. cors({ origin: allowedOrigin, credentials: true })          ← Req-3
2. secureHeaders()                                             ← existing
3. bearerAuth(deps.apiToken) on non-health routes              ← Req-1
4. rateLimiter per expensive route                             ← Req-2
```

The global error handler remains unchanged and is registered after middleware setup.

No existing route handler logic changes. The auth and rate-limit concerns are additive
middleware layers only.

## Scope

- **IN**: `packages/daemon/src/middleware/auth.ts` (new), `packages/daemon/src/middleware/rate-limit.ts` (new), `packages/daemon/src/http.ts` (CORS + middleware wiring), `packages/daemon/src/config.ts` (add `apiToken`)
- **OUT**: Persistent rate-limit storage (Redis, DB), per-IP keying, token rotation, OAuth, JWT, any dashboard changes, any other Hono app in the repo

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/middleware/auth.ts` | New — bearerAuth factory |
| `packages/daemon/src/middleware/rate-limit.ts` | New — sliding-window rateLimiter factory |
| `packages/daemon/src/http.ts` | CORS narrowed, auth + rate-limit middleware wired |
| `packages/daemon/src/config.ts` | `Config.apiToken: string` added, startup validation |

## Risks

| Risk | Mitigation |
|------|-----------|
| Existing dashboard clients break if `NV_API_TOKEN` is not set before deployment | Fail-fast at daemon startup gives an immediate, actionable error rather than silent 401s; set the var in the deployment env before rolling out |
| In-memory rate limit resets on daemon restart | Acceptable for a local single-process daemon; a restart clears any abuse anyway. Document if persistent limiting is needed later |
| `NV_DASHBOARD_ORIGIN` mismatch in prod causes CORS failures | Default is localhost for local dev; prod must set the var. Document in deployment notes |
| bearerAuth applied too broadly accidentally blocks health checks | Health check exclusion is explicit in the route pattern list (Req-1); verify with a scenario test |
