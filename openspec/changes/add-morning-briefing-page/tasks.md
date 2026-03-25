# Implementation Tasks

<!-- beads:epic:nv-837 -->

## Rust: BriefingStore

- [ ] [1.1] [P-1] Create `crates/nv-daemon/src/briefing_store.rs` — define `BriefingEntry` struct with fields: `id: String` (UUID v4), `generated_at: DateTime<Utc>`, `content: String`, `suggested_actions: Vec<SuggestedAction>`, `sources_status: HashMap<String, String>`; derive `Serialize`, `Deserialize`, `Debug`, `Clone` [owner:api-engineer]
- [ ] [1.2] [P-1] Implement `BriefingStore` struct with `path: PathBuf` pointing to `~/.nv/state/briefing-log.jsonl`; add `BriefingStore::new(nv_base: &Path) -> Self` constructor [owner:api-engineer]
- [ ] [1.3] [P-1] Implement `BriefingStore::append(&self, entry: &BriefingEntry) -> Result<()>` — serialize entry as a single JSONL line, append to file (create if absent), then trim the file to the last 30 entries using a read-rewrite cycle [owner:api-engineer]
- [ ] [1.4] [P-2] Implement `BriefingStore::list(&self, limit: usize) -> Result<Vec<BriefingEntry>>` — read all JSONL lines, deserialize, return up to `limit` entries in newest-first order [owner:api-engineer]
- [ ] [1.5] [P-2] Implement `BriefingStore::latest(&self) -> Result<Option<BriefingEntry>>` — convenience wrapper: `self.list(1).map(|v| v.into_iter().next())` [owner:api-engineer]
- [ ] [1.6] [P-3] Add `mod briefing_store;` declaration in `crates/nv-daemon/src/lib.rs` (or `main.rs` depending on module layout post-extract-nextjs-dashboard) [owner:api-engineer]

## Rust: BriefingStore Tests

- [ ] [2.1] [P-2] Unit test: `append_and_list_round_trip` — append 3 entries, call `list(10)`, assert all 3 returned newest-first, assert `content` and `generated_at` preserved exactly [owner:api-engineer]
- [ ] [2.2] [P-2] Unit test: `cap_at_30_entries` — append 35 entries, call `list(30)`, assert exactly 30 returned and the 5 oldest were dropped [owner:api-engineer]
- [ ] [2.3] [P-2] Unit test: `latest_returns_most_recent` — append 2 entries with distinct `generated_at`, call `latest()`, assert returns the newer one [owner:api-engineer]
- [ ] [2.4] [P-2] Unit test: `list_empty_store` — call `list(10)` on a store whose file does not exist, assert returns empty vec without error [owner:api-engineer]

## Rust: Digest Pipeline Integration

- [ ] [3.1] [P-1] Add `briefing_store: Option<Arc<BriefingStore>>` field to `DashboardState` in `dashboard.rs` [owner:api-engineer]
- [ ] [3.2] [P-1] In the worker or actions module that handles `CronEvent::MorningBriefing`: after `synthesize_digest()` (or `synthesize_digest_fallback()`) returns a `DigestResult`, construct a `BriefingEntry` (new UUID via `uuid::Uuid::new_v4()`, `generated_at: Utc::now()`, copy `result.content` and `result.suggested_actions`, copy `sources_status` from `DigestState`) and call `briefing_store.append(&entry)` [owner:api-engineer]
- [ ] [3.3] [P-2] Init `BriefingStore` in `main.rs` (or equivalent startup site) with `nv_base` path; wrap in `Arc`; pass to `DashboardState` [owner:api-engineer]
- [ ] [3.4] [P-3] Add `uuid` dependency to `Cargo.toml` for `nv-daemon` if not already present (`uuid = { version = "1", features = ["v4"] }`) [owner:api-engineer]

## Rust: API Endpoints

- [ ] [4.1] [P-1] Add `GET /api/briefing` route in `build_dashboard_router()` in `dashboard.rs` [owner:api-engineer]
- [ ] [4.2] [P-1] Implement `get_briefing` handler: call `briefing_store.latest()`, return 200 + `{"entry": ...}` on success; 404 + `{"error": "no briefing available"}` if empty; 503 + `{"error": "briefing store not available"}` if `briefing_store` is `None` [owner:api-engineer]
- [ ] [4.3] [P-1] Add `GET /api/briefing/history` route in `build_dashboard_router()` [owner:api-engineer]
- [ ] [4.4] [P-2] Implement `get_briefing_history` handler: accept `?limit=N` query param (default 10, clamp to max 30), call `briefing_store.list(limit)`, return 200 + `{"entries": [...]}` (empty array is valid); 503 if store absent [owner:api-engineer]
- [ ] [4.5] [P-3] Add `BriefingQuery` struct for query param deserialization: `struct BriefingQuery { limit: Option<usize> }` [owner:api-engineer]

## Rust: API Tests

- [ ] [5.1] [P-2] Unit test: `get_briefing_empty_store` — handler with empty BriefingStore returns 404 [owner:api-engineer]
- [ ] [5.2] [P-2] Unit test: `get_briefing_with_entry` — handler with one entry returns 200 + correct JSON shape [owner:api-engineer]
- [ ] [5.3] [P-2] Unit test: `get_briefing_history_respects_limit` — store with 15 entries, `?limit=5` returns 5 entries [owner:api-engineer]
- [ ] [5.4] [P-2] Unit test: `get_briefing_history_clamps_limit` — `?limit=100` is clamped to 30 [owner:api-engineer]

## Frontend: TypeScript Types

- [ ] [6.1] [P-1] Add to `dashboard/src/types/api.ts`: `BriefingAction` interface (`id`, `label`, `status: "pending" | "completed" | "dismissed"`); `BriefingEntry` interface (`id`, `generated_at`, `content`, `suggested_actions: BriefingAction[]`, `sources_status: Record<string, string>`); `BriefingGetResponse` (`entry: BriefingEntry`); `BriefingHistoryGetResponse` (`entries: BriefingEntry[]`) [owner:ui-engineer]

## Frontend: Section Parser Utility

- [ ] [7.1] [P-1] Create `dashboard/src/utils/briefing.ts` — export `BriefingSection` interface (`title: string`, `body: string`) and `parseBriefingSections(content: string): BriefingSection[]` [owner:ui-engineer]
- [ ] [7.2] [P-2] Implement `parseBriefingSections`: detect `-- Title --` lines (fallback format from `synthesize_digest_fallback`) and `### Title` lines (Claude markdown format); split content into sections at each detected header; trim each body; return array of `{ title, body }` [owner:ui-engineer]
- [ ] [7.3] [P-2] Fallback: if no section headers detected, return `[{ title: "Summary", body: content.trim() }]` so the page always has something to render [owner:ui-engineer]
- [ ] [7.4] [P-3] Unit tests for `parseBriefingSections`: both delimiter formats, mixed format (first section has no header), empty string, content with only whitespace [owner:ui-engineer]

## Frontend: BriefingPage Component

- [ ] [8.1] [P-1] Create `dashboard/src/pages/BriefingPage.tsx` — page shell with `useState` for `entry: BriefingEntry | null`, `history: BriefingEntry[]`, `loading: boolean`, `error: string | null`, `selectedId: string | null` [owner:ui-engineer]
- [ ] [8.2] [P-1] On mount, fetch `GET /api/briefing` and `GET /api/briefing/history?limit=10` in parallel via `Promise.allSettled`; populate state accordingly [owner:ui-engineer]
- [ ] [8.3] [P-1] Render loading skeleton while `loading === true`: 3-4 pulse placeholder cards matching section card height [owner:ui-engineer]
- [ ] [8.4] [P-1] Render empty state when `entry === null` and not loading: center-aligned `Sun` icon, "No briefing yet today", subtitle "Nova generates a briefing each morning at 7am" [owner:ui-engineer]
- [ ] [8.5] [P-1] Render content panel when entry is available: call `parseBriefingSections(entry.content)` and map each section to a `BriefingSectionCard` sub-component with title and body text [owner:ui-engineer]
- [ ] [8.6] [P-2] Render `suggested_actions` as a horizontal strip of chips below the section cards; chip label shows action label; chip background reflects status (`pending` = purple tint, `completed` = emerald tint, `dismissed` = muted); chips are read-only (no interactivity in this spec) [owner:ui-engineer]
- [ ] [8.7] [P-2] Render history rail as a vertical list on the right side (or below on narrow viewports); each entry shows date + time formatted as "Mon Mar 25, 7:00am"; clicking an entry sets `selectedId` and displays that entry's content in the content panel [owner:ui-engineer]
- [ ] [8.8] [P-2] Header row: title "Morning Briefing", subtitle showing `generated_at` formatted as "Today, 7:00am" (or full date for historical entries), Refresh button that re-fetches latest [owner:ui-engineer]
- [ ] [8.9] [P-3] Display `sources_status` as a small badge row under the header: each source name + status indicator dot (green = "ok", red = "unavailable", grey = unknown) [owner:ui-engineer]

## Frontend: Auto-refresh

- [ ] [9.1] [P-2] Add `useEffect` with `setInterval` polling `GET /api/briefing` every 60 seconds; on each poll compare `entry.generated_at` with polled result; if different, update `entry` state [owner:ui-engineer]
- [ ] [9.2] [P-3] Show a brief "Briefing updated" inline notification (non-blocking banner) when auto-refresh detects a new entry; auto-dismisses after 4 seconds [owner:ui-engineer]
- [ ] [9.3] [P-3] Clear the interval on component unmount to prevent memory leaks [owner:ui-engineer]

## Frontend: Navigation

- [ ] [10.1] [P-1] Add `{ to: "/briefing", label: "Briefing", icon: Sun }` to `NAV_ITEMS` array in `dashboard/src/components/Sidebar.tsx` — position between the Dashboard entry and the Obligations entry; import `Sun` from `lucide-react` [owner:ui-engineer]
- [ ] [10.2] [P-1] Add `import BriefingPage from "@/pages/BriefingPage"` and `<Route path="/briefing" element={<BriefingPage />} />` to `dashboard/src/App.tsx` — positioned after the root Dashboard route [owner:ui-engineer]

## Verify

- [ ] [11.1] `cargo build` passes for all workspace members [owner:api-engineer]
- [ ] [11.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [11.3] `cargo test -p nv-daemon` — all BriefingStore tests pass, all API endpoint tests pass [owner:api-engineer]
- [ ] [11.4] `pnpm build` (or `vite build`) passes in the `dashboard/` directory with no TypeScript errors [owner:ui-engineer]
- [ ] [11.5] [user] Manual smoke test: trigger a morning briefing digest manually (or wait for 07:00), then navigate to `/briefing` in the dashboard and confirm the briefing content renders with sections, action chips, and sources status [owner:ui-engineer]
- [ ] [11.6] [user] Manual smoke test: confirm history rail shows at least the current entry; click it to reload and verify the content panel updates [owner:ui-engineer]
- [ ] [11.7] [user] Manual smoke test: wait for the 60-second auto-refresh poll to fire (or reduce interval temporarily) and confirm the page updates in-place when a new briefing is available [owner:ui-engineer]
