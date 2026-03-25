# Implementation Tasks

<!-- beads:epic:nv-t4b -->

## Batch 1 — Scaffold (Next.js app skeleton)

- [x] [1.1] [P-1] Create `apps/dashboard/package.json` — name `nova-dashboard`, Next.js 15, React 19, `lucide-react`, `geist`, `react-router-dom` removed (App Router handles routing) [owner:ui-engineer]
- [x] [1.2] [P-1] Create `apps/dashboard/next.config.ts` — `output: 'standalone'`, rewrites from `/api/*` to daemon proxy via `DAEMON_URL` env var [owner:ui-engineer]
- [x] [1.3] [P-1] Create `apps/dashboard/tsconfig.json` — path alias `@/*` → `apps/dashboard/*`, strict mode, Next.js plugin [owner:ui-engineer]
- [x] [1.4] [P-1] Create `apps/dashboard/tailwind.config.ts` — port all cosmic color tokens, font families (Geist Sans Variable, Geist Mono Variable), backgroundImage, boxShadow, borderRadius verbatim from `dashboard/tailwind.config.ts` [owner:ui-engineer]
- [x] [1.5] [P-1] Create `apps/dashboard/app/globals.css` — Tailwind directives, CSS vars for cosmic theme, Geist font face declarations [owner:ui-engineer]
- [x] [1.6] [P-1] Create `apps/dashboard/app/layout.tsx` — root layout with `<html>`, Geist font loading via `next/font` (or `geist` npm package), Tailwind `bg-cosmic-gradient min-h-dvh flex` wrapper, `<Sidebar />` + `<main>` slot [owner:ui-engineer]
- [x] [1.7] [P-2] Create `apps/dashboard/lib/daemon.ts` — `DAEMON_URL` constant from `process.env.DAEMON_URL ?? 'http://127.0.0.1:3443'`, typed `daemonFetch(path, init?)` helper that returns `Response` [owner:ui-engineer]
- [x] [1.8] [P-2] Port `apps/dashboard/types/api.ts` — copy `dashboard/src/types/api.ts` verbatim, update import paths if any [owner:ui-engineer]

## Batch 2 — Components (parallel with Batch 3)

- [ ] [2.1] [P-1] Port `apps/dashboard/components/Sidebar.tsx` — replace `react-router-dom` `Link`/`NavLink` with `next/link` `Link`, preserve all route paths and cosmic styling [owner:ui-engineer]
- [ ] [2.2] [P-1] Port `apps/dashboard/components/SessionCard.tsx` — direct copy, no routing changes needed [owner:ui-engineer]
- [ ] [2.3] [P-1] Port `apps/dashboard/components/ObligationItem.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.4] [P-1] Port `apps/dashboard/components/NovaMark.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.5] [P-1] Port `apps/dashboard/components/NovaBadge.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.6] [P-1] Port `apps/dashboard/components/LeoBadge.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.7] [P-2] Port `apps/dashboard/components/ProjectAccordion.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.8] [P-2] Port `apps/dashboard/components/ActiveSession.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.9] [P-2] Port `apps/dashboard/components/IntegrationCard.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.10] [P-2] Port `apps/dashboard/components/ConfigureModal.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.11] [P-2] Port `apps/dashboard/components/MemoryPreview.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.12] [P-2] Port `apps/dashboard/components/UsageSparkline.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.13] [P-2] Port `apps/dashboard/components/MiniChart.tsx` — direct copy [owner:ui-engineer]
- [ ] [2.14] [P-2] Port `apps/dashboard/components/ServerHealth.tsx` — direct copy [owner:ui-engineer]

## Batch 3 — Pages (parallel with Batch 2)

- [ ] [3.1] [P-1] Port `apps/dashboard/app/page.tsx` — DashboardPage from `dashboard/src/pages/DashboardPage.tsx`; replace `fetch('/api/...')` with relative paths (unchanged — Next.js serves `/api/*` from the same origin) [owner:ui-engineer]
- [ ] [3.2] [P-1] Port `apps/dashboard/app/obligations/page.tsx` — from `dashboard/src/pages/ObligationsPage.tsx` [owner:ui-engineer]
- [ ] [3.3] [P-1] Port `apps/dashboard/app/projects/page.tsx` — from `dashboard/src/pages/ProjectsPage.tsx` [owner:ui-engineer]
- [ ] [3.4] [P-1] Port `apps/dashboard/app/nexus/page.tsx` — from `dashboard/src/pages/NexusPage.tsx` [owner:ui-engineer]
- [ ] [3.5] [P-1] Port `apps/dashboard/app/integrations/page.tsx` — from `dashboard/src/pages/IntegrationsPage.tsx` [owner:ui-engineer]
- [ ] [3.6] [P-1] Port `apps/dashboard/app/usage/page.tsx` — from `dashboard/src/pages/UsagePage.tsx` [owner:ui-engineer]
- [ ] [3.7] [P-2] Port `apps/dashboard/app/memory/page.tsx` — from `dashboard/src/pages/MemoryPage.tsx` [owner:ui-engineer]
- [ ] [3.8] [P-2] Port `apps/dashboard/app/settings/page.tsx` — from `dashboard/src/pages/SettingsPage.tsx` [owner:ui-engineer]

## Batch 4 — API Route Handlers

- [x] [4.1] [P-1] Create `apps/dashboard/app/api/obligations/route.ts` — `GET` proxies to `${DAEMON_URL}/api/obligations` forwarding query params (`status`, `owner`); return daemon response body + status code unchanged [owner:ui-engineer]
- [x] [4.2] [P-1] Create `apps/dashboard/app/api/obligations/[id]/route.ts` — `PATCH` proxies to `${DAEMON_URL}/api/obligations/:id` forwarding JSON body; return daemon response [owner:ui-engineer]
- [x] [4.3] [P-1] Create `apps/dashboard/app/api/projects/route.ts` — `GET` proxies to `${DAEMON_URL}/api/projects`; return daemon response [owner:ui-engineer]
- [x] [4.4] [P-1] Create `apps/dashboard/app/api/sessions/route.ts` — `GET` proxies to `${DAEMON_URL}/api/sessions`; return daemon response [owner:ui-engineer]
- [x] [4.5] [P-1] Create `apps/dashboard/app/api/solve/route.ts` — `POST` proxies to `${DAEMON_URL}/api/solve` forwarding JSON body (`project`, `error`, `context`); return daemon response [owner:ui-engineer]
- [x] [4.6] [P-2] Create `apps/dashboard/app/api/memory/route.ts` — `GET` proxies to `${DAEMON_URL}/api/memory` forwarding `?topic=` param; `PUT` proxies JSON body; return daemon responses [owner:ui-engineer]
- [x] [4.7] [P-2] Create `apps/dashboard/app/api/config/route.ts` — `GET` proxies to `${DAEMON_URL}/api/config`; `PUT` proxies JSON body; return daemon responses [owner:ui-engineer]
- [x] [4.8] [P-2] Create `apps/dashboard/app/api/server-health/route.ts` — `GET` proxies to `${DAEMON_URL}/api/server-health`; return daemon response [owner:ui-engineer]

## Batch 5 — Docker

- [x] [5.1] [P-1] Create `apps/dashboard/Dockerfile` — stage 1: `node:22-alpine`, install deps with `npm ci`, run `npm run build`; stage 2: `node:22-alpine`, copy `.next/standalone` + `.next/static` + `public/`; expose port 3000; `CMD ["node", "server.js"]` [owner:ui-engineer]
- [x] [5.2] [P-1] Create `apps/dashboard/.dockerignore` — exclude `node_modules`, `.next`, `.env*` [owner:ui-engineer]
- [x] [5.3] [P-2] Add `dashboard` service to project Docker Compose file (check for existing `docker-compose.yml` at repo root first; create if absent) — build `apps/dashboard`, port 3000:3000, `DAEMON_URL=http://nv-daemon:3443`, `depends_on: nv-daemon`, `restart: unless-stopped` [owner:ui-engineer]

## Batch 6 — Strip Rust Embed

- [x] [6.1] [P-1] Remove `DashboardAssets` RustEmbed struct and `#[folder = "../../dashboard/dist/"]` from `crates/nv-daemon/src/dashboard.rs` [owner:api-engineer]
- [x] [6.2] [P-1] Remove all `/api/*` route registrations from `build_dashboard_router()` in `crates/nv-daemon/src/dashboard.rs` — remove handler functions: `get_obligations`, `patch_obligation`, `get_projects`, `get_sessions`, `post_solve`, `get_memory`, `put_memory`, `get_config`, `put_config`, `get_server_health` [owner:api-engineer]
- [x] [6.3] [P-1] Remove SPA static file handlers from `crates/nv-daemon/src/dashboard.rs`: `spa_index_handler`, `spa_asset_handler`, `spa_fallback_handler`, `serve_embedded_file`, and the `/assets/{*path}`, `/`, fallback routes [owner:api-engineer]
- [x] [6.4] [P-1] Remove `build_dashboard_router` call and `DashboardState` wiring from `crates/nv-daemon/src/http.rs`; remove `use crate::dashboard::{DashboardState, build_dashboard_router}` import [owner:api-engineer]
- [x] [6.5] [P-2] Check all `use` sites of `rust-embed` and `mime_guess` in `crates/nv-daemon/`; if `dashboard.rs` was the sole user, remove `rust-embed` and `mime_guess` from `crates/nv-daemon/Cargo.toml` and from `[workspace.dependencies]` in root `Cargo.toml` [owner:api-engineer]
- [x] [6.6] [P-2] Reduce or delete `DashboardState` struct — remove fields used only by the removed API handlers (`obligation_store`, `nv_base`, `config_json`, `nexus_client`, `messages_db_path`) if they are not referenced elsewhere in the daemon; keep `health` field if still used by any surviving handler [owner:api-engineer]
- [x] [6.7] [P-3] File a beads issue (separate from this spec) to delete the `dashboard/` Vite SPA source directory after one week of production soak — do not delete in this spec [owner:api-engineer]

## Batch 7 — Verify

- [ ] [7.1] [P-1] `cd apps/dashboard && npm install && npm run build` passes with zero TypeScript errors and zero build errors [owner:ui-engineer]
- [ ] [7.2] [P-1] `npm run typecheck` in `apps/dashboard/` passes [owner:ui-engineer]
- [ ] [7.3] [P-1] `cargo build` passes after Rust embed removal [owner:api-engineer]
- [ ] [7.4] [P-1] `cargo clippy -- -D warnings` passes after Rust embed removal [owner:api-engineer]
- [ ] [7.5] [P-2] Docker build succeeds: `docker build -t nova-dashboard apps/dashboard` produces a runnable image [owner:ui-engineer]
- [ ] [7.6] [P-2] `cargo test` — existing HTTP handler tests in `crates/nv-daemon/src/http.rs` continue to pass (health, ask, digest, stats, teams webhook tests must not be broken by dashboard removal) [owner:api-engineer]
- [ ] [7.7] [user] Manual smoke test: start daemon (`cargo run -p nv-daemon`) + Next.js dev server (`npm run dev` in `apps/dashboard/`); visit `http://localhost:3000` and verify DashboardPage loads with real data from all 4 API calls (obligations, projects, sessions, server-health) [owner:ui-engineer]
- [ ] [7.8] [user] Manual smoke test: verify all 8 pages render without console errors — Obligations, Projects, Nexus, Integrations, Usage, Memory, Settings [owner:ui-engineer]
- [ ] [7.9] [user] Manual smoke test: deploy Docker container, reach dashboard at Tailscale IP:3000, confirm all pages load [owner:ui-engineer]
