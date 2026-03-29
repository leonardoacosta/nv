# Implementation Tasks

<!-- beads:epic:nv-add-http-api-auth -->

## API Batch

- [x] [1.1] [P-2] Create `packages/daemon/src/middleware/auth.ts` — export `bearerAuth(token: string): MiddlewareHandler` that checks `Authorization: Bearer <token>`, returns 401 with `{ error: "Unauthorized" }` on mismatch [owner:api-engineer]
- [x] [1.2] [P-2] Create `packages/daemon/src/middleware/rate-limit.ts` — export `rateLimiter(routeKey: string, limit: number): MiddlewareHandler` using an in-memory `Map<string, number[]>` sliding window (60s), returns 429 with `{ error: "Rate limit exceeded. Max <limit> requests per minute." }` when over limit [owner:api-engineer]
- [x] [1.3] [P-2] Extend `packages/daemon/src/config.ts` — add `apiToken: string` to `Config` interface, read `NV_API_TOKEN` env var in `loadConfig`, throw `Error("NV_API_TOKEN environment variable is required but not set.")` if absent, include in returned config object [owner:api-engineer]
- [x] [1.4] [P-2] Update `packages/daemon/src/http.ts` — add `apiToken: string` to `HttpServerDeps`; replace `cors({ origin: "*" })` with `cors({ origin: process.env["NV_DASHBOARD_ORIGIN"] ?? "http://localhost:3101", credentials: true })`; wire `bearerAuth` on `/chat`, `/briefing/*`, `/dream`, `/dream/status`; wire `rateLimiter` on `/chat`, `/briefing/generate`, `/dream` (limit 10) [owner:api-engineer]

## E2E Batch

- [x] [2.1] Build verification — run `pnpm --filter @nova/daemon typecheck` to confirm zero type errors after all changes [owner:api-engineer]
- [ ] [2.2] [user] Manual test — start daemon with `NV_API_TOKEN=test-token` set, verify `GET /health` returns 200 with no token, `POST /chat` without token returns 401, `POST /chat` with correct bearer token initiates SSE stream [owner:user]
- [ ] [2.3] [user] Manual test — send 11 rapid requests to `POST /dream` with a valid token, verify the 11th returns 429 [owner:user]
- [ ] [2.4] [user] Manual test — start daemon without `NV_API_TOKEN` set, verify daemon fails at startup with the descriptive error message [owner:user]
