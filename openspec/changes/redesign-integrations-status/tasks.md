# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Add `GET /api/fleet-status` Next.js API route -- returns static fleet service registry with "unknown" status (fleet unreachable from Docker), includes channel list from config [owner:ui-engineer]
- [x] [2.2] [P-1] (adapted) Fleet registry data embedded in `/api/fleet-status` response -- no separate endpoint needed since tool-router is on host network [owner:ui-engineer]
- [x] [2.3] [P-2] (adapted) Route registered as Next.js App Router file-based API route at `app/api/fleet-status/route.ts` [owner:ui-engineer]

## UI Batch

- [x] [3.1] [P-1] Add `FleetHealthResponse`, `FleetServiceStatus`, and `ChannelStatus` types to `types/api.ts` [owner:ui-engineer]
- [x] [3.2] [P-1] Create `components/ChannelRow.tsx` -- status dot, channel name, direction badge, dense single-line layout [owner:ui-engineer]
- [x] [3.3] [P-1] Create `components/ServiceRow.tsx` -- expandable row with status dot, service name, port, latency, tool count; expanded state shows base URL, tool list, last check timestamp [owner:ui-engineer]
- [x] [3.4] [P-1] Replace `app/integrations/page.tsx` with StatusPage -- three sections (Channels, Fleet Services, Infrastructure), fetches `/api/fleet-status` + `/api/server-health` in parallel, auto-refresh every 30s with cleanup, "Last checked" indicator [owner:ui-engineer]
- [x] [3.5] [P-1] (adapted) No App.tsx in Next.js App Router -- route is file-based at `app/integrations/page.tsx`, kept existing path [owner:ui-engineer]
- [x] [3.6] [P-1] Update `components/Sidebar.tsx` -- renamed "Integrations" to "Status", changed icon from `Plug` to `Activity`, kept `/integrations` path [owner:ui-engineer]
- [x] [3.7] [P-2] Old IntegrationsPage code replaced in-place (same file path in App Router) [owner:ui-engineer]
- [x] [3.8] [P-2] Delete `components/IntegrationCard.tsx` [owner:ui-engineer]
- [x] [3.9] [P-2] Delete `components/ConfigureModal.tsx` [owner:ui-engineer]

## E2E Batch

(no E2E tasks -- dashboard has no E2E test suite)

## Verify

- [x] [4.1] (N/A) No Rust changes -- fleet status served from Next.js API route [owner:ui-engineer]
- [x] [4.2] (N/A) No Rust changes [owner:ui-engineer]
- [x] [4.3] (N/A) No Rust changes [owner:ui-engineer]
- [x] [4.4] `pnpm --filter nova-dashboard exec tsc --noEmit` passes [owner:ui-engineer]
- [x] [4.5] No references to IntegrationCard, ConfigureModal, or IntegrationsPage remain in codebase [owner:ui-engineer]
- [x] [4.6] StatusPage renders three sections with loading skeletons when fleet is unreachable [owner:ui-engineer]
