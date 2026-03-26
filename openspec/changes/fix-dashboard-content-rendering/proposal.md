# Proposal: Fix Dashboard Content Rendering

## Change ID
`fix-dashboard-content-rendering`

## Summary
Five dashboard pages render structural layout but show blank/empty content because the Rust daemon is missing several API routes and the component-to-API field mappings are misaligned. This spec repairs the mismatches across `/settings`, `/approvals`, `/projects`, `/integrations`, and `/obligations`.

## Context
- Extends: `crates/nv-daemon/src/http.rs` (missing routes), `apps/dashboard/app/*/page.tsx` (field mapping fixes), `apps/dashboard/components/ObligationItem.tsx` (type mismatch), `apps/dashboard/types/api.ts` (missing types)
- Related: `fix-dashboard-api-proxy` must be complete first (Next.js proxy routes must be reachable)

## Motivation
The v7 Rust refactoring replaced `dashboard.rs` with `http.rs`, re-implementing endpoints but omitting several routes that the dashboard components depend on (`/api/obligations`, `/api/projects`, `/api/config`). At the same time, component interfaces were designed against an assumed API shape that diverged from the actual Rust types. The result is pages that mount correctly but display nothing because they either receive a 502 error (missing route → "Daemon unreachable") or silently fail to map the response fields.

## Requirements

### Req-1: Add missing daemon routes
The Rust router in `http.rs` must expose three new GET endpoints:

- `GET /api/obligations` — list obligations, supporting optional `?status=` and `?owner=` query params, returning `{ obligations: Obligation[] }`
- `GET /api/projects` — return the configured project registry, returning `{ projects: ApiProject[] }` (already has the `ApiProject` type at `crates/nv-core/src/types.rs`)
- `GET /api/config` — return daemon configuration as JSON (`Record<string, unknown>`)
- `PATCH /api/obligations/:id` — update obligation status (needed by `/approvals` dismiss handler)

#### Scenario: obligations list
Given the daemon has stored obligations, when the dashboard calls `GET /api/obligations?owner=leo&status=open`, then the response is `{ obligations: [...] }` with all matching records.

#### Scenario: missing daemon config file
Given no config file exists, when `GET /api/config` is called, then the response is `{}` (empty object, HTTP 200), not a 502.

### Req-2: Fix /obligations page — field name mismatch
The `ObligationItem` component interface (`title`, `description`, `due_at`, `tags`, `status: "open"|"in_progress"|"completed"|"dismissed"`) does not match the Rust `Obligation` struct (`detected_action`, `source_message`, `deadline`, `source_channel`, `status: ObligationStatus`).

Fix options (choose one per task owner decision):
- **Option A (preferred):** Update `ObligationItem.tsx` type and rendering to use Rust field names (`detected_action` as display title, `deadline` instead of `due_at`, drop `tags`).
- **Option B:** Add a normalizer in the obligations page that maps daemon fields to component fields before setting state.

The daemon status `Done` serializes as `"done"` — component filters on `"completed"`. The status map must be corrected: `"done"` → treat as completed for history tab.

#### Scenario: obligation renders with detected action
Given the daemon returns `{ detected_action: "Send weekly report", source_channel: "telegram", status: "open", owner: "nova", priority: 2, created_at: "..." }`, when ObligationsPage renders it, then the row displays "Send weekly report" as the obligation title.

### Req-3: Fix /approvals page — response shape mismatch
`ApprovalsPage` fetches `/api/obligations?owner=leo&status=open` and casts the result as `Approval[]`. The Rust response will be `{ obligations: [...] }` (wrapped array). The page must unwrap `.obligations` and then map `Obligation` fields to the `Approval` interface used by `DetailPanel` and `QueueItem`:

- `detected_action` → `title`
- `source_message` → `description`
- `status` (must translate: "open" → "pending", "done" → "approved")
- `urgency` must be derived from `priority` (0→"critical", 1→"high", 2→"medium", 3/4→"low")
- `proposed_changes` and `context` are optional and can be empty

#### Scenario: pending obligation appears as approval
Given the daemon returns an obligation with `owner: "leo"`, `status: "open"`, `priority: 1`, when ApprovalsPage loads, then a QueueItem renders with the obligation's `detected_action` as the title and urgency badge "High".

### Req-4: Fix /projects page — response shape mismatch
`ProjectsPage` fetches `/api/projects` and casts `(await res.json()) as Project[]`. The daemon returns `{ projects: [{ code, path }] }`. Two issues:
1. Must unwrap `.projects` from the response.
2. `ApiProject` (`{ code, path }`) does not match `Project` (`{ id, name, path, status, errors }`). Must map: `code → id`, `code → name`, path is shared, `status: "unknown"` (daemon doesn't provide health status), `errors: []`.

#### Scenario: projects list renders from daemon response
Given daemon returns `{ projects: [{ code: "nv", path: "/home/nyaptor/nv" }] }`, when ProjectsPage loads, then one ProjectAccordion renders with name "nv" and status badge "Unknown".

### Req-5: Fix /settings page — missing daemon route + rendering fallback
`SettingsPage` fetches `/api/config` (missing daemon route → 502 today). Once Req-1 adds the route, the settings page should handle the case where the config response is an empty object `{}` gracefully: show all four section cards with an empty state message ("No fields configured") rather than blank invisible cards.

#### Scenario: empty config shows placeholder
Given the config endpoint returns `{}`, when SettingsPage renders, then all four section cards (Daemon, Channels, Integrations, Memory) are visible with a "No fields configured" placeholder inside each.

### Req-6: Fix /integrations page — missing daemon route + buildFromConfig fallback
`IntegrationsPage` fetches `/api/config` and tries `raw.integrations as Integration[]` or falls back to `buildFromConfig(raw)`. The `buildFromConfig` function is referenced but not defined in the file — this is a runtime crash. The function must be implemented to derive `Integration` cards from a flat config object based on known service key patterns (e.g. a key `telegram.token` → Telegram integration with status "connected" if non-empty).

#### Scenario: integrations derived from config
Given the config contains `{ "telegram": { "token": "..." }, "anthropic": { "api_key": "..." } }`, when IntegrationsPage loads, then Telegram and Anthropic integration cards appear in their respective category sections.

#### Scenario: no config fields shows empty state
Given the config returns `{}`, when IntegrationsPage loads, then section headers render with "No integrations configured" rather than blank content.

## Scope
- **IN**: Adding the three missing Rust routes, fixing field name mismatches in obligations/approvals/projects pages, implementing `buildFromConfig` in integrations, fixing empty config handling in settings
- **OUT**: Redesigning component interfaces, adding new daemon capabilities, changing the `ObligationItem` priority system, changing how the config is stored or structured, fixing the `/messages` page (it is already correct at the code level — content issue is a data/daemon concern)

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/http.rs` | Add `GET /api/obligations`, `GET /api/projects`, `GET /api/config`, `PATCH /api/obligations/:id` |
| `apps/dashboard/app/obligations/page.tsx` | Unwrap response, map `Obligation` fields to component props |
| `apps/dashboard/app/approvals/page.tsx` | Unwrap response, map `Obligation` → `Approval` (title, urgency, status) |
| `apps/dashboard/app/projects/page.tsx` | Unwrap `{ projects }`, map `ApiProject` → `Project` |
| `apps/dashboard/app/integrations/page.tsx` | Implement `buildFromConfig`, add empty state |
| `apps/dashboard/app/settings/page.tsx` | Add empty-section placeholder for empty config |
| `apps/dashboard/components/ObligationItem.tsx` | Update interface to match Rust field names |
| `apps/dashboard/types/api.ts` | Add `ObligationsGetResponse`, `ProjectsGetResponse` (already partial) |

## Risks
| Risk | Mitigation |
|------|-----------|
| `GET /api/config` exposes secrets (tokens, API keys) | Mark secret fields with `***` masking in the response; settings page already implements secret masking via `isSecret()` |
| Rust `ObligationStore::list` may not exist yet | Implement `list()` method on `ObligationStore` or use inline SQL in the handler |
| `GET /api/projects` state source unclear (config file vs hardcoded) | Use `state.config` if available or return `{}` projects gracefully |
