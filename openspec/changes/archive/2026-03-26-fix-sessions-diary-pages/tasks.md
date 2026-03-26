# Tasks: fix-sessions-diary-pages

## Batch 1 — Daemon: Add GET /api/sessions

### T1 — Check HttpState for TeamAgentDispatcher access
**Agent:** `analyst`
**File:** `crates/nv-daemon/src/http.rs`
- [x] Read `HttpState` struct definition
- [x] Confirmed `TeamAgentDispatcher` was absent; added `dispatcher: Option<TeamAgentDispatcher>` field
- [x] Also identified `project_registry` and `config_path` were in `HttpState` but not wired through `run_http_server` — fixed both

### T2 — Implement get_sessions_handler in http.rs
**Agent:** `engineer`
**Files:** `crates/nv-daemon/src/http.rs`, `crates/nv-daemon/src/team_agent/types.rs`
**Depends on:** T1
- [x] Added `SessionsResponse` and `SessionItem` structs with `Serialize`
- [x] Implemented `get_sessions_handler` calling `dispatcher.list_agents().await`
- [x] Added `.route("/api/sessions", get(get_sessions_handler))` in `build_router`
- [x] Added `dispatcher` param to `run_http_server` and updated `main.rs` to pass it
- [x] Also wired `project_registry` and `config_path` through `run_http_server` (pre-existing gap)
- [x] Gate: `cargo check -p nv-daemon` passes (0 errors)

## Batch 2 — Next.js: Add /api/diary proxy route

### T3 — Create apps/dashboard/app/api/diary/route.ts
**Agent:** `engineer`
**File:** `apps/dashboard/app/api/diary/route.ts`
- [x] Diary proxy route already existed from previous spec — confirmed correct implementation
- [x] Updated `apps/dashboard/app/api/sessions/route.ts` from 501 stub to real daemon proxy
- [x] Gate: `pnpm typecheck` passes (0 errors)

## Batch 3 — Dashboard: Remove /nexus duplicate

### T4 — Remove /nexus page and update navigation
**Agent:** `engineer`
**Files:**
  - `apps/dashboard/app/nexus/page.tsx` (delete)
  - `apps/dashboard/components/Sidebar.tsx` (remove nexus nav link if present)
  - `apps/dashboard/next.config.ts` (add redirect to `/sessions`)
- [x] Deleted `apps/dashboard/app/nexus/page.tsx` and directory
- [x] Removed `{ to: "/nexus", label: "Nexus", icon: Zap }` from Sidebar NAV_ITEMS
- [x] Removed unused `Zap` import from Sidebar
- [x] Added `{ source: '/nexus', destination: '/sessions', permanent: true }` redirect in `next.config.ts`
- [x] Gate: `pnpm typecheck` passes

## Batch 4 — Validation

### T5 — Run typecheck and verify
**Agent:** `engineer`
- [x] `cargo check -p nv-daemon`: 0 errors, 1 pre-existing unused import warning
- [x] `cargo clippy -p nv-daemon`: 0 errors, 2 pre-existing warnings (both unrelated to this spec)
- [x] `pnpm typecheck` (apps/dashboard): 0 errors
- [deferred] Manual browser smoke: navigate to /sessions and /diary, verify no error banners

## Notes

- Tasks T1–T3 are independent and can run in parallel (T2 depends on T1 findings, T3 is fully independent)
- T4 is independent of T1–T3 (frontend-only cleanup)
- T5 gates the whole batch
- Rust `progress` field: serialize as `Option<serde_json::Value>` = `None` → JSON `null`. Frontend
  `NexusSessionRaw.progress` is already typed as optional.
- Pre-existing gap fixed: `project_registry` and `config_path` were in `HttpState` but not passed
  through `run_http_server` — this was causing a latent compile error that was masked by the missing
  `dispatcher` field. Both are now wired correctly.
