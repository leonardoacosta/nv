# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] Add `GET /api/fleet-health` handler in `crates/nv-daemon/src/http.rs` -- proxy GET to `http://127.0.0.1:4100/health` (tool-router) and `http://127.0.0.1:4103/channels` (channels-svc) in parallel, merge into combined response, 5s/3s timeouts respectively, return graceful fallback on connection refused [owner:api-engineer]
- [ ] [2.2] [P-1] Add `GET /api/fleet-registry` handler in `crates/nv-daemon/src/http.rs` -- proxy GET to `http://127.0.0.1:4100/registry`, cache response in-memory for 60s using `tokio::sync::RwLock<Option<(Instant, Value)>>`, return empty object on failure [owner:api-engineer]
- [ ] [2.3] [P-2] Register both new routes in the Axum router alongside existing `/api/config` etc. [owner:api-engineer]

## UI Batch

- [ ] [3.1] [P-1] Add `FleetHealthResponse` and `FleetRegistryResponse` types to `dashboard/src/types/api.ts` matching the shapes from Req-2 and Req-3 [owner:ui-engineer]
- [ ] [3.2] [P-1] Create `dashboard/src/components/ChannelRow.tsx` -- status dot (green/red/yellow), channel name, direction badge, dense single-line layout [owner:ui-engineer]
- [ ] [3.3] [P-1] Create `dashboard/src/components/ServiceRow.tsx` -- expandable row with status dot, service name, port, latency, tool count; expanded state shows base URL, tool list, last check timestamp [owner:ui-engineer]
- [ ] [3.4] [P-1] Create `dashboard/src/pages/StatusPage.tsx` -- three sections (Channels, Fleet Services, Infrastructure), fetches `/api/fleet-health`, `/api/fleet-registry`, `/api/server-health` in parallel, auto-refresh every 30s with cleanup, "Last checked" indicator [owner:ui-engineer]
- [ ] [3.5] [P-1] Update `dashboard/src/App.tsx` -- replace `/integrations` route with `/status`, update import from IntegrationsPage to StatusPage [owner:ui-engineer]
- [ ] [3.6] [P-1] Update `dashboard/src/components/Sidebar.tsx` -- rename "Integrations" to "Status", change icon from `Plug` to `Activity`, update `to` path to `/status` [owner:ui-engineer]
- [ ] [3.7] [P-2] Delete `dashboard/src/pages/IntegrationsPage.tsx` [owner:ui-engineer]
- [ ] [3.8] [P-2] Delete `dashboard/src/components/IntegrationCard.tsx` [owner:ui-engineer]
- [ ] [3.9] [P-2] Delete `dashboard/src/components/ConfigureModal.tsx` [owner:ui-engineer]

## E2E Batch

(no E2E tasks -- dashboard has no E2E test suite)

## Verify

- [ ] [4.1] `cargo build` passes with new fleet-health and fleet-registry handlers [owner:api-engineer]
- [ ] [4.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [4.3] Existing Rust tests pass [owner:api-engineer]
- [ ] [4.4] `pnpm --filter dashboard build` passes (Vite + tsc) [owner:ui-engineer]
- [ ] [4.5] No references to IntegrationCard, ConfigureModal, or IntegrationsPage remain in codebase [owner:ui-engineer]
- [ ] [4.6] StatusPage renders three sections with loading skeletons when fleet is unreachable [owner:ui-engineer]
