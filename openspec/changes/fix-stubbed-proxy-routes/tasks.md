# Implementation Tasks

<!-- beads:epic:nv-cm5a -->

## API Batch

- [x] [2.1] [P-1] Replace 501 stub in `apps/dashboard/app/api/config/route.ts` — GET proxies `daemonFetch("/api/config")`, PUT reads body and proxies `daemonFetch("/api/config", { method: "PUT", body, headers })`, both catch to 502 [owner:api-engineer]
- [x] [2.2] [P-1] Replace 501 stub in `apps/dashboard/app/api/projects/route.ts` — GET proxies `daemonFetch("/api/projects")`, catch to 502 [owner:api-engineer]
- [x] [2.3] [P-1] Add `GET /api/memory` handler to `crates/nv-daemon/src/http.rs` — no topic param returns `{ topics: [...] }` from memory dir listing; `?topic=<name>` returns `{ topic, content }` or 404 [owner:api-engineer]
- [x] [2.4] [P-1] Add `PUT /api/memory` handler to `crates/nv-daemon/src/http.rs` — accepts `{ topic, content }` body, writes via `Memory`, returns `{ topic, written: <bytes> }` [owner:api-engineer]
- [x] [2.5] [P-1] Expose memory base path or `Memory` instance in `HttpState` so the new memory handlers can access it (check `state.rs` — add field if not present) [owner:api-engineer]
- [x] [2.6] [P-1] Register `GET /api/memory` and `PUT /api/memory` in `build_router()` in `crates/nv-daemon/src/http.rs` [owner:api-engineer]
- [x] [2.7] [P-1] Add `POST /api/solve` handler to `crates/nv-daemon/src/http.rs` — accepts `{ project, error, context? }` body, returns `{ session_id: <uuid> }` (minimal implementation; full session wiring is out of scope) [owner:api-engineer]
- [x] [2.8] [P-1] Register `POST /api/solve` in `build_router()` in `crates/nv-daemon/src/http.rs` [owner:api-engineer]
- [x] [2.9] [P-1] Replace 501 stub in `apps/dashboard/app/api/memory/route.ts` — GET forwards optional `?topic=` param via `daemonFetch`, PUT reads body and proxies with method PUT, both catch to 502 [owner:api-engineer]
- [x] [2.10] [P-1] Replace 501 stub in `apps/dashboard/app/api/solve/route.ts` — POST reads body and proxies `daemonFetch("/api/solve", { method: "POST", body, headers })`, catch to 502 [owner:api-engineer]

## E2E Batch

- [x] [4.1] Add smoke tests in `http.rs` verifying `GET /api/memory` returns HTTP 200 with `{ topics: [...] }`, `PUT /api/memory` with valid body returns 200, and `POST /api/solve` returns 200 with `session_id` (following existing test harness pattern in the file) [owner:e2e-engineer]
