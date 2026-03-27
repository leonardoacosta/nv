# Implementation Tasks

<!-- beads:epic:TBD -->

## DB Batch

- [x] [1.1] Add `routing_tier` (integer, nullable) and `routing_confidence` (real, nullable) columns to the `diary` table in `@nova/db`. Generate migration with `pnpm drizzle-kit generate`. These columns are nullable so existing diary entries are unaffected. [owner:db-engineer]

## API Batch

### Group A: Core router infrastructure

- [x] [2.1] Create `packages/daemon/src/brain/router.ts` -- export `RouteResult` type (`{ tier: RouteTier, tool?: string, params?: Record<string, unknown>, confidence: number }`) and `MessageRouter` class. Constructor accepts `KeywordRouter` and `EmbeddingRouter | null` (null when Tier 2 disabled). `route(text: string): Promise<RouteResult>` method evaluates Tier 0 (starts with `/`), Tier 1, Tier 2, Tier 3 in order, returning on first match. Export a `formatToolResponse(result: unknown): string` function that converts fleet JSON into Markdown (use `text` field if present, format arrays as bulleted lists, fall back to JSON code block). [owner:api-engineer]

- [x] [2.2] Create `packages/daemon/src/brain/keyword-router.ts` -- export `KeywordRouter` class with a static pattern table. Each entry: `{ regex: RegExp, tool: string, port: number, extractParams?: (match: RegExpMatchArray, text: string) => Record<string, unknown> }`. The `match(text: string): KeywordMatch | null` method lowercases input, tests each regex sequentially, returns first match with `confidence: 0.95` or `null`. Include 20+ patterns covering: calendar_today, calendar_upcoming, email_inbox, email_send, weather_current, reminders_list, reminder_create, memory_read, messages_recent, contact_lookup, health_check, datetime_now. [owner:api-engineer]

- [x] [2.3] Create `packages/daemon/src/brain/embedding-router.ts` -- export `EmbeddingRouter` class with static `create(): Promise<EmbeddingRouter>` factory. On create: load `Xenova/all-MiniLM-L6-v2` via `@xenova/transformers` pipeline (feature-extraction), load all intent JSON files from `intents/` dir, compute centroids for any intent missing one (mean of encoded utterances), write centroids back to JSON files. `match(text: string): Promise<EmbeddingMatch | null>` method encodes input, computes cosine similarity against all centroids, returns best match if similarity >= threshold (default 0.82, overridable via `NV_EMBEDDING_THRESHOLD` env var) or `null`. [owner:api-engineer]

### Group B: Intent seed data

- [x] [2.4] Create `packages/daemon/src/brain/intents/` directory with 12 JSON files, one per intent. Each file: `{ "tool": string, "port": number, "utterances": string[] }`. 5-10 utterances per intent covering natural language variations. Files: `calendar-today.json`, `calendar-upcoming.json`, `email-inbox.json`, `email-send.json`, `weather-current.json`, `reminders-list.json`, `reminder-create.json`, `memory-read.json`, `messages-recent.json`, `contact-lookup.json`, `health-check.json`, `datetime-now.json`. [owner:api-engineer]

### Group C: Integration

- [x] [2.5] Add `@xenova/transformers` to `packages/daemon/package.json` dependencies. [owner:api-engineer]

- [x] [2.6] Extend `DiaryWriteInput` in `packages/daemon/src/features/diary/writer.ts` with optional `routingTier?: number` and `routingConfidence?: number` fields. Update `writeEntry` to include these fields in the `db.insert(diary).values()` call (pass `null` when undefined). [owner:api-engineer]

- [x] [2.7] Modify `packages/daemon/src/index.ts` -- import `MessageRouter`, `KeywordRouter`, `EmbeddingRouter`. After `initFleetClient()` and before Telegram adapter setup, initialize the router: create `KeywordRouter`, attempt `EmbeddingRouter.create()` (catch failure, log warning, set to null), create `MessageRouter(keywordRouter, embeddingRouter)`. In the `telegram.onMessage` handler, after callback routing guards and before the agent loop: call `router.route(data)`, if tier is 1 or 2 execute the fleet tool via `fleetPost(result.port, "/execute", { tool: result.tool, params: result.params })`, format response with `formatToolResponse`, send via Telegram, log diary entry with routing metadata, and return. Keep typing indicator for routed messages. [owner:api-engineer]

## UI Batch

(No UI changes -- routed responses use the same Telegram send path as agent responses)

## E2E Batch

- [x] [4.1] Verify daemon builds cleanly: `pnpm typecheck` passes with new files. No circular imports between router, keyword-router, embedding-router, and index.ts. [owner:e2e-engineer]

- [x] [4.2] Unit test `KeywordRouter.match()` -- verify that "what's on my calendar" matches `calendar_today`, "check my email" matches `email_inbox`, "tell me about quantum physics" returns `null` (falls through to agent). Create `packages/daemon/tests/keyword-router.test.ts`. [owner:e2e-engineer]

- [x] [4.3] Unit test `MessageRouter.route()` -- verify cascade order: Tier 0 for `/start`, Tier 1 for "what's on my calendar", Tier 3 for ambiguous text. Mock `EmbeddingRouter` to test Tier 2 in isolation. Create `packages/daemon/tests/router.test.ts`. [owner:e2e-engineer]

- [x] [4.4] Verify diary schema migration applies cleanly: `routing_tier` and `routing_confidence` columns exist and accept null values. [owner:e2e-engineer]
