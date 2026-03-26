# Implementation Tasks

<!-- beads:epic:nv-quom -->

## DB Batch

- [ ] [1.1] [P-1] Add `project_registry: HashMap<String, PathBuf>` field to `HttpState` in `crates/nv-daemon/src/http.rs` and populate it in `main.rs` from the daemon's existing project registry source [owner:db-engineer] <!-- beads:nv-j631 -->

## API Batch

- [ ] [2.1] [P-1] Implement `GET /api/obligations` handler in `http.rs` — accepts `?status=` and `?owner=` params, uses `ObligationStore::list_by_status` / `list_by_owner` / `list_all`, returns `{ obligations: [...] }` [owner:api-engineer] <!-- beads:nv-z3uc -->
- [ ] [2.2] [P-1] Implement `PATCH /api/obligations/:id` handler in `http.rs` — accepts `{ "status": "..." }` body, calls `ObligationStore::update_status`, broadcasts `DaemonEvent::ApprovalUpdated`, returns `{ id, status }` or 404 [owner:api-engineer] <!-- beads:nv-xe48 -->
- [ ] [2.3] [P-1] Implement `GET /api/projects` handler in `http.rs` — reads `state.project_registry`, returns `{ projects: [{ code, path }] }` or `{ projects: [] }` [owner:api-engineer] <!-- beads:nv-mu78 -->
- [ ] [2.4] [P-1] Implement `GET /api/config` and `PUT /api/config` handlers in `http.rs` — GET reads daemon config from disk masking secrets ("***"), PUT accepts `{ "fields": { ... } }`, both return gracefully when no config file exists [owner:api-engineer] <!-- beads:nv-jhlf -->
- [ ] [2.5] [P-1] Register all new routes in `build_router()`: `GET /api/obligations`, `PATCH /api/obligations/:id`, `GET /api/projects`, `GET /api/config`, `PUT /api/config` [owner:api-engineer]
- [ ] [2.6] [P-1] Add `ObligationsGetResponse` type to `apps/dashboard/types/api.ts` with fields matching the Rust `Obligation` struct (`detected_action`, `source_channel`, `source_message`, `deadline`, `project_code`, `owner`, `status`, `priority`, etc.) [owner:api-engineer] <!-- beads:nv-wxij -->

## UI Batch

- [ ] [3.1] [P-1] Fix `apps/dashboard/app/obligations/page.tsx` — unwrap `{ obligations }` from response and map daemon `Obligation` fields to component interface (`detected_action→title`, `deadline→due_at`, `"done"→"completed"` status) [owner:ui-engineer] <!-- beads:nv-2fgt -->
- [ ] [3.2] [P-1] Fix `apps/dashboard/app/approvals/page.tsx` — unwrap `{ obligations }` from response and map daemon `Obligation` to `Approval` interface (`detected_action→title`, `priority→urgency` 0→critical/1→high/2→medium/3-4→low, `"open"→"pending"`) [owner:ui-engineer] <!-- beads:nv-imro -->
- [ ] [3.3] [P-1] Fix `apps/dashboard/app/projects/page.tsx` — unwrap `{ projects }` from response and map `ApiProject` (`code`, `path`) to `Project` (`id=code`, `name=code`, `status="unknown"`, `errors=[]`) [owner:ui-engineer] <!-- beads:nv-4bu3 -->
- [ ] [3.4] [P-1] Fix `apps/dashboard/app/integrations/page.tsx` — implement `buildFromConfig(raw)` mapping known config keys (telegram/discord/slack/teams/github/linear/notion/openai/anthropic/stripe/resend/sentry/posthog) to `Integration` objects with derived status [owner:ui-engineer] <!-- beads:nv-e916 -->
- [ ] [3.5] [P-2] Fix `apps/dashboard/app/integrations/page.tsx` — add "No integrations configured." placeholder per category group when `items.length === 0` [owner:ui-engineer]
- [ ] [3.6] [P-2] Fix `apps/dashboard/app/settings/page.tsx` — add "No fields configured." placeholder inside section body when `fields.length === 0` for that section [owner:ui-engineer] <!-- beads:nv-pqhz -->

## E2E Batch

- [ ] [4.1] Add smoke tests in `http.rs` verifying `GET /api/obligations`, `GET /api/projects`, `GET /api/config` return HTTP 200 with correct wrapper shape (following existing test harness pattern in the file) [owner:e2e-engineer]
