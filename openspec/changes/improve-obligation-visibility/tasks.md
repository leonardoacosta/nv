# Implementation Tasks

<!-- beads:epic:nv-15cq -->

## API Batch — Activity Infrastructure

- [x] [1.1] [P-1] Add `ObligationActivityEvent` struct to `crates/nv-daemon/src/http.rs` (or new `obligation_activity.rs`) — fields: `id: String`, `event_type: String`, `obligation_id: String`, `description: String`, `timestamp: DateTime<Utc>`, `metadata: Option<serde_json::Value>` [owner:api-engineer]
- [x] [1.2] [P-1] Add `ActivityRingBuffer` — `Arc<Mutex<VecDeque<ObligationActivityEvent>>>` with capacity 200; `push(event)` evicts oldest when full; `recent(limit) -> Vec` returns newest N [owner:api-engineer]
- [x] [1.3] [P-1] Wire `ActivityRingBuffer` into `HttpState` and `SharedDeps`; pass to obligation_executor, obligation_detector, and orchestrator callback handlers [owner:api-engineer]
- [x] [1.4] [P-2] Add `GET /api/obligations/activity?limit=50` endpoint to `build_router()` — returns `{ events: [...] }` from the ring buffer [owner:api-engineer]
- [x] [1.5] [P-2] Emit `obligation.detected` event in `obligation_detector.rs` when a new obligation is stored [owner:api-engineer]
- [x] [1.6] [P-2] Emit `obligation.execution_started`, `obligation.tool_called`, `obligation.execution_completed` events in `obligation_executor.rs` at appropriate points [owner:api-engineer]
- [x] [1.7] [P-2] Emit `obligation.confirmed` and `obligation.reopened` events in `callbacks.rs` confirm_done/reopen handlers [owner:api-engineer]
- [x] [1.8] [P-2] Broadcast each activity event as a `DaemonEvent` variant on `event_tx` for WebSocket delivery [owner:api-engineer]

## API Batch — Obligation Enrichment

- [x] [2.1] [P-1] Add `ObligationStore::list_notes(obligation_id) -> Vec<ObligationNote>` method — reads from `obligation_notes` table, returns newest-first, max 10 per obligation [owner:api-engineer]
- [x] [2.2] [P-1] Add `ObligationStore::get_stats() -> ObligationStats` method — returns counts: open_nova, open_leo, in_progress, proposed_done, done_today [owner:api-engineer]
- [x] [2.3] [P-2] Extend `GET /api/obligations` response — each obligation now includes `notes: Vec<ObligationNote>`, `attempt_count: u32`, `last_attempt_at: Option<String>` [owner:api-engineer]
- [x] [2.4] [P-2] Add `GET /api/obligations/stats` endpoint returning `ObligationStats` [owner:api-engineer]

## API Batch — Telegram Commands

- [x] [3.1] [P-1] Add `/obligations` (alias `/ob`) command to `handle_bot_commands` in orchestrator — lists open obligations grouped by owner, max 10, formatted with priority + status icon + truncated action [owner:api-engineer]
- [x] [3.2] [P-1] Add `/ob done <id_prefix>` command — match obligation by UUID prefix (min 6 chars), transition to `done`, reply with confirmation [owner:api-engineer]
- [x] [3.3] [P-1] Add `/ob assign <id_prefix> nova|leo` command — update obligation owner, reply with confirmation [owner:api-engineer]
- [x] [3.4] [P-1] Add `/ob create <text>` command — create new obligation with owner=nova, priority=2, source_channel=telegram, status=open; reply with confirmation [owner:api-engineer]
- [x] [3.5] [P-2] Add `/ob status` command — summary line: "Nova: N open, N in_progress, N proposed_done | Leo: N open" [owner:api-engineer]
- [x] [3.6] [P-2] Register all /ob subcommands in `classify_bot_command_triggers` for proper routing [owner:api-engineer]

## UI Batch — Dashboard Obligations Page

- [ ] [4.1] [P-1] Add `apps/dashboard/app/api/obligations/activity/route.ts` — GET proxy to daemon `/api/obligations/activity` [owner:ui-engineer]
- [ ] [4.2] [P-1] Add `apps/dashboard/app/api/obligations/stats/route.ts` — GET proxy to daemon `/api/obligations/stats` [owner:ui-engineer]
- [ ] [4.3] [P-1] Add obligation types to `apps/dashboard/types/api.ts` — `ObligationNote`, `ObligationActivity`, `ObligationStats`, extend `DaemonObligation` with `notes`, `attempt_count`, `last_attempt_at` [owner:ui-engineer]
- [ ] [4.4] [P-1] Create `apps/dashboard/components/ActivityFeed.tsx` — real-time event feed component; subscribes to WebSocket obligation events via `useDaemonEvents`; falls back to polling `/api/obligations/activity` every 10s; renders last 50 events reverse-chronological with type icon, timestamp, description [owner:ui-engineer]
- [ ] [4.5] [P-2] Add stats bar to `apps/dashboard/app/obligations/page.tsx` — 5 StatCard components across the top: Open (Nova), In Progress, Proposed Done, Done Today, Open (Leo); fetch from `/api/obligations/stats` [owner:ui-engineer]
- [ ] [4.6] [P-2] Redesign obligation card in `apps/dashboard/app/obligations/page.tsx` — header (action + status badge + priority + owner), context section (channel + source message expandable), execution history timeline (from notes), action buttons per status [owner:ui-engineer]
- [ ] [4.7] [P-2] Add `<ActivityFeed>` panel to obligations page — positioned as right sidebar on desktop (1/3 width), below obligations list on mobile [owner:ui-engineer]
- [ ] [4.8] [P-3] Add [Start] button on open obligations — sends POST to `/api/obligations/{id}/execute` (new endpoint that triggers immediate execution regardless of idle state) [owner:ui-engineer]

## Verify

- [x] [5.1] `cargo build -p nv-daemon` passes [owner:api-engineer]
- [ ] [5.2] `cargo clippy -p nv-daemon -- -D warnings` passes [owner:api-engineer]
- [ ] [5.3] `cd apps/dashboard && npx next build` passes [owner:ui-engineer]
- [ ] [5.4] [user] Telegram: send `/obligations` — verify list renders with priorities and owners
- [ ] [5.5] [user] Telegram: send `/ob create Test obligation for Nova` — verify obligation created
- [ ] [5.6] [user] Telegram: send `/ob status` — verify summary counts
- [ ] [5.7] [user] Dashboard: verify stats bar shows correct counts
- [ ] [5.8] [user] Dashboard: verify activity feed updates in real-time when Nova executes an obligation
