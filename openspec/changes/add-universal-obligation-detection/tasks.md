# Implementation Tasks

## DB Batch
- [x] [1.1] [P-1] Add `detection_source` (text, nullable) and `routed_tool` (text, nullable) columns to obligations schema in `packages/db/src/schema/obligations.ts` [owner:db-engineer]
- [x] [1.2] [P-1] Generate Drizzle migration for new columns via `pnpm drizzle-kit generate` [owner:db-engineer]

## API Batch
- [x] [2.1] [P-1] Create `packages/daemon/src/features/obligations/signal-detector.ts` with regex pattern matching, confidence scoring, and 2+ signal threshold logic [owner:api-engineer]
- [x] [2.2] [P-1] Add `DetectionSource` type ("tier1" | "tier2" | "tier3" | "manual") and `detectionSource`/`routedTool` fields to `ObligationRecord` and `CreateObligationInput` in `packages/daemon/src/features/obligations/types.ts` [owner:api-engineer]
- [x] [2.3] [P-2] Add `ObligationRow.detection_source` and `ObligationRow.routed_tool` to row interface and `rowToRecord` mapping in `packages/daemon/src/features/obligations/store.ts` [owner:api-engineer]
- [x] [2.4] [P-2] Update `ObligationStore.create()` INSERT query to include `detection_source` and `routed_tool` columns in `packages/daemon/src/features/obligations/store.ts` [owner:api-engineer]
- [x] [2.5] [P-2] Add Haiku-model lightweight detection function to `packages/daemon/src/features/obligations/detector.ts` that accepts signal-detector output and returns obligation or null [owner:api-engineer]
- [x] [2.6] [P-2] Add dedup-by-message-ID check to `ObligationStore` — skip creation if obligation already exists for the same `source_message` [owner:api-engineer]
- [x] [2.7] [P-3] Add post-routing obligation hook in `packages/daemon/src/brain/router.ts` — after Tier 1/2 dispatch, run signal-detector async (fire-and-forget), enqueue Haiku detection job if signals detected [owner:api-engineer]
- [x] [2.8] [P-3] Add in-memory hourly rate limiter (max 10 detection jobs/hour) to the post-routing hook in `packages/daemon/src/brain/router.ts` [owner:api-engineer]
- [x] [2.9] [P-3] Export `SignalResult` type and `detectSignals` function from `packages/daemon/src/features/obligations/index.ts` [owner:api-engineer]

## UI Batch
(No UI tasks)

## E2E Batch
- [ ] [4.1] [P-1] Test: signal-detector returns correct signals and confidence for obligation-bearing messages (deadline patterns, commitment phrases) in `packages/daemon/src/features/obligations/obligations.test.ts` [owner:e2e-engineer]
- [ ] [4.2] [P-1] Test: signal-detector returns `detected: false` for casual messages without obligation signals [owner:e2e-engineer]
- [ ] [4.3] [P-2] Test: Tier 1 routed message with obligation signals triggers Haiku detection and creates obligation with `detectionSource: "tier1"` and correct `routedTool` [owner:e2e-engineer]
- [ ] [4.4] [P-2] Test: Tier 2 routed message with obligation signals creates obligation with `detectionSource: "tier2"` [owner:e2e-engineer]
- [ ] [4.5] [P-2] Test: rate limiter caps detection jobs at 10/hour — 11th signal-detected message is skipped [owner:e2e-engineer]
- [ ] [4.6] [P-2] Test: dedup prevents duplicate obligation when same message triggers both Tier 1/2 signal detection and Tier 3 full detection [owner:e2e-engineer]
