# Proposal: Fix Daemon URL Port (3443 → 8400)

## Change ID
`fix-daemon-url-port`

## Summary
Correct all hardcoded references to daemon port 3443 so they point to the actual daemon HTTP
port 8400, eliminating the 502 errors that currently affect every dashboard API proxy call and
the Traefik WebSocket route.

## Context
- Extends: `docker-compose.yml` (DAEMON_URL env var), `apps/dashboard/lib/daemon.ts` (default
  fallback URL), `~/dev/hl/homelab/traefik/dynamic/routes.yml` (nova-daemon service definition)
- Related: `fix-dashboard-api-proxy` spec (fixed proxy route paths — this fixes the underlying
  transport layer that was also misconfigured)

## Motivation
The `extract-nextjs-dashboard` spec that introduced the Docker deployment assumed port 3443 for
the NV daemon HTTP server. In reality:

- `crates/nv-daemon/src/main.rs` defaults to port **8400** (`NV_HEALTH_PORT` env var, fallback
  `.unwrap_or(8400)`)
- `crates/nv-core/src/config.rs` defines the default health port as **8400**
- `deploy/install.sh` sets `HEALTH_PORT="${NV_HEALTH_PORT:-8400}"`

Port 3443 does not exist — no listener binds to it. Every `daemonFetch()` call made from a
Docker container therefore receives a connection refused, which Next.js converts to a 502. This
affects every single dashboard page that makes an API call.

## Requirements

### Req-1: Fix docker-compose.yml DAEMON_URL

The `DAEMON_URL` environment variable passed to the dashboard container must use port 8400 so
that `daemonFetch` inside the container reaches the host daemon.

**Current value:**
```
DAEMON_URL=http://host.docker.internal:3443
```

**Required value:**
```
DAEMON_URL=http://host.docker.internal:8400
```

#### Scenario: Dashboard container proxies reach the daemon

Given the dashboard container is running via `docker compose up`,
when any API route handler calls `daemonFetch("/api/messages")`,
then the HTTP request reaches the daemon at `host.docker.internal:8400` and returns a non-502
response.

### Req-2: Fix apps/dashboard/lib/daemon.ts default fallback

The fallback URL used when `DAEMON_URL` is not set (local dev without Docker) must also use
port 8400. This ensures `pnpm dev` in `apps/dashboard` works out of the box on a machine
running the daemon.

**Current value:**
```typescript
export const DAEMON_URL =
  process.env.DAEMON_URL ?? "http://127.0.0.1:3443";
```

**Required value:**
```typescript
export const DAEMON_URL =
  process.env.DAEMON_URL ?? "http://127.0.0.1:8400";
```

#### Scenario: Local dev without Docker resolves daemon URL correctly

Given a developer runs `pnpm dev` in `apps/dashboard` without setting `DAEMON_URL`,
when a page makes a `daemonFetch("/health")` call,
then the request targets `http://127.0.0.1:8400/health` (matching the running daemon).

### Req-3: Fix Traefik routes.yml nova-daemon service URL

The Traefik file provider defines a `nova-daemon` service for the WebSocket proxy. Its URL
must use port 8400 so that `wss://nova.leonardoacosta.dev/ws/...` connections are forwarded
to the daemon's actual HTTP/WS port.

**Current value (in `~/dev/hl/homelab/traefik/dynamic/routes.yml`):**
```yaml
nova-daemon:
  loadBalancer:
    servers:
      - url: "http://172.20.0.1:3443"
```

**Required value:**
```yaml
nova-daemon:
  loadBalancer:
    servers:
      - url: "http://172.20.0.1:8400"
```

#### Scenario: WebSocket connections reach the daemon

Given Traefik is running with the updated routes.yml,
when a browser connects to `wss://nova.leonardoacosta.dev/ws/events`,
then Traefik proxies the connection to `172.20.0.1:8400` where the daemon's WS handler
is listening, and the connection upgrades successfully.

## Scope
- **IN**: `docker-compose.yml` DAEMON_URL env var; `apps/dashboard/lib/daemon.ts` fallback
  default; `~/dev/hl/homelab/traefik/dynamic/routes.yml` nova-daemon service URL
- **OUT**: Any changes to daemon source code; adding or modifying NV_HEALTH_PORT handling;
  changes to other Traefik routes or services; changes to any other dashboard files

## Impact
| Area | Change |
|------|--------|
| `docker-compose.yml` | 1 env var value: `:3443` → `:8400` |
| `apps/dashboard/lib/daemon.ts` | 1 string literal: `3443` → `8400` |
| `~/dev/hl/homelab/traefik/dynamic/routes.yml` | 1 URL value: `3443` → `8400` |
| All dashboard API proxies | Unblocked — no longer 502 |
| WebSocket `/ws/` routes | Unblocked — correct backend port |

## Risks
| Risk | Mitigation |
|------|-----------|
| Traefik routes.yml is in a separate repo (`hl/homelab`) | Task must include that repo — note the file path clearly |
| `docker compose` requires restart after env var change | Document in tasks: `docker compose down && docker compose up -d` after applying |
| If a future operator intentionally runs daemon on 3443 | They must set `NV_HEALTH_PORT=3443` and `DAEMON_URL=http://host.docker.internal:3443` explicitly — this spec removes the incorrect default |
