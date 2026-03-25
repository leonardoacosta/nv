# Proposal: Add Cold-Start Logging

## Change ID
`add-cold-start-logging`

## Summary

Instrument every cold-start Claude session with structured timing metrics — context build
latency, time to first Claude response, total wall time, tool call count, and token usage —
persisted to SQLite and surfaced in the dashboard as a latency chart with P50/P95/P99
percentiles and a trend line.

## Context
- Phase: Wave 2b — depends on `extract-nextjs-dashboard`
- Related beads: nv-clp (cold-start-dashboard-logging)
- Extends: `crates/nv-daemon/src/worker.rs` (Worker::run timing), `crates/nv-daemon/src/claude.rs` (cold-start path)
- Stores into: `~/.nv/messages.db` (existing SQLite database, alongside obligations/schedule/reminders)
- Surfaces via: `dashboard/` (new page or widget on DashboardPage)

## Motivation

Cold-start latency is the dominant user-perceived delay on every Nova interaction (~8–14s per
session as logged at line 952 of `claude.rs`). The timing data is already computed in
`Worker::run` — `context_build_start`, `call_start`, `task_start` — and logged via `tracing`
only. There is no persistent record, no percentile view, no way to know whether changes are
improving or regressing latency over time.

This spec captures that data into SQLite and puts it in front of you on the dashboard.

## Design

### Where to Instrument

`Worker::run` in `worker.rs` already has all the timing primitives:

| Variable | Description | Line (approx) |
|----------|-------------|---------------|
| `task_start` | Overall worker start | 583 |
| `context_build_start` | Context assembly start | 612 |
| `StageComplete { duration_ms }` | Context build duration | 800-804 |
| `call_start` | Claude API call start | 819 |
| `response_time_ms` | Time from call_start to response | 891 |
| `tokens_in` / `tokens_out` | From `response.usage` | 892-893 |

Tool calls are already tracked (emitted as `WorkerEvent::ToolCalled`). A counter is easy to
accumulate during the tool use loop.

`send_messages_cold_start_with_image` in `claude.rs` also records `cold_start.elapsed()` at
line 952 but this is only the subprocess wall time, not the full session view.

### Data Model

New table `cold_start_events` in `messages.db` (shares the existing SQLite file and
`rusqlite`/`rusqlite_migration` stack — no new dependencies):

```sql
CREATE TABLE IF NOT EXISTS cold_start_events (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT    NOT NULL,        -- task UUID from worker
    started_at      TEXT    NOT NULL,        -- ISO-8601 UTC timestamp
    context_build_ms INTEGER NOT NULL,       -- time to build context (StageComplete duration_ms)
    first_response_ms INTEGER NOT NULL,      -- time from call_start to first Claude response
    total_ms        INTEGER NOT NULL,        -- task_start to response received
    tool_count      INTEGER NOT NULL DEFAULT 0,
    tokens_in       INTEGER NOT NULL DEFAULT 0,
    tokens_out      INTEGER NOT NULL DEFAULT 0,
    trigger_type    TEXT    NOT NULL DEFAULT ''   -- 'message' | 'cron' | 'nexus' | 'cli'
);
```

Retention: no auto-deletion. Events are small (~100 bytes each); 10k events = ~1 MB.

### Store API

New `ColdStartStore` in `crates/nv-daemon/src/cold_start_store.rs`, following the same
pattern as `ObligationStore` (sync `rusqlite::Connection`, migration via
`rusqlite_migration`):

```rust
pub struct ColdStartEvent {
    pub session_id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub context_build_ms: u64,
    pub first_response_ms: u64,
    pub total_ms: u64,
    pub tool_count: u32,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub trigger_type: String,
}

impl ColdStartStore {
    pub fn new(db_path: &Path) -> Result<Self>
    pub fn insert(&self, event: &ColdStartEvent) -> Result<()>
    pub fn get_recent(&self, limit: usize) -> Result<Vec<ColdStartEvent>>
    pub fn get_percentiles(&self, window_hours: u32) -> Result<Percentiles>
}

pub struct Percentiles {
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub sample_count: usize,
}
```

`get_percentiles` uses SQLite window functions or ORDER BY + LIMIT math on `total_ms`.

### Migration

The table is added to `MessageStore`'s migration sequence (same file, same pattern used for
`obligations`, `schedules`, `reminders`). `ColdStartStore::new` opens the same `messages.db`
path.

### Worker Integration

In `Worker::run`, after the tool use loop completes and `tokens_in`/`tokens_out` are known:

1. Accumulate `tool_count` as a `usize` counter incremented on each `ToolCalled` event
   (already emitted) or on each iteration of the tool dispatch loop.
2. Compute `total_ms = task_start.elapsed().as_millis() as u64`.
3. `context_build_ms` is already computed via `StageComplete` event.
4. `first_response_ms = call_start.elapsed().as_millis() as u64` (already `response_time_ms`
   as `i64`, just reuse it).
5. Build and insert `ColdStartEvent`. Wrap in `spawn_blocking` since `ColdStartStore` is sync.

`ColdStartStore` is added to `SharedDeps` as `Option<Arc<std::sync::Mutex<ColdStartStore>>>`,
consistent with `obligation_store`, `schedule_store`, `reminder_store`.

### Dashboard API Endpoint

New endpoint: `GET /api/cold-starts?limit=200`

The daemon's HTTP health server (`crates/nv-daemon/src/health.rs` or equivalent) gains this
route. Response is JSON:

```json
{
  "events": [
    {
      "session_id": "...",
      "started_at": "2026-03-25T10:00:00Z",
      "context_build_ms": 320,
      "first_response_ms": 8200,
      "total_ms": 9100,
      "tool_count": 3,
      "tokens_in": 4200,
      "tokens_out": 310,
      "trigger_type": "message"
    }
  ],
  "percentiles": {
    "p50_ms": 8500,
    "p95_ms": 13200,
    "p99_ms": 18700,
    "sample_count": 142
  }
}
```

### Dashboard UI

New page `dashboard/src/pages/ColdStartPage.tsx` (or a widget on `DashboardPage.tsx` —
implementer's choice, but a dedicated page is preferred for drill-down):

- **Latency chart**: line chart of `total_ms` over time (last 100 events). X-axis: timestamp.
  Y-axis: ms. One series for `total_ms`, one for `first_response_ms`.
- **Percentile cards**: P50, P95, P99 from the last 24h.
- **Trend line**: 20-event rolling average of `total_ms` overlaid on the chart.
- **Stats row**: avg `tool_count`, avg `tokens_in`, avg `tokens_out` for the visible window.

Use the same Recharts (or whatever charting library `UsagePage.tsx` uses) and Tailwind
patterns already present in `dashboard/`.

## Scope

**IN:** `cold_start_store.rs`, migration for `cold_start_events` table, Worker::run
instrumentation, `SharedDeps` field, `/api/cold-starts` endpoint, `ColdStartPage.tsx`

**OUT:** Per-tool breakdown, cold-start vs persistent-session comparison view (future),
alerts/notifications on latency regression, export to CSV

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/cold_start_store.rs` | New module: `ColdStartStore`, `ColdStartEvent`, `Percentiles` |
| `crates/nv-daemon/src/worker.rs` | Accumulate `tool_count`, insert `ColdStartEvent` after tool loop |
| `crates/nv-daemon/src/worker.rs` (`SharedDeps`) | Add `cold_start_store: Option<Arc<Mutex<ColdStartStore>>>` |
| `crates/nv-daemon/src/main.rs` | Init `ColdStartStore`, pass into `SharedDeps` |
| `crates/nv-daemon/src/messages.rs` (migration) | Add `cold_start_events` table to migration sequence |
| `crates/nv-daemon/src/health.rs` (or HTTP router) | Add `GET /api/cold-starts` route |
| `dashboard/src/pages/ColdStartPage.tsx` | New page: latency chart, P50/P95/P99 cards, trend line |
| `dashboard/src/App.tsx` | Register new page/route |

## Risks

| Risk | Mitigation |
|------|-----------|
| `insert` blocks the async worker thread | Wrap in `tokio::task::spawn_blocking` |
| `messages.db` schema conflict with other stores | Migration is additive — new table only |
| Dashboard charts require a charting lib not yet present | Check existing dashboard deps; add Recharts if missing (lightweight) |
| Cold-start timing double-counts retried cold-starts | Record `first_response_ms` from first attempt only; note retry in `trigger_type` field if needed |
