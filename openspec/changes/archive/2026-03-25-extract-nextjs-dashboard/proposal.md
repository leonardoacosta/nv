# Proposal: Extract Next.js Dashboard

## Change ID
`extract-nextjs-dashboard`

## Summary

Extract the embedded React SPA from `nv-daemon` into a standalone Next.js 15 app at `apps/dashboard/`, served by its own Docker container and accessible via Tailscale. The Rust daemon is stripped down to a minimal health endpoint only — all dashboard API routes move to Next.js API route handlers that call the daemon's underlying data stores and services directly.

## Context

- Phase: Wave 2a — foundation for Wave 2b (cc-session-management, add-morning-briefing-page, add-cold-start-logging, rebuild-dashboard-wireframes)
- Current implementation: `crates/nv-daemon/src/dashboard.rs` (793 lines), serving a Vite/React SPA via `rust_embed` from `dashboard/dist/`
- Related beads: nv-4zs (dashboard-wireframe-drift), nv-t4b (extract-nv-dashboard)
- Ideas that feed into this dashboard: nv-tft, nv-0n8, nv-967, nv-e34, nv-8y9, nv-9p9, nv-jea, nv-x3m, nv-42e

## Motivation

The current architecture bakes the compiled dashboard bundle into the Rust binary at compile time via `rust_embed`. This creates three compounding problems:

1. **Slow iteration.** Every UI change requires a full `cargo build` cycle on `nv-daemon`, which rebuilds the daemon even when only a TypeScript file changed.
2. **Tight coupling.** The dashboard API lives in Rust but its consumer is TypeScript. Type mismatches between the handler and the frontend are caught only at runtime.
3. **No room to grow.** Wave 2b specs need server-side rendering, streaming, and rich session management — none of which are possible with a static SPA embedded in a binary.

Moving to a standalone Next.js app decouples the deployment lifecycle, enables SSR/streaming, and eliminates RustEmbed compile overhead.

## Architecture

```
Tailscale network
  └─ apps/dashboard  (Next.js 15, port 3000)   ← this spec
       ├─ /api/*     Next.js Route Handlers
       └─ pages/     React + Tailwind (cosmic theme)

  └─ nv-daemon       (Axum, port 3443)
       └─ /health    minimal health endpoint only
```

The dashboard container reaches the daemon via Tailscale IP (or `host.docker.internal` in dev). All data access that previously went through `DashboardState` in Rust is now done directly from Next.js Route Handlers: file reads for memory, TOML reads/writes for config, SQLite reads for server-health and stats, and HTTP proxying to the Nexus gRPC-over-HTTP client for sessions.

For the initial extraction, the Next.js API routes **proxy** to the daemon's internal services via the daemon's existing `/api/*` endpoints — this lets both systems coexist during migration. The Rust API routes are stripped in a follow-on cleanup task after the Next.js app is verified.

## Requirements

### Req-1: Scaffold `apps/dashboard/`

Create a Next.js 15 app using the App Router at `apps/dashboard/`. The directory is a peer of `crates/` in the workspace root — this is a Rust workspace, not a monorepo, so there is no root `package.json` to extend.

```
apps/dashboard/
  app/
    layout.tsx          — root layout, loads Geist fonts, cosmic CSS vars
    page.tsx            — DashboardPage (port from dashboard/src/pages/)
    obligations/page.tsx
    projects/page.tsx
    nexus/page.tsx
    integrations/page.tsx
    usage/page.tsx
    memory/page.tsx
    settings/page.tsx
    api/
      obligations/route.ts
      obligations/[id]/route.ts
      projects/route.ts
      sessions/route.ts
      solve/route.ts
      memory/route.ts
      config/route.ts
      server-health/route.ts
  components/
    Sidebar.tsx
    SessionCard.tsx
    ObligationItem.tsx
    ProjectAccordion.tsx
    ActiveSession.tsx
    IntegrationCard.tsx
    ConfigureModal.tsx
    MemoryPreview.tsx
    NovaMark.tsx
    NovaBadge.tsx
    LeoBadge.tsx
    UsageSparkline.tsx
    MiniChart.tsx
    ServerHealth.tsx
  lib/
    daemon.ts           — base URL config + typed fetch helpers
  types/
    api.ts              — port of dashboard/src/types/api.ts verbatim
  globals.css
  next.config.ts
  tailwind.config.ts    — port cosmic theme tokens verbatim
  tsconfig.json
  package.json
```

### Req-2: API Route Handlers (proxy mode)

Each Next.js API route proxies to the daemon at `DAEMON_URL` (env var, default `http://127.0.0.1:3443`). This proxy approach keeps both systems live during cutover with zero data duplication.

| Next.js Route | Method | Proxies to daemon |
|---|---|---|
| `/api/obligations` | GET | `GET /api/obligations` |
| `/api/obligations/[id]` | PATCH | `PATCH /api/obligations/:id` |
| `/api/projects` | GET | `GET /api/projects` |
| `/api/sessions` | GET | `GET /api/sessions` |
| `/api/solve` | POST | `POST /api/solve` |
| `/api/memory` | GET, PUT | `GET /api/memory`, `PUT /api/memory` |
| `/api/config` | GET, PUT | `GET /api/config`, `PUT /api/config` |
| `/api/server-health` | GET | `GET /api/server-health` |

All proxied responses forward status codes and JSON bodies without transformation. Error responses from the daemon are passed through unchanged.

The daemon URL is configured via `DAEMON_URL` in `.env.local` (dev) and Docker env at runtime.

### Req-3: Migrate All UI Pages

Port all 8 pages and all 13 components from `dashboard/src/` to `apps/dashboard/`. The migration is a direct port — no redesign, no feature changes, no new pages. The existing cosmic design system (colors, fonts, border radii, shadows) is preserved exactly by copying `tailwind.config.ts` token values.

Path alias `@/` maps to `apps/dashboard/` in `tsconfig.json`, matching the existing SPA convention.

### Req-4: Tailwind + Geist Fonts

Port `tailwind.config.ts` cosmic color tokens verbatim:
- `cosmic.purple` = `#7C3AED`
- `cosmic.rose` = `#F43F5E`
- `cosmic.dark` = `#0F0B1A`
- `cosmic.surface` = `#1A1425`
- `cosmic.border` = `#2D2640`
- `cosmic.muted` = `#6B5B8A`
- `cosmic.text` = `#E8E0F0`
- `cosmic.bright` = `#F5F0FF`

Load Geist Sans Variable and Geist Mono Variable via `next/font/google` (or local font if Geist is not in the Google Fonts directory — check at implementation time; fallback to `geist` npm package import).

### Req-5: Docker Container

Dockerfile at `apps/dashboard/Dockerfile` using multi-stage build:
- Stage 1: `node:22-alpine` — install deps, build Next.js
- Stage 2: `node:22-alpine` — copy `.next/standalone` output + `public/`

Container listens on port 3000. `DAEMON_URL` injected at runtime via Docker env. The container does not need to reach the daemon at build time.

`docker-compose.yml` (or equivalent entry in the project's existing compose file if one exists) adds a `dashboard` service:
```yaml
dashboard:
  build: apps/dashboard
  ports:
    - "3000:3000"
  environment:
    - DAEMON_URL=http://nv-daemon:3443
  depends_on:
    - nv-daemon
  restart: unless-stopped
```

Check whether the project has an existing `docker-compose.yml` before creating a new one.

### Req-6: Strip Rust Dashboard Code

After the Next.js app is verified:
1. Remove the `DashboardAssets` RustEmbed struct and `dashboard/dist/` embed from `dashboard.rs`
2. Remove all `/api/*` routes from `build_dashboard_router()`
3. Remove the SPA static file handlers (`spa_index_handler`, `spa_asset_handler`, `spa_fallback_handler`, `serve_embedded_file`)
4. Keep only `GET /health` (already on `http.rs`) — remove `build_dashboard_router` call from `http.rs`
5. Remove `rust_embed` and `mime_guess` from `nv-daemon` `Cargo.toml` if no other users exist

The `DashboardState` struct can be removed or reduced. The `DashboardState` fields used only by dashboard API handlers (not by daemon core) are removed.

`dashboard/` (the old Vite SPA source) is deleted after the Rust embed is removed. A follow-on task (separate beads issue) tracks the actual deletion so it can be reversed independently.

### Req-7: Tailscale Access

The dashboard runs on the Tailscale network only — no public exposure. The Docker container's port 3000 is reachable at `http://<tailscale-ip>:3000` or via Tailscale DNS. No nginx reverse proxy is required for the initial extraction.

A follow-on spec (`rebuild-dashboard-wireframes`) will add auth gating if needed.

### Req-8: Environment Variables

| Variable | Where | Default | Purpose |
|---|---|---|---|
| `DAEMON_URL` | Docker env / `.env.local` | `http://127.0.0.1:3443` | Daemon base URL for proxy routes |
| `PORT` | Docker env | `3000` | Next.js server port |
| `NODE_ENV` | Docker env | `production` | Next.js mode |

No other secrets are required for the proxy-mode initial extraction.

## Scope

**IN:**
- Next.js 15 App Router scaffold at `apps/dashboard/`
- All 8 pages and 13 components ported from the Vite SPA
- 8 API route handlers (proxy mode — no direct DB access yet)
- Dockerfile (multi-stage, standalone output)
- Docker Compose entry for `dashboard` service
- Strip Rust embed code from `dashboard.rs` + `http.rs`
- Remove `rust_embed` / `mime_guess` from `Cargo.toml` if unused
- Delete `dashboard/` Vite SPA source directory

**OUT:**
- Direct SQLite access from Next.js (proxy mode is sufficient for Wave 2a)
- Auth / access control (follow-on spec)
- New pages or UI features (Wave 2b specs)
- Nginx / reverse proxy setup
- Tailscale ACL changes
- CI/CD pipeline changes

## Impact

| Area | Change |
|---|---|
| `apps/dashboard/` | New: complete Next.js 15 app |
| `crates/nv-daemon/src/dashboard.rs` | Strip to empty / delete (all API routes removed) |
| `crates/nv-daemon/src/http.rs` | Remove `build_dashboard_router` call, remove DashboardState wiring |
| `crates/nv-daemon/Cargo.toml` | Remove `rust-embed`, `mime_guess` if no other users |
| `dashboard/` | Delete (Vite SPA source, superseded) |
| `Cargo.toml` (workspace) | Remove `rust-embed` workspace dep if daemon is sole user |

## Risks

| Risk | Mitigation |
|---|---|
| API contract drift between proxy and daemon | Types in `apps/dashboard/types/api.ts` are ported verbatim from the Vite SPA — any mismatch is caught by TypeScript at the call sites |
| Daemon port changes break proxy | `DAEMON_URL` is a runtime env var — easy to override per deployment |
| RustEmbed removal breaks compile | Strip in a dedicated task after the Next.js app builds and serves correctly |
| `dashboard/` deletion is hard to reverse | Schedule deletion as a separate beads issue after one full week of production soak |
| Next.js standalone output misses assets | Use `output: 'standalone'` in `next.config.ts` and verify `public/` is copied in Dockerfile |
