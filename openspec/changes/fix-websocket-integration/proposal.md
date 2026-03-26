# Proposal: Fix WebSocket Integration

**Spec ID:** fix-websocket-integration
**Status:** complete
**Priority:** 1 (Critical)
**Created:** 2026-03-25

---

## Problem

The WebSocket integration between the dashboard and daemon is broken in production. The dashboard
runs as a Docker container accessible via Traefik at `nova.leonardoacosta.dev`. The daemon runs
as a systemd service on the host at port 3443.

### Root Cause

`DaemonEventContext.tsx` builds the WebSocket URL client-side using:

```ts
const host = process.env.NEXT_PUBLIC_DAEMON_WS_HOST ?? window.location.host;
return `${proto}://${host}/ws/events`;
```

When `NEXT_PUBLIC_DAEMON_WS_HOST` is not set (it is never set — it is not in `docker-compose.yml`
or any env file), this falls back to `window.location.host`, which is `nova.leonardoacosta.dev`.
The browser then attempts to open a WebSocket to `wss://nova.leonardoacosta.dev/ws/events`.

That path is NOT routed anywhere. Traefik has no rule to forward `/ws/events` from the dashboard
domain to the host daemon. The daemon's WebSocket endpoint (`GET /ws/events`) listens only on
the host at port 3443, which is unreachable directly from the browser.

The daemon port (3443) is a server-side implementation detail defined by:
- `config/nv.toml`: `health_port = 8400` (the daemon HTTP port is actually 8400 — the 3443 value
  in `docker-compose.yml` and `next.config.ts` is the agreed-upon daemon API port, separate from
  the internal health probe port; see `apps/dashboard/lib/daemon.ts` default)
- `DAEMON_URL=http://host.docker.internal:3443` is set in Docker Compose and used server-side
  only for Next.js rewrites (`/api/:path*` → daemon)

### Why Existing Rewrites Don't Help

Next.js rewrites (`/api/:path*` → `${DAEMON_URL}/api/:path*`) proxy HTTP traffic server-side.
WebSocket upgrades require a different mechanism. Next.js 15 App Router does not support
WebSocket proxying in `rewrites()`.

### What IS Working

- `DaemonEventContext.tsx` exists and is feature-complete
- It is correctly wrapped in `apps/dashboard/app/layout.tsx`
- `WsStatusDot` is rendered in `Sidebar.tsx` footer
- `useDaemonEvents` is wired into `page.tsx`, `sessions/page.tsx`, `sessions/[id]/page.tsx`,
  and `approvals/page.tsx`
- The daemon exposes `GET /ws/events` WebSocket endpoint on port 3443

The entire integration layer exists — only the URL routing is broken.

---

## Proposed Solution

### Option A: Next.js Custom Server WebSocket Proxy (Recommended)

Add a Next.js custom server (`server.ts`) that intercepts WebSocket upgrade requests on the path
`/ws/events` and pipes them through to the daemon via `http-proxy` or raw Node.js `http.request`
upgrade forwarding.

**Pros:**
- No Traefik changes needed
- No new env vars exposed to the browser
- WebSocket connection originates from `nova.leonardoacosta.dev/ws/events` — exactly what
  `DaemonEventContext.tsx` already expects when `NEXT_PUBLIC_DAEMON_WS_HOST` is unset
- Daemon URL stays server-side only (`DAEMON_URL=http://host.docker.internal:3443`)

**Cons:**
- Requires switching from Next.js standalone output with default server to a custom Node server,
  which adds a small amount of boilerplate

**Implementation sketch:**

```ts
// apps/dashboard/server.ts
import { createServer } from "http";
import { parse } from "url";
import next from "next";
import httpProxy from "http-proxy";

const app = next({ dev: process.env.NODE_ENV !== "production" });
const handle = app.getRequestHandler();
const proxy = httpProxy.createProxyServer();

const DAEMON_URL = process.env.DAEMON_URL ?? "http://127.0.0.1:3443";
const DAEMON_WS_URL = DAEMON_URL.replace(/^http/, "ws");

app.prepare().then(() => {
  const server = createServer((req, res) => {
    handle(req, res, parse(req.url!, true));
  });

  server.on("upgrade", (req, socket, head) => {
    if (req.url === "/ws/events") {
      proxy.ws(req, socket, head, { target: DAEMON_WS_URL });
    } else {
      socket.destroy();
    }
  });

  server.listen(3000);
});
```

The `DaemonEventContext.tsx` URL logic requires **zero changes** — it already falls back to
`window.location.host`, which routes through Traefik to the Next.js custom server, which
upgrades and proxies to the daemon.

### Option B: NEXT_PUBLIC_DAEMON_WS_HOST env var pointing to daemon directly

Set `NEXT_PUBLIC_DAEMON_WS_HOST=nova.leonardoacosta.dev` and add a Traefik TCP/websocket
passthrough rule specifically for `/ws/events` that routes directly to `host.docker.internal:3443`.

**Pros:** Simplest path if Traefik already supports it

**Cons:**
- Requires Traefik config changes (not in this repo)
- Exposes daemon port to Traefik routing — adds surface area
- Does not work for non-Traefik/local dev without extra configuration

### Decision

**Use Option A.** The custom server pattern is self-contained within the repo, works in all
environments (local dev and Docker production), and requires no infrastructure changes.

---

## Files Changed

| File | Change |
|------|--------|
| `apps/dashboard/server.ts` | New: custom Node server with WebSocket upgrade proxy |
| `apps/dashboard/package.json` | Add `http-proxy` and `@types/http-proxy` dev dep; update `start` script |
| `apps/dashboard/next.config.ts` | Set `output: "standalone"` remains; no change needed |
| `apps/dashboard/Dockerfile` | Update `CMD` to run `node server.js` instead of default standalone server |
| `apps/dashboard/app/api/ws/events/route.ts` | Optional: health-check stub returning 426 Upgrade Required |

### Files NOT changed

| File | Why |
|------|-----|
| `apps/dashboard/components/providers/DaemonEventContext.tsx` | Already correct — no changes |
| `apps/dashboard/app/layout.tsx` | Already wraps DaemonEventProvider — no changes |
| `apps/dashboard/components/Sidebar.tsx` | Already has WsStatusDot in footer — no changes |
| `apps/dashboard/app/page.tsx` | Already wired to useDaemonEvents — no changes |
| `apps/dashboard/app/sessions/page.tsx` | Already wired to useDaemonEvents — no changes |
| `apps/dashboard/app/approvals/page.tsx` | Already wired to useDaemonEvents — no changes |
| `docker-compose.yml` | DAEMON_URL already correct for host.docker.internal |

---

## Environment Variables

No new env vars needed. The custom server reads `DAEMON_URL` (already set in Docker Compose).

For local development without Docker, `DAEMON_URL` defaults to `http://127.0.0.1:3443` (already
the default in `apps/dashboard/lib/daemon.ts` and `next.config.ts`).

---

## Acceptance Criteria

1. Browser opens a WebSocket to `wss://nova.leonardoacosta.dev/ws/events` and receives a 101
   Upgrade response (not a 404 or connection refused)
2. Sidebar footer shows a green dot within 2 seconds of page load when the daemon is running
3. Sidebar footer shows a red dot when the daemon is stopped
4. Sidebar footer shows an amber pulsing dot during the reconnection backoff window
5. No regressions to existing HTTP API routes (`/api/*` still proxy correctly)
6. Docker build succeeds with the updated Dockerfile CMD

---

## Out of Scope

- Changing DaemonEventContext event subscription logic
- Adding new event types to the daemon
- Authentication on the WebSocket endpoint
- Monitoring/alerting for WebSocket disconnections
