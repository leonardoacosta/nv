# Proposal: Redesign Integrations as Service Status Page

## Change ID
`redesign-integrations-status`

## Summary

Replace the misleading Integrations page (which shows env var names with fake "Connected" badges
derived from config key presence) with a real Service Status page that shows actual connectivity
health across three categories: Channels, Fleet Services, and Infrastructure.

## Context
- Replaces: `dashboard/src/pages/IntegrationsPage.tsx`, `dashboard/src/components/IntegrationCard.tsx`, `dashboard/src/components/ConfigureModal.tsx`
- Consumes: Rust daemon `/api/server-health`, tool-router `:4100/health` + `/registry`, channels-svc `:4103/channels`
- Depends on: Rust daemon proxy route (new) to relay fleet health from host network into Docker
- Related: v10 tool fleet architecture (9 services + router on ports 4100-4109)

## Motivation

The current Integrations page is misleading. It reads `/api/config`, extracts key names (like
`telegram`, `discord`), and shows "Connected" badges based purely on whether the key has a truthy
value. This has nothing to do with actual connectivity. A service could be configured but
unreachable, or the token could be expired.

The page also implies users can configure third-party integrations (configure modal, env var
editing), which is not the purpose of Nova. Nova's services are operator-managed, not user-managed.

A real status page provides:
1. **Honest health** -- green/yellow/red based on actual reachability checks
2. **Useful metadata** -- port, uptime, tool count, channel direction
3. **Network topology awareness** -- the dashboard runs in Docker and cannot reach fleet services
   on 127.0.0.1, so fleet health must be proxied through the Rust daemon on the host

## Requirements

### Req-1: Rename Route and Navigation

Rename the route from `/integrations` to `/status`. Update the Sidebar nav item label from
"Integrations" to "Status" and change the icon from `Plug` to `Activity` (from lucide-react).

### Req-2: New API Endpoint on Rust Daemon

Add `GET /api/fleet-health` to the Rust daemon (`crates/nv-daemon/src/http.rs`). This endpoint
proxies `GET http://127.0.0.1:4100/health` from the tool-router (which aggregates health from
all 9 fleet services) and `GET http://127.0.0.1:4103/channels` from channels-svc. Returns a
combined JSON response:

```json
{
  "fleet": {
    "status": "healthy|degraded|unhealthy",
    "services": {
      "memory-svc": { "status": "healthy|unreachable", "url": "http://127.0.0.1:4101", "latency_ms": 12 },
      ...
    },
    "healthy_count": 9,
    "total_count": 9
  },
  "channels": [
    { "name": "telegram", "status": "connected", "direction": "bidirectional" },
    ...
  ]
}
```

The daemon runs on the host network and can reach all fleet services directly. The dashboard
(in Docker) calls `/api/fleet-health` through the existing Vite proxy to `:3443`.

Timeout: 5s for the fleet health call, 3s for channels. On timeout or connection refused, return
the structure with `"unreachable"` status for the failing component.

### Req-3: Add Fleet Registry to API

Add `GET /api/fleet-registry` to the Rust daemon. Proxies `GET http://127.0.0.1:4100/registry`.
Returns the tool-to-service mapping so the dashboard can display tool counts per service. Cache
the response in memory for 60s (the registry is static at runtime).

### Req-4: Service Status Page Layout

Replace `IntegrationsPage` with `StatusPage`. Three sections, rendered as vertical lists with
status dots (not cards with badges):

**Channels** -- from `/api/fleet-health` channels array:
- Row per channel: status dot (green=connected, red=disconnected, yellow=error), channel name,
  direction badge (bidirectional/inbound/outbound)

**Fleet Services** -- from `/api/fleet-health` fleet object + `/api/fleet-registry`:
- Row per service: status dot (green=healthy, red=unreachable), service name, port number,
  latency in ms, tool count (from registry)
- Aggregate status line at top: "9/9 healthy" or "7/9 healthy (2 unreachable)"

**Infrastructure** -- from existing `/api/server-health`:
- Postgres: show daemon health status (the daemon connects to Postgres, so if the daemon is
  healthy, Postgres is reachable)
- Daemon: uptime from server-health response

### Req-5: Service Detail Expansion

Clicking a fleet service row expands it inline (not a modal) to show:
- Port and base URL
- Uptime (from service health, if available via future enhancement -- for now show latency)
- Tool list (names from registry)
- Last health check timestamp (show relative time, e.g. "2s ago")

### Req-6: Auto-Refresh

Poll `/api/fleet-health` every 30 seconds when the Status page is active. Show a subtle
"Last checked: Xs ago" indicator next to the refresh button. Stop polling when the user navigates
away (cleanup in useEffect).

### Req-7: Remove Obsolete Components

Delete `IntegrationCard.tsx` and `ConfigureModal.tsx`. These are not used anywhere else and
represent the fake integration pattern being replaced.

## Scope
- **IN**: Page redesign, new daemon proxy endpoints, real health display, channel status, fleet
  status, inline service detail, auto-refresh
- **OUT**: Configuring services from the UI, adding new services, fleet service uptime tracking
  (deferred -- would require persistent health snapshots in meta-svc), direct fleet-to-dashboard
  communication (blocked by Docker network)

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/http.rs` | Add `GET /api/fleet-health` and `GET /api/fleet-registry` handlers with proxy logic |
| `dashboard/src/pages/IntegrationsPage.tsx` | Delete, replaced by `StatusPage.tsx` |
| `dashboard/src/pages/StatusPage.tsx` | New page: three-section status view with auto-refresh |
| `dashboard/src/components/IntegrationCard.tsx` | Delete (obsolete) |
| `dashboard/src/components/ConfigureModal.tsx` | Delete (obsolete) |
| `dashboard/src/components/ServiceRow.tsx` | New component: expandable row with status dot, metadata, tool list |
| `dashboard/src/components/ChannelRow.tsx` | New component: channel status row |
| `dashboard/src/App.tsx` | Update route `/integrations` to `/status`, update import |
| `dashboard/src/components/Sidebar.tsx` | Rename nav item, change icon |
| `dashboard/src/types/api.ts` | Add `FleetHealthResponse`, `FleetRegistryResponse` types |

## Risks
| Risk | Mitigation |
|------|-----------|
| Fleet services not running (dev machine off) | Show "unreachable" gracefully, not an error page; the page still shows channels and infra |
| Proxy adds latency to dashboard load | Fleet health is not on critical path; page renders immediately with loading skeleton, fleet data fills in async |
| Tool-router unavailable | Daemon proxy returns `{ fleet: { status: "unhealthy", services: {}, healthy_count: 0, total_count: 0 } }` with empty services |
| Channels-svc unavailable | Return empty channels array; dashboard shows "No channel data available" |
