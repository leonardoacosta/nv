# Implementation Tasks

<!-- beads:epic:nv-y6hj -->

## UI Batch

- [x] [3.1] [P-1] Add `app/api/briefing/route.ts` — GET proxy to daemon `/api/briefing` using `daemonFetch` [owner:ui-engineer]
- [x] [3.2] [P-1] Add `app/api/briefing/history/route.ts` — GET proxy forwarding `limit` query param to daemon `/api/briefing/history` [owner:ui-engineer]
- [x] [3.3] [P-1] Add `app/api/cold-starts/route.ts` — GET proxy forwarding `limit` query param to daemon `/api/cold-starts` [owner:ui-engineer]
- [x] [3.4] [P-1] Add `app/api/stats/route.ts` — GET proxy to daemon `/stats` (note: no `/api/` prefix on daemon side) [owner:ui-engineer]
- [x] [3.5] [P-1] Update `/usage` page: change `fetch("/stats")` to `fetch("/api/stats")` to match new proxy path [owner:ui-engineer]
- [x] [3.6] [P-1] Add `app/api/approvals/[id]/approve/route.ts` — POST proxy forwarding JSON body to daemon `/api/approvals/{id}/approve` [owner:ui-engineer]
- [x] [3.7] [P-1] Add `app/api/contacts/route.ts` — GET proxy forwarding `relationship` and `q` query params; POST proxy forwarding JSON body; both to daemon `/api/contacts` [owner:ui-engineer]
- [x] [3.8] [P-1] Add `app/api/contacts/[id]/route.ts` — GET, PUT (forward body), DELETE proxy to daemon `/api/contacts/{id}` [owner:ui-engineer]
- [x] [3.9] [P-1] Add `app/api/diary/route.ts` — GET proxy forwarding `date` and `limit` query params to daemon `/api/diary` [owner:ui-engineer]
- [x] [3.10] [P-1] Fix `app/api/server-health/route.ts` — change daemon target from `/api/server-health` to `/health` [owner:ui-engineer]
- [x] [3.11] [P-2] Stub `app/api/memory/route.ts` — return 501 `{ "error": "Not implemented — daemon endpoint pending" }` for GET and PUT [owner:ui-engineer]
- [x] [3.12] [P-2] Stub `app/api/config/route.ts` — return 501 `{ "error": "Not implemented — daemon endpoint pending" }` for GET and PUT [owner:ui-engineer]
- [x] [3.13] [P-2] Stub `app/api/projects/route.ts` — return 501 `{ "error": "Not implemented — daemon endpoint pending" }` for GET [owner:ui-engineer]
- [x] [3.14] [P-2] Stub `app/api/sessions/route.ts` — return 501 `{ "error": "Not implemented — daemon endpoint pending" }` for GET [owner:ui-engineer]
- [x] [3.15] [P-2] Stub `app/api/solve/route.ts` — return 501 `{ "error": "Not implemented — daemon endpoint pending" }` for POST [owner:ui-engineer]
