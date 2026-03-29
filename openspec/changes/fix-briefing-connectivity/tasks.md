# Implementation Tasks

<!-- beads:epic:nv-53k -->

## API Batch

- [x] [2.1] [P-1] Create `apps/dashboard/app/api/briefing/stream/route.ts` — GET handler that reads `DAEMON_URL` from env (default `http://localhost:7700`), opens fetch to `${DAEMON_URL}/api/briefing/stream`, returns a streaming Response that pipes each SSE chunk to the client; on daemon connection failure return 503 with SSE error event `{ type: "error", message: "daemon_unavailable" }` [owner:api-engineer] [beads:nv-rqq5]
- [x] [2.2] [P-1] Update `apps/dashboard/app/briefing/page.tsx` — change EventSource URL from `${DAEMON_URL}/api/briefing/stream` to `/api/briefing/stream` (relative); remove `NEXT_PUBLIC_DAEMON_URL` env var usage and the `typeof window` conditional; keep the tRPC mutation fallback in `es.onerror` unchanged [owner:ui-engineer] [beads:nv-547p]

## UI Batch

- [ ] [3.1] [P-1] Update `docker-compose.yml` — change `DAEMON_URL=ws://172.20.0.1:8400` to `DAEMON_URL=http://172.20.0.1:7700` [owner:devops-engineer] [beads:nv-u4p2]

## E2E Batch

- [ ] [4.1] Rebuild and restart dashboard container (`docker compose up -d --build`), verify briefing page loads without "fetch failed", verify "Generate Now" streams blocks via SSE through the proxy [owner:e2e-engineer] [beads:nv-hrhk]
