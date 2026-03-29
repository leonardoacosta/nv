# Proposal: Fix Briefing Connectivity

## Change ID
`fix-briefing-connectivity`

## Summary
Fix the "fetch failed" error on the briefing page by correcting the Docker DAEMON_URL and routing the client-side SSE stream through a Next.js API proxy instead of hitting the daemon directly from the browser.

## Context
- Extends: `docker-compose.yml` (DAEMON_URL env var)
- Extends: `apps/dashboard/app/briefing/page.tsx` (SSE EventSource connection)
- Extends: `packages/api/src/routers/briefing.ts` (tRPC mutation uses DAEMON_URL)
- New: `apps/dashboard/app/api/briefing/stream/route.ts` (SSE proxy)
- Related: `add-briefing-cron` (completed — original briefing pipeline)
- Related: `consolidate-briefing-pipeline` (active — subsumes generative-ui, unrelated to connectivity)

## Motivation
The briefing page shows "Failed to load briefing / fetch failed" because two connectivity paths are broken:

1. **Server-side (tRPC → daemon):** `docker-compose.yml` sets `DAEMON_URL=ws://172.20.0.1:8400` but the briefing router makes an HTTP `fetch()` call — `ws://` is not a valid fetch protocol, and port 8400 is wrong (daemon listens on 7700).

2. **Client-side (SSE stream):** The browser opens `EventSource("http://localhost:7700/api/briefing/stream")` because `NEXT_PUBLIC_DAEMON_URL` is unset. The dashboard is accessed via `nova.leonardoacosta.dev` (Traefik), so `localhost:7700` from the browser resolves to nothing. The daemon is not exposed through Traefik.

## Requirements

### Req-1: Fix DAEMON_URL in docker-compose.yml
Change `DAEMON_URL=ws://172.20.0.1:8400` to `DAEMON_URL=http://172.20.0.1:7700` so the tRPC briefing.generate mutation can reach the daemon via HTTP fetch.

### Req-2: Add server-side SSE proxy route
Create `apps/dashboard/app/api/briefing/stream/route.ts` that proxies the SSE stream from the daemon server-side. The browser connects to `/api/briefing/stream` on the dashboard (routed through Traefik), and the Next.js route forwards to `${DAEMON_URL}/api/briefing/stream` on the host network.

### Req-3: Update client to use proxied SSE
Change the EventSource URL in `briefing/page.tsx` from `${DAEMON_URL}/api/briefing/stream` (direct daemon) to `/api/briefing/stream` (relative, hits the Next.js proxy). Remove the `NEXT_PUBLIC_DAEMON_URL` env var usage entirely — the client should never need to know the daemon's address.

## Scope
- **IN**: docker-compose DAEMON_URL fix, SSE proxy route, client EventSource URL fix
- **OUT**: Daemon systemd changes (already enabled as nova-ts.service), briefing content/rendering changes, Traefik config changes

## Impact
| Area | Change |
|------|--------|
| `docker-compose.yml` | Fix DAEMON_URL protocol and port |
| `apps/dashboard/app/api/briefing/stream/route.ts` | New SSE proxy route |
| `apps/dashboard/app/briefing/page.tsx` | Switch EventSource to relative URL |

## Risks
| Risk | Mitigation |
|------|-----------|
| SSE proxy adds latency | Negligible — same host, Docker bridge network |
| Daemon URL changes break other consumers | Grep confirms only briefing.ts and chat/send use DAEMON_URL — both benefit from the fix |
| Container rebuild required | Standard `docker compose up -d --build` after merge |
