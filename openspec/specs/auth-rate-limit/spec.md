# auth-rate-limit Specification

## Purpose
TBD - created by archiving change add-http-api-auth. Update Purpose after archive.
## Requirements
### Requirement: Bearer token middleware guards non-health routes

`packages/daemon/src/middleware/auth.ts` MUST export a `bearerAuth(token: string)` Hono middleware factory. The middleware MUST return HTTP 401 with `{ error: "Unauthorized" }` when the `Authorization` header is absent or does not exactly match `Bearer <token>`. When the header matches, it MUST call `next()`. The `GET /health` endpoint MUST remain public — no auth check SHALL run for that path. At daemon startup, if `NV_API_TOKEN` is not set, `loadConfig` MUST throw with the message `"NV_API_TOKEN environment variable is required but not set."`.

#### Scenario: Missing Authorization header is rejected

Given a request to `POST /chat` with no `Authorization` header,
when the middleware runs,
then it returns HTTP 401 with `{ error: "Unauthorized" }` and the route handler is never called.

#### Scenario: Health endpoint bypasses auth

Given a request to `GET /health` with no `Authorization` header,
when the request is processed,
then it returns HTTP 200 with the health payload and no auth check runs.

### Requirement: Sliding-window rate limiter caps expensive endpoints at 10 req/min

`packages/daemon/src/middleware/rate-limit.ts` MUST export a `rateLimiter(routeKey: string, limit: number)` factory returning a Hono middleware. The middleware MUST use an in-memory `Map<string, number[]>` keyed by `routeKey`, prune timestamps older than 60 seconds on each request, and return HTTP 429 with `{ error: "Rate limit exceeded. Max <limit> requests per minute." }` when the timestamp count reaches or exceeds `limit`. Each endpoint (`/chat`, `/briefing/generate`, `/dream`) MUST have its own independent bucket with `limit: 10`.

#### Scenario: 11th request within 60 seconds is rejected

Given 10 requests to `POST /dream` within the last 60 seconds,
when the 11th request arrives,
then the rate limiter returns HTTP 429 and the route handler is never called.

#### Scenario: Window slide resets the counter

Given 10 requests to `POST /chat` made between t=0s and t=10s,
when a new request arrives at t=61s,
then all prior timestamps are pruned, the counter resets, and the request is allowed through.

### Requirement: CORS origin restricted to known dashboard origin

The wildcard `cors({ origin: "*" })` in `packages/daemon/src/http.ts` MUST be replaced with `cors({ origin: allowedOrigin, credentials: true })` where `allowedOrigin` is read from `process.env["NV_DASHBOARD_ORIGIN"]` defaulting to `"http://localhost:3101"`. The `credentials: true` option SHALL be set to allow the dashboard to send the `Authorization` header on cross-origin requests.

#### Scenario: Requests from the allowed origin succeed

Given `NV_DASHBOARD_ORIGIN` is set to `"https://nova.leonardoacosta.dev"`,
when the dashboard sends a credentialed request from that origin,
then the CORS headers permit the request and no preflight error occurs.

