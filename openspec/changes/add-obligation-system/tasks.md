# Implementation Tasks

<!-- beads:epic:nv-c7ro -->

## Types

- [ ] [1.1] [P-1] Create `packages/daemon/src/features/obligations/types.ts` — define `ObligationStatus` string enum with values: `open`, `in_progress`, `proposed_done`, `done`, `dismissed`; define `ObligationRecord` interface with all DB-mapped fields (id, detectedAction, owner, status, priority, projectCode, sourceChannel, sourceMessage, deadline, lastAttemptAt, createdAt, updatedAt); define `CreateObligationInput` type (omits id, createdAt, updatedAt, lastAttemptAt) [owner:api-engineer]

## Store

- [ ] [2.1] [P-1] Create `packages/daemon/src/features/obligations/store.ts` — import DrizzleDb type from `@nova/db`; define `ObligationStore` class with constructor accepting `DrizzleDb` [owner:api-engineer]
- [ ] [2.2] [P-1] Implement `ObligationStore.create(input: CreateObligationInput): Promise<ObligationRecord>` — inserts row, generates UUID for id, sets `createdAt`/`updatedAt` to `new Date()`, returns inserted record [owner:api-engineer]
- [ ] [2.3] [P-1] Implement `ObligationStore.getById(id: string): Promise<ObligationRecord | null>` — drizzle `eq` query on id; returns null if not found [owner:api-engineer]
- [ ] [2.4] [P-1] Implement `ObligationStore.listByStatus(status: ObligationStatus): Promise<ObligationRecord[]>` — filters by status column; casts DB string to `ObligationStatus` [owner:api-engineer]
- [ ] [2.5] [P-1] Implement `ObligationStore.listReadyForExecution(cooldownHours = 2): Promise<ObligationRecord[]>` — filters `owner = "nova"` AND `status IN ("open", "in_progress")`; excludes rows where `last_attempt_at IS NOT NULL AND last_attempt_at > now() - interval`; orders by `priority ASC, created_at ASC` [owner:api-engineer]
- [ ] [2.6] [P-2] Implement `ObligationStore.updateStatus(id: string, status: ObligationStatus): Promise<void>` — drizzle `update` with `eq(obligations.id, id)`; also sets `updatedAt = new Date()` [owner:api-engineer]
- [ ] [2.7] [P-2] Implement `ObligationStore.updateLastAttemptAt(id: string, timestamp: Date): Promise<void>` — drizzle update; also sets `updatedAt = new Date()` [owner:api-engineer]
- [ ] [2.8] [P-2] Implement `ObligationStore.appendNote(id: string, note: string): Promise<void>` — reads current `sourceMessage`, appends `\n[{ISO timestamp}] {note}`, writes back; if sourceMessage is null, sets it to the note directly [owner:api-engineer]

## Detector

- [ ] [3.1] [P-1] Create `packages/daemon/src/features/obligations/detector.ts` — define `DetectedObligation` interface: `{ detectedAction: string; owner: "nova" | "leo"; priority: 1 | 2 | 3; projectCode: string | null; deadline: Date | null }`; import and create Anthropic client [owner:api-engineer]
- [ ] [3.2] [P-1] Implement `detectObligations(message: string, response: string, channel: string): Promise<DetectedObligation[]>` — builds focused detection prompt containing both user message and Nova's response; calls `anthropic.messages.create` (non-streaming, single turn, `max_tokens: 512`); requests JSON array output [owner:api-engineer]
- [ ] [3.3] [P-2] Parse JSON from detector response — extract text content from `response.content[0]`; locate JSON array in text (may be wrapped in markdown code block); parse with `JSON.parse`; validate each item has required fields; return `[]` on any parse/validation failure (never throws) [owner:api-engineer]
- [ ] [3.4] [P-2] Detection prompt must instruct Claude to output only `[]` if no obligations are found; ask for `owner`, `priority` (1/2/3), `projectCode` (null if unknown), `deadline` (ISO string or null), `detectedAction` (imperative verb phrase) [owner:api-engineer]

## Executor

- [ ] [4.1] [P-1] Create `packages/daemon/src/features/obligations/executor.ts` — define `ExecutorConfig` interface: `{ enabled: boolean; timeoutMs: number; cooldownHours: number; idleDebounceMs: number; pollIntervalMs: number }`; define `ObligationExecutor` class with constructor accepting `ObligationStore`, `AgentSdkClient`, `TelegramSender`, `ExecutorConfig` [owner:api-engineer]
- [ ] [4.2] [P-1] Implement `ObligationExecutor.notifyActivity()` — sets `this.lastActivityAt = Date.now()` [owner:api-engineer]
- [ ] [4.3] [P-1] Implement `ObligationExecutor.start()` — starts `setInterval(pollIntervalMs)` poll loop; each tick: if `Date.now() - lastActivityAt > idleDebounceMs && !isExecuting`, calls `tryExecuteNext()` [owner:api-engineer]
- [ ] [4.4] [P-1] Implement `ObligationExecutor.stop(): Promise<void>` — clears interval; if `isExecuting`, waits for in-flight execution to complete via a draining promise (resolve on next `isExecuting = false`) [owner:api-engineer]
- [ ] [4.5] [P-1] Implement `buildExecutionPrompt(obligation: ObligationRecord): string` — formats system prompt with detectedAction, priority, projectCode, sourceChannel, sourceMessage; instructs Nova to use tools and provide a 3-5 sentence summary when done [owner:api-engineer]
- [ ] [4.6] [P-2] Implement `ObligationExecutor.tryExecuteNext()` — calls `store.listReadyForExecution(cooldownHours)`; returns early if empty; sets `isExecuting = true`; updates status to `in_progress` and `lastAttemptAt`; builds prompt; calls `agentSdk.query()` wrapped in `Promise.race` with `timeoutMs` rejection; dispatches to success or failure handler; always sets `isExecuting = false` in `finally` block [owner:api-engineer]
- [ ] [4.7] [P-2] Implement success handler — appends note `[Auto-executed {ISO}] {summary}`; updates status to `proposed_done`; sends Telegram message with inline keyboard: `[Confirm Done]` (callback: `obligation_confirm:{id}`) and `[Reopen]` (callback: `obligation_reopen:{id}`); summary truncated to 500 chars [owner:api-engineer]
- [ ] [4.8] [P-2] Implement failure handler — appends note `[Attempt failed {ISO}] {error.message}`; keeps status as `in_progress`; sends Telegram plain message: `"Failed to complete: {detectedAction} — {error summary}"` [owner:api-engineer]

## Callbacks

- [ ] [5.1] [P-1] Create callback handlers in `packages/daemon/src/features/obligations/executor.ts` (or a separate `callbacks.ts`): `handleObligationConfirm(id, store, telegram, messageId): Promise<void>` — reads obligation, verifies `status === proposed_done`, transitions to `done`, edits Telegram message to `"Obligation confirmed."` [owner:api-engineer]
- [ ] [5.2] [P-1] Implement `handleObligationReopen(id, store, telegram, messageId): Promise<void>` — reads obligation, verifies `status === proposed_done`, transitions to `open`, edits Telegram message to `"Reopened — Nova will retry."` [owner:api-engineer]
- [ ] [5.3] [P-2] Export callback prefix constants: `OBLIGATION_CONFIRM_PREFIX = "obligation_confirm:"` and `OBLIGATION_REOPEN_PREFIX = "obligation_reopen:"` — used by `add-telegram-adapter` for routing [owner:api-engineer]

## Config

- [ ] [6.1] [P-1] Add `AutonomyConfig` interface to `packages/daemon/src/config.ts`: `{ enabled: boolean; timeoutMs: number; cooldownHours: number; idleDebounceMs: number; pollIntervalMs: number }`; add optional `autonomy?: AutonomyConfig` to `Config` type; parse from `[autonomy]` TOML section with defaults: `enabled=true`, `timeoutMs=300000`, `cooldownHours=2`, `idleDebounceMs=60000`, `pollIntervalMs=30000` [owner:api-engineer]
- [ ] [6.2] [P-2] Add `[autonomy]` section to `~/.nv/config/nv.toml` with `enabled = true` and commented defaults for all fields [owner:api-engineer]

## Barrel Export

- [ ] [7.1] [P-2] Create `packages/daemon/src/features/obligations/index.ts` — re-exports `ObligationStatus`, `ObligationRecord`, `ObligationStore`, `detectObligations`, `ObligationExecutor`, `handleObligationConfirm`, `handleObligationReopen`, `OBLIGATION_CONFIRM_PREFIX`, `OBLIGATION_REOPEN_PREFIX` [owner:api-engineer]

## Verify

- [ ] [8.1] `cd packages/daemon && pnpm typecheck` passes — zero TypeScript errors [owner:api-engineer]
- [ ] [8.2] Unit test: `ObligationStore.listReadyForExecution` returns only `owner=nova` rows with status `open`/`in_progress` and `lastAttemptAt` outside cooldown window [owner:api-engineer]
- [ ] [8.3] Unit test: `ObligationStatus.ProposedDone` round-trips through store (insert as `proposed_done`, read back as `ObligationStatus.ProposedDone`) [owner:api-engineer]
- [ ] [8.4] Unit test: `detectObligations` returns `[]` when Claude output is malformed JSON or empty string [owner:api-engineer]
- [ ] [8.5] Unit test: `ObligationExecutor` — mock `agentSdk.query` to reject after 1s, verify executor calls failure handler and sets `isExecuting = false` [owner:api-engineer]
- [ ] [8.6] [user] Manual test: send "Nova, remind me to review the Jira backlog tomorrow" via Telegram; verify obligation created in DB with `owner=leo`, `priority=2` [owner:api-engineer]
- [ ] [8.7] [user] Manual test: create a `owner=nova` obligation directly in DB; wait 60s idle; verify Nova executes it and sends Telegram summary with `[Confirm Done]` / `[Reopen]` buttons [owner:api-engineer]
- [ ] [8.8] [user] Manual test: tap `[Confirm Done]` — obligation transitions to `done`, Telegram message updates [owner:api-engineer]
- [ ] [8.9] [user] Manual test: tap `[Reopen]` — obligation transitions to `open`, Telegram message updates [owner:api-engineer]
