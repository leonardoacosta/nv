# Implementation Tasks

<!-- beads:epic:nv-32sl -->

## Phase 1: Rust — DiaryEntry Field Extensions

- [x] [1.1] [P-1] `diary.rs`: Add `channel_source: String` and `response_latency_ms: u64` fields to `DiaryEntry` struct [owner:api-engineer]
- [x] [1.2] [P-1] `diary.rs`: Update `format_entry()` to include `**Channel:** {channel_source}` line and `**Latency:** {response_latency_ms}ms` line in the formatted output (see proposal Req-1 for exact order) [owner:api-engineer]
- [x] [1.3] [P-1] `diary.rs`: Add `base_path() -> &Path` accessor method to `DiaryWriter` (needed by orchestrator cmd_diary and HTTP handler) [owner:api-engineer]
- [x] [1.4] [P-2] `diary.rs`: Add rollover test — construct two `DiaryEntry` values with timestamps spanning midnight (day N at 23:59, day N+1 at 00:00), call `write_entry` for both, assert two separate daily files exist [owner:api-engineer]
- [x] [1.5] [P-2] `diary.rs`: Update existing tests that construct `DiaryEntry` to include the two new fields (compiler will flag them) [owner:api-engineer]

## Phase 2: Rust — Worker Sites

- [x] [2.1] [P-1] `worker.rs` cold-start path (line ~1515): populate `channel_source` from `task.triggers` first trigger's channel field; populate `response_latency_ms` from `response_time_ms.max(0) as u64` [owner:api-engineer]
- [x] [2.2] [P-1] `worker.rs` dashboard-path entry (line ~1114): populate `channel_source` from `task.triggers` first trigger's channel field; populate `response_latency_ms` from `task_start.elapsed().as_millis() as u64` [owner:api-engineer]

## Phase 3: Rust — /diary Bot Command

- [x] [3.1] [P-2] `orchestrator.rs`: Add `"diary"` arm to the command dispatch match in `handle_bot_commands` (alongside "status", "digest", "health", etc.) [owner:api-engineer]
- [x] [3.2] [P-2] `orchestrator.rs`: Implement `cmd_diary(&self, args: &[String]) -> String` — parse optional N from args (default 5, clamp to 20); read today's and optionally yesterday's diary file via `deps.diary.lock().unwrap().base_path()`; split on `## ` headings; return last N entries as plain text in the format shown in proposal Req-4 [owner:api-engineer]
- [x] [3.3] [P-3] `orchestrator.rs` tests: add `classify_bot_command_triggers` assertion for `/diary` → `TriggerClass::BotCommand` [owner:api-engineer]

## Phase 4: Rust — HTTP Endpoint

- [x] [4.1] [P-2] `http.rs`: Add `diary: Arc<std::sync::Mutex<DiaryWriter>>` field to `HttpState` struct [owner:api-engineer]
- [x] [4.2] [P-2] `http.rs`: Add `GET /api/diary` route in `build_router` [owner:api-engineer]
- [x] [4.3] [P-2] `http.rs`: Implement `get_diary_handler` — accept `?date=YYYY-MM-DD` (default today) and `?limit=N` (default 50); read the daily markdown file; parse each `## HH:MM` block into a `DiaryEntryItem` JSON struct; return `DiaryGetResponse` (see proposal Req-5 for shape) [owner:api-engineer]
- [x] [4.4] [P-2] `main.rs`: Thread `Arc::clone(&diary)` into `HttpState` at construction site [owner:api-engineer]

## Phase 5: Dashboard

- [x] [5.1] [P-2] `dashboard/src/types/api.ts`: Add `DiaryEntryItem` and `DiaryGetResponse` interfaces matching the JSON shape from proposal Req-5 [owner:ui-engineer]
- [x] [5.2] [P-2] `dashboard/src/components/DiaryEntry.tsx`: New component rendering a single diary entry card — heading line (time, trigger type, channel, slug), tools pills row, result text, latency chip, token cost badge; use existing cosmic theme classes [owner:ui-engineer]
- [x] [5.3] [P-2] `dashboard/src/pages/DiaryPage.tsx`: New page — date state (default today), fetch `GET /api/diary?date=YYYY-MM-DD`, show loading/error/empty states matching existing page patterns; header with prev/next day arrows + formatted date display; summary bar (total entries, total tokens, avg latency); reverse-chronological list of `DiaryEntry` cards [owner:ui-engineer]
- [x] [5.4] [P-2] `dashboard/src/components/Sidebar.tsx`: Add `{ to: "/diary", label: "Diary", icon: BookOpen }` to `NAV_ITEMS`; add `BookOpen` to the lucide-react import [owner:ui-engineer]
- [x] [5.5] [P-2] `dashboard/src/App.tsx`: Add `import DiaryPage` and `<Route path="/diary" element={<DiaryPage />} />` [owner:ui-engineer]

## Verify

- [x] [6.1] `cargo build` passes with zero warnings [owner:api-engineer]
- [x] [6.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [6.3] `cargo test` — all diary module tests pass including new rollover test and updated struct construction tests [owner:api-engineer]
- [x] [6.4] Dashboard: `pnpm build` passes with zero TypeScript errors [owner:ui-engineer]
- [ ] [6.5] Manual smoke: start daemon, send a Telegram message, send `/diary`, confirm entry appears with channel, latency, and token fields populated [owner:api-engineer]
