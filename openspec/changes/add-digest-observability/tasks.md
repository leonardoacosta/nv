# Implementation Tasks

## DB Batch
- [x] [1.1] [P-1] Create `digest_suppression` table schema in `packages/db/src/schema/digest-suppression.ts` (hash TEXT PK, source TEXT NOT NULL, priority INT NOT NULL, last_sent_at TIMESTAMPTZ NOT NULL, expires_at TIMESTAMPTZ NOT NULL) [owner:db-engineer]
- [x] [1.2] [P-1] Export new table from `packages/db/src/schema/index.ts` barrel file [owner:db-engineer]
- [x] [1.3] [P-2] Run `pnpm drizzle-kit generate` to create migration for the new table [owner:db-engineer]

## API Batch
- [x] [2.1] [P-1] Rewrite `suppressItems` in `packages/daemon/src/features/digest/suppress.ts` to query `digest_suppression` table instead of deserializing `sentHashes` JSON blob [owner:api-engineer]
- [x] [2.2] [P-1] Add upsert logic in `suppress.ts`: on send, upsert to `digest_suppression` with `last_sent_at` and calculated `expires_at` based on priority cooldown [owner:api-engineer]
- [x] [2.3] [P-1] Add cleanup query in `suppress.ts`: `DELETE WHERE expires_at < now()` at start of each suppress call (replaces in-loop `prunedHashes` pruning) [owner:api-engineer]
- [x] [2.4] [P-1] Remove `sentHashes` from `DigestMeta` type, retain `lastDigestAt` and `weeklyStats` only [owner:api-engineer]
- [x] [2.5] [P-2] Add per-item DEBUG logging in `suppress.ts`: log suppressed items `{ item_hash, source, priority, reason: "cooldown", last_sent_at, cooldown_remaining_ms }` and passed items `{ item_hash, source, priority, reason: "new" | "cooldown_expired" }` [owner:api-engineer]
- [x] [2.6] [P-2] Add aggregate per-run logging in `suppress.ts`: `{ total_items, passed, suppressed, suppression_rate_pct }` [owner:api-engineer]
- [x] [2.7] [P-2] Write `digest_run` diary entry via `features/diary/writer.ts` after each Tier 1 run in `scheduler.ts` (pass aggregate stats from suppress result) [owner:api-engineer]
- [x] [2.8] [P-3] Add `system.digestStats` tRPC procedure in `packages/api/src/routers/system.ts` returning `{ last_run_at, items_total, items_suppressed, items_sent, suppression_by_priority, active_suppressions_count }` [owner:api-engineer]
- [x] [2.9] [P-3] Add `system.digestSuppressions` tRPC procedure in `packages/api/src/routers/system.ts` returning active suppressions list from `digest_suppression WHERE expires_at > now() ORDER BY last_sent_at DESC` [owner:api-engineer]

## UI Batch
(No UI tasks)

## E2E Batch
- [ ] [4.1] [P-3] Test: suppression persistence survives across digest runs -- insert suppression, run suppress, verify item is still suppressed via DB query [owner:e2e-engineer]
- [ ] [4.2] [P-3] Test: expired suppressions are cleaned up -- insert suppression with past `expires_at`, run suppress, verify row deleted [owner:e2e-engineer]
- [ ] [4.3] [P-3] Test: `system.digestStats` returns valid shape with correct `active_suppressions_count` matching DB state [owner:e2e-engineer]
- [ ] [4.4] [P-3] Test: `system.digestSuppressions` returns only non-expired entries ordered by `last_sent_at DESC` [owner:e2e-engineer]
