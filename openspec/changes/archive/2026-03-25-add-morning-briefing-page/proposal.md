# Proposal: Add Morning Briefing Page

## Change ID
`add-morning-briefing-page`

## Summary

Add a `/briefing` page to the Next.js dashboard that renders the latest AI-generated morning
digest. Nova already generates daily briefings at 07:00 via `CronEvent::MorningBriefing` and
persists the result in `~/.nv/state/last-digest.json`. This spec exposes that data through a
new API endpoint and renders it in a structured dashboard page with a history log.

## Context

- Phase: Wave 2b (Dashboard & Architecture Migration)
- Depends on: `extract-nextjs-dashboard` — the dashboard must exist as a standalone Next.js
  app before this page can be added
- Related beads: nv-837 (morning-briefing-digest)
- Key sources:
  - `crates/nv-daemon/src/digest/state.rs` — `DigestState`, `SuggestedAction`, `DigestActionStatus`
  - `crates/nv-daemon/src/digest/synthesize.rs` — `DigestResult` with `content` + `suggested_actions`
  - `crates/nv-daemon/src/digest/format.rs` — Telegram formatting (reference for section parsing)
  - `crates/nv-daemon/src/scheduler.rs` — `CronEvent::MorningBriefing` fires daily at 07:00
  - `crates/nv-daemon/src/dashboard.rs` — existing API route patterns (axum + `DashboardState`)
  - `dashboard/src/types/api.ts` — canonical TS type location
  - `dashboard/src/App.tsx` — route registration
  - `dashboard/src/components/Sidebar.tsx` — nav registration

## Motivation

The morning briefing is Nova's highest-signal daily output: Jira triage, active sessions,
memory highlights, calendar events, and 3-5 suggested actions — all synthesized by Claude.
Currently this content only reaches Leo via Telegram at 07:00. If Leo missed the notification
or wants to revisit it later, there is no persistent view. The dashboard page provides:

- A persistent, readable view of today's briefing at any time
- Historical access to past briefings (scroll back through the day log)
- A richer rendering than the 4096-char Telegram format allows — proper sections, action
  item chips, calendar event cards
- Auto-refresh when a new digest is generated (polling or SSE)

## Design

### Data Storage

The current `DigestState` in `~/.nv/state/last-digest.json` stores only the latest digest
state (hash, timestamp, actions). It does not store the full rendered content. Two options:

**Option A — Extend last-digest.json**: Add a `rendered_content: Option<String>` field to
`DigestState` so the synthesized text is persisted alongside the hash.

**Option B — Separate briefing log**: Write a new file `~/.nv/state/briefing-log.jsonl` where
each line is a `BriefingEntry { id, generated_at, content, suggested_actions, sources_status }`.

This spec uses **Option B**. A JSONL log is append-only, survives daemon restarts cleanly, and
supports the history requirement without mutating the existing `DigestState` format. The log is
capped at 30 entries (rotate oldest on append).

### Rust: BriefingStore

New file `crates/nv-daemon/src/briefing_store.rs`:

```rust
pub struct BriefingEntry {
    pub id: String,              // UUID v4
    pub generated_at: DateTime<Utc>,
    pub content: String,         // Full synthesized text from DigestResult
    pub suggested_actions: Vec<SuggestedAction>,
    pub sources_status: HashMap<String, String>,
}

pub struct BriefingStore {
    path: PathBuf,  // ~/.nv/state/briefing-log.jsonl
}

impl BriefingStore {
    pub fn append(&self, entry: &BriefingEntry) -> Result<()>;
    pub fn list(&self, limit: usize) -> Result<Vec<BriefingEntry>>;
    pub fn latest(&self) -> Result<Option<BriefingEntry>>;
}
```

Cap: `append()` trims the log to the last 30 entries after each write.

### Rust: Wiring in the Digest Pipeline

In `crates/nv-daemon/src/digest/actions.rs` (or the worker that handles
`CronEvent::MorningBriefing`), after `synthesize_digest()` succeeds, call
`briefing_store.append(entry)`. The `DashboardState` gets a new field:
`briefing_store: Option<Arc<BriefingStore>>`.

### Rust: API Endpoints

Two new routes in `dashboard.rs`:

```
GET /api/briefing         — latest briefing entry (or 404 if none)
GET /api/briefing/history — array of up to 30 past entries, newest first
```

Response shape for both (single vs array wrapper):

```json
{
  "id": "...",
  "generated_at": "2026-03-25T07:00:00Z",
  "content": "-- Jira --\n...",
  "suggested_actions": [
    { "id": "digest_act_1", "label": "Close OO-142", "status": "pending" }
  ],
  "sources_status": { "jira": "ok", "calendar": "ok" }
}
```

### Frontend: BriefingPage

New file `dashboard/src/pages/BriefingPage.tsx`. Layout:

1. **Header row** — title "Morning Briefing", date of latest briefing, Refresh button
2. **Content panel** — parsed sections rendered as cards:
   - Jira card: list of issues grouped by priority, P0/P1 flagged
   - Sessions card: active Nexus sessions
   - Memory card: recent topics
   - Calendar card: today's events with time ranges
   - Suggested Actions strip: action chips with `pending` / `completed` / `dismissed` state
3. **History rail** — vertical list of past briefings by date; clicking one loads it into the
   content panel
4. **Loading skeleton** — pulse placeholders matching section layout
5. **Empty state** — "No briefing yet today. Nova generates a briefing each morning at 7am."

Section parsing: the `content` field uses `-- Section --` delimiters (from `synthesize.rs`
fallback format) or `### Section` markdown headers (from the Claude-synthesized format). A
`parseBriefingSections(content: string)` utility handles both.

### Frontend: Auto-refresh

Poll `GET /api/briefing` every 60 seconds. If `generated_at` changes, replace the displayed
content and show a toast "Briefing updated". No WebSocket dependency — polling is sufficient
for a once-daily update cadence.

### Nav Registration

Add "Briefing" entry to `NAV_ITEMS` in `Sidebar.tsx` between "Dashboard" and "Obligations"
using the `Newspaper` lucide icon (or `Sun` for morning theme). Route: `/briefing`.

## Requirements

### Req-1: BriefingStore (Rust)

New `crates/nv-daemon/src/briefing_store.rs`:
- `BriefingEntry` struct: `id` (uuid), `generated_at`, `content`, `suggested_actions`,
  `sources_status`
- `BriefingStore::append()` — writes entry as JSONL line, trims to last 30 entries
- `BriefingStore::list(limit)` — reads up to `limit` entries newest-first
- `BriefingStore::latest()` — convenience wrapper over `list(1)`
- Unit tests: append + list round-trip, cap at 30, latest returns most recent

### Req-2: Digest Pipeline Integration (Rust)

After `synthesize_digest()` (or `synthesize_digest_fallback()`) succeeds and before sending
to Telegram, call `briefing_store.append()` with the full `DigestResult`. The briefing store
receives the same content that goes to Telegram, not a stripped version.

Wire `BriefingStore` into `DashboardState` (optional Arc — store may be absent if init fails,
API returns 503 gracefully).

### Req-3: GET /api/briefing (Rust)

Handler in `dashboard.rs`:
- Reads latest entry from `BriefingStore`
- Returns 200 + JSON on success
- Returns 404 `{"error": "no briefing available"}` if store is empty
- Returns 503 `{"error": "briefing store not available"}` if store is absent

### Req-4: GET /api/briefing/history (Rust)

Handler in `dashboard.rs`:
- Accepts optional `?limit=N` query param (default 10, max 30)
- Returns 200 + `{"entries": [...]}` — newest first
- Empty array is valid (no 404 for empty history)

### Req-5: TypeScript Types (Frontend)

Add to `dashboard/src/types/api.ts`:

```typescript
export interface BriefingAction {
  id: string;
  label: string;
  status: "pending" | "completed" | "dismissed";
}

export interface BriefingEntry {
  id: string;
  generated_at: string;
  content: string;
  suggested_actions: BriefingAction[];
  sources_status: Record<string, string>;
}

export interface BriefingGetResponse {
  entry: BriefingEntry;
}

export interface BriefingHistoryGetResponse {
  entries: BriefingEntry[];
}
```

### Req-6: BriefingPage Component (Frontend)

`dashboard/src/pages/BriefingPage.tsx`:
- Fetches `GET /api/briefing` on mount
- Shows loading skeleton while fetching
- Shows empty state when 404
- Parses content into sections using `parseBriefingSections()`
- Renders each section as a distinct card
- Renders `suggested_actions` as a horizontal strip of chips
- History rail fetches `GET /api/briefing/history` and lists entries by date
- Clicking a history entry replaces the content panel with that entry

### Req-7: Section Parser Utility (Frontend)

`dashboard/src/utils/briefing.ts`:

```typescript
export interface BriefingSection {
  title: string;
  body: string;
}

export function parseBriefingSections(content: string): BriefingSection[]
```

Handles both `-- Title --` (fallback format) and `### Title` (Claude markdown format).
Returns array of `{ title, body }` objects. Unknown/unlabeled leading text becomes a section
with title `"Summary"`.

### Req-8: Auto-refresh (Frontend)

Poll `GET /api/briefing` every 60 seconds via `setInterval`. On each poll:
- Compare `generated_at` with the currently displayed entry
- If different, update state and show a brief "Briefing updated" notification

No full page reload; update is in-place.

### Req-9: Sidebar + Router Registration (Frontend)

- Add `{ to: "/briefing", label: "Briefing", icon: Sun }` to `NAV_ITEMS` in `Sidebar.tsx`
  (position: between Dashboard and Obligations)
- Add `<Route path="/briefing" element={<BriefingPage />} />` to `App.tsx`

## Scope

**IN**: BriefingStore, digest pipeline hook, two API endpoints, BriefingPage, section parser,
auto-refresh, sidebar nav entry, TS types.

**OUT**: Action dismissal/completion via dashboard (read-only view for now), push
notifications/SSE (polling sufficient), editing briefing content, configuring briefing time
from the UI, per-section source attribution beyond `sources_status`.

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/briefing_store.rs` | New: BriefingStore + BriefingEntry |
| `crates/nv-daemon/src/dashboard.rs` | Add BriefingStore field to DashboardState; add GET /api/briefing and GET /api/briefing/history handlers |
| `crates/nv-daemon/src/lib.rs` or `main.rs` | Init BriefingStore, pass to DashboardState |
| Digest pipeline (actions.rs or worker) | Call briefing_store.append() after synthesis |
| `dashboard/src/types/api.ts` | Add BriefingEntry, BriefingAction, BriefingGetResponse, BriefingHistoryGetResponse |
| `dashboard/src/utils/briefing.ts` | New: parseBriefingSections() |
| `dashboard/src/pages/BriefingPage.tsx` | New: full page component |
| `dashboard/src/components/Sidebar.tsx` | Add Briefing nav entry |
| `dashboard/src/App.tsx` | Add /briefing route |

## Risks

| Risk | Mitigation |
|------|-----------|
| DigestResult content not persisted today | BriefingStore populated on first briefing after deploy. No backfill needed — empty state is handled gracefully. |
| Content format inconsistency (fallback vs Claude) | `parseBriefingSections()` handles both `-- Title --` and `### Title` formats explicitly. Falls back to raw text display if neither matches. |
| JSONL file grows unbounded | `append()` trims to last 30 entries after each write. |
| extract-nextjs-dashboard not yet applied | Hard dependency. This spec cannot be applied until the Next.js dashboard exists as a standalone app. Blocked in wave-plan. |
| Section parser produces empty output | Parser returns a single `{ title: "Summary", body: content }` fallback so the page never renders blank. |
