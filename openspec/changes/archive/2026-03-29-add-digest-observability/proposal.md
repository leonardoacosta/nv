# Proposal: Add Digest Observability

## Change ID
`add-digest-observability`

## Summary

Add observability to the digest suppression system. The 3-tier digest system uses hash-based suppression with TTLs (P0: 30min, P1: 4hr, P2: 12hr) to deduplicate items, but there is no logging of what was suppressed, no dashboard visibility into suppression rates, and TTL state is stored as a serialized JSON blob in the `memory` table (`_digest_meta` topic) -- a daemon restart does not clear it, but the format is opaque, unparseable without custom code, and impossible to query relationally.

## Context
- Digest system: `packages/daemon/src/features/digest/`
- Suppression logic: `suppress.ts` -- hashes each `DigestItem` via SHA-256 of `source:title:detail`, tracks `sentHashes` (hash -> unix ms timestamp) in a `DigestMeta` JSON blob persisted to the `memory` table under topic `_digest_meta`
- Cooldown defaults in `config.ts`: P0 = 1,800,000ms (30min), P1 = 14,400,000ms (4hr), P2 = 43,200,000ms (12hr), hash TTL = 172,800,000ms (48hr)
- Hash pruning: entries older than `hashTtlMs` (48h) are discarded on each suppress call
- Scheduler: `scheduler.ts` runs Tier 1 (thin digest at 7am/12pm/5pm weekdays), Tier 2 (weekly LLM synthesis Monday 9am), P0 real-time (every 5min)
- Existing tRPC system router: `packages/api/src/routers/system.ts` -- already has `health`, `stats`, `fleetStatus`, `activityFeed` procedures
- DB schema directory: `packages/db/src/schema/`

## Motivation

When a user asks "why didn't I get notified about X?", there is currently no way to answer. The suppression system is a black box:

1. **No logging** -- suppressed items are silently `continue`d in the filter loop with no trace
2. **No queryable state** -- suppression data lives as a JSON blob inside a `memory` row, not as individual records that can be filtered by source, priority, or time
3. **No aggregate metrics** -- no suppression rate tracking, no per-priority breakdown, no trend data
4. **Hash collision risk is invisible** -- if SHA-256 of `source:title:detail` collides for different items, they get incorrectly suppressed with zero evidence

Adding observability enables debugging missed notifications and tuning cooldown parameters with data.

## Requirements

### Req-1: Suppression logging

In the `suppressItems` function (`suppress.ts`):

- Log each suppressed item at DEBUG level: `{ item_hash, source, priority, reason: "cooldown", last_sent_at, cooldown_remaining_ms }`
- Log each passed-through item at DEBUG level: `{ item_hash, source, priority, reason: "new" | "cooldown_expired" }`
- Aggregate per-run: `{ total_items, passed, suppressed, suppression_rate_pct }`
- Write aggregate to diary as a `digest_run` entry type (via the diary writer from `features/diary/writer.ts`)

### Req-2: Persistent suppression state

Replace the `DigestMeta.sentHashes` JSON blob (stored in the `memory` table under `_digest_meta`) with a dedicated DB table:

- Table: `digest_suppression` (hash TEXT PK, source TEXT NOT NULL, priority INT NOT NULL, last_sent_at TIMESTAMPTZ NOT NULL, expires_at TIMESTAMPTZ NOT NULL)
- On suppress check: query the `digest_suppression` table instead of deserializing the JSON blob
- On send: upsert to `digest_suppression` with new `last_sent_at` and calculated `expires_at` (based on priority cooldown)
- Cleanup: `DELETE WHERE expires_at < now()` on each digest run (auto-purge expired entries, replacing the current in-loop pruning of `prunedHashes`)
- The `_digest_meta` memory row retains `lastDigestAt` and `weeklyStats` only -- `sentHashes` is removed from `DigestMeta`

### Req-3: Dashboard visibility

Add tRPC procedure `system.digestStats`:

- Returns: `{ last_run_at, items_total, items_suppressed, items_sent, suppression_by_priority: { p0: number, p1: number, p2: number }, active_suppressions_count }`
- `last_run_at` sourced from `lastDigestAt` in `_digest_meta`
- `active_suppressions_count` from `SELECT count(*) FROM digest_suppression WHERE expires_at > now()`
- `suppression_by_priority` from the most recent `digest_run` diary entry
- Queryable from the dashboard automations page

### Req-4: Suppression audit

Add tRPC procedure `system.digestSuppressions`:

- Returns: list of currently active suppressions `{ hash, source, priority, last_sent_at, expires_at }[]`
- Query: `SELECT * FROM digest_suppression WHERE expires_at > now() ORDER BY last_sent_at DESC`
- Enables manual debugging: "show me what is being suppressed right now"

## Scope
- **IN**: `packages/daemon/src/features/digest/suppress.ts` (DB-backed suppression), `packages/db/src/schema/digest-suppression.ts` (new table), `packages/api/src/routers/system.ts` (new procedures), `packages/daemon/src/features/digest/scheduler.ts` (diary logging per run)
- **OUT**: Digest scheduling logic (unchanged), Telegram delivery (unchanged), briefing system (separate), weekly stats / Tier 2 synthesis (unchanged), dashboard UI changes (future spec)

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/features/digest/suppress.ts` | Rewrite -- replace JSON blob `sentHashes` with `digest_suppression` table queries; add per-item DEBUG logging; add aggregate diary logging |
| `packages/db/src/schema/digest-suppression.ts` | New -- `digest_suppression` table schema |
| `packages/api/src/routers/system.ts` | Extended -- `digestStats` and `digestSuppressions` procedures |
| `packages/daemon/src/features/digest/scheduler.ts` | Extended -- pass logger to `suppressItems`, write `digest_run` diary entry after each Tier 1 run |
| `packages/daemon/src/config.ts` | None -- cooldown values unchanged, `hashTtlMs` becomes the default `expires_at` ceiling |

## Risks
| Risk | Mitigation |
|------|-----------|
| DB query latency on hot path (5min P0 checks, Tier 1 runs) | Single indexed query on `hash` PK, sub-ms at this scale (typically <50 active suppressions) |
| Migration: existing `sentHashes` JSON data lost | Clean slate is acceptable -- suppressions re-establish within one cycle (max 12hr for P2) |
| Diary spam from frequent digest runs | Aggregate logging only (one diary entry per run, not per item); P0 real-time runs log only when items exist |
| Schema addition requires Drizzle migration | Standard `drizzle-kit generate` workflow, no manual SQL |
