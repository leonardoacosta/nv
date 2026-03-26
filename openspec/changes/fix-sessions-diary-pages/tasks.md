# Tasks: fix-sessions-diary-pages

## Batch 1 — Daemon: Add GET /api/sessions

### T1 — Check HttpState for TeamAgentDispatcher access
**Agent:** `analyst`
**File:** `crates/nv-daemon/src/http.rs`
- Read `HttpState` struct definition
- Confirm whether `TeamAgentDispatcher` is already a field
- If absent, identify the correct field to add and how it is constructed in `main.rs`
- Output: findings note for T2

### T2 — Implement get_sessions_handler in http.rs
**Agent:** `engineer`
**Files:** `crates/nv-daemon/src/http.rs`, `crates/nv-daemon/src/team_agent/types.rs`
**Depends on:** T1
- Add a `Serialize`-capable response struct `SessionsResponse` (or reuse `serde_json::json!`) that
  maps `SessionSummary` fields to the JSON shape:
  `{ id, project, status, agent_name, started_at, duration_display, branch, spec, progress: null }`
- Implement `async fn get_sessions_handler(State(state): State<Arc<HttpState>>) -> impl IntoResponse`
- Add `.route("/api/sessions", get(get_sessions_handler))` in `build_router`
- Gate: `cargo check -p nv-daemon` must pass

## Batch 2 — Next.js: Add /api/diary proxy route

### T3 — Create apps/dashboard/app/api/diary/route.ts
**Agent:** `engineer`
**File:** `apps/dashboard/app/api/diary/route.ts` (new file)
- Implement `GET` handler forwarding `?date=` and `?limit=` query params to `DAEMON_URL/api/diary`
- Mirror the pattern used by `apps/dashboard/app/api/sessions/route.ts` for error handling
- Gate: `pnpm typecheck` in `apps/dashboard` must pass

## Batch 3 — Dashboard: Remove /nexus duplicate

### T4 — Remove /nexus page and update navigation
**Agent:** `engineer`
**Files:**
  - `apps/dashboard/app/nexus/page.tsx` (delete)
  - `apps/dashboard/components/Sidebar.tsx` (remove nexus nav link if present)
  - `apps/dashboard/next.config.ts` or `apps/dashboard/app/nexus/page.tsx` replacement
    (add redirect to `/sessions` if needed)
- Delete the nexus page file
- Check Sidebar.tsx for any `/nexus` href and update to `/sessions`
- Add Next.js redirect: `{ source: '/nexus', destination: '/sessions', permanent: true }`
- Gate: `pnpm build` must pass (no missing page errors)

## Batch 4 — Validation

### T5 — Run typecheck and verify
**Agent:** `engineer`
- Run `cargo check -p nv-daemon` from repo root
- Run `pnpm typecheck` from `apps/dashboard`
- Confirm no errors
- [deferred] Manual browser smoke: navigate to /sessions and /diary, verify no error banners

## Notes

- Tasks T1–T3 are independent and can run in parallel (T2 depends on T1 findings, T3 is fully independent)
- T4 is independent of T1–T3 (frontend-only cleanup)
- T5 gates the whole batch
- Rust `progress` field: serialize as `Option<serde_json::Value>` = `None` → JSON `null`. Frontend
  `NexusSessionRaw.progress` is already typed as optional.
