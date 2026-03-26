# Proposal: Add Obligation System

## Change ID
`add-obligation-system`

## Summary

Port the obligation system to TypeScript. Obligation detection via Claude analysis, Drizzle-backed
CRUD storage, and autonomous idle execution via the Anthropic Agent SDK. When no messages arrive for
60 seconds, Nova picks her highest-priority open obligation and works on it autonomously, then
proposes completion via a Telegram inline keyboard.

## Context
- Phase: 3 — Features | Wave: 6
- Stack: TypeScript (`packages/daemon/`)
- Depends on: `setup-postgres-drizzle` (obligations table in `packages/db/`), `add-agent-sdk-integration` (Agent SDK `query()` available)
- Extends: `packages/daemon/src/types.ts` (Obligation type defined in scaffold-ts-daemon)
- Related: `add-autonomous-obligation-execution` (archived Rust version — ported here), `add-telegram-adapter` (Telegram inline keyboard callbacks)

## Motivation

Nova's obligation system exists in the Rust daemon (`obligation_store.rs`, `obligation_executor.rs`)
but the TypeScript daemon port lacks it. Without this spec:

1. The TS daemon cannot detect commitments Nova makes during conversation
2. Obligations cannot be stored or queried from the Postgres schema defined in `setup-postgres-drizzle`
3. Nova remains reactive — she can only respond to messages, never proactively work on open tasks
4. The `proposed_done` flow (Confirm / Reopen inline keyboard) is missing from Telegram callbacks

The obligation system is the backbone of Nova's autonomous agency. This spec ports it cleanly
to TypeScript using Drizzle for storage and the Agent SDK for execution.

## Requirements

### Req-1: ObligationStatus Enum (`types.ts`)

Define a string enum `ObligationStatus` in `packages/daemon/src/features/obligations/types.ts`:

```typescript
export enum ObligationStatus {
  Open        = "open",
  InProgress  = "in_progress",
  ProposedDone = "proposed_done",
  Done        = "done",
  Dismissed   = "dismissed",
}
```

Also export `ObligationRecord` — the Drizzle-mapped row shape (mirrors `packages/db/src/schema/obligations.ts`):

```typescript
export interface ObligationRecord {
  id: string;
  detectedAction: string;
  owner: string;
  status: ObligationStatus;
  priority: number;
  projectCode: string | null;
  sourceChannel: string;
  sourceMessage: string | null;
  deadline: Date | null;
  lastAttemptAt: Date | null;
  createdAt: Date;
  updatedAt: Date;
}
```

Note: `obligations.ts` in `packages/db/` was specced with `status` as a plain text column using
values `pending/in_progress/done/cancelled`. This spec extends the status vocabulary — the DB
column remains `text` but the TS enum now governs all valid values. The store layer casts DB strings
to `ObligationStatus`.

### Req-2: Obligation Store (`store.ts`)

Create `packages/daemon/src/features/obligations/store.ts` — Drizzle CRUD layer.

```typescript
export class ObligationStore {
  constructor(private db: DrizzleDb) {}

  async create(input: CreateObligationInput): Promise<ObligationRecord>
  async getById(id: string): Promise<ObligationRecord | null>
  async listByStatus(status: ObligationStatus): Promise<ObligationRecord[]>
  async listReadyForExecution(cooldownHours?: number): Promise<ObligationRecord[]>
  async updateStatus(id: string, status: ObligationStatus): Promise<void>
  async updateLastAttemptAt(id: string, timestamp: Date): Promise<void>
  async appendNote(id: string, note: string): Promise<void>
}
```

`listReadyForExecution(cooldownHours = 2)`:
- Filters: `owner = "nova"` AND `status IN ("open", "in_progress")`
- Excludes: obligations where `last_attempt_at > now() - cooldown_hours`
- Orders: `priority ASC`, then `created_at ASC` (P1 first, oldest first within same priority)

`appendNote(id, note)`:
- Reads current `source_message` field (used as notes accumulator in this schema)
- Appends `[{timestamp}] {note}` with newline separator
- Writes back via `updateSet`

### Req-3: Obligation Detector (`detector.ts`)

Create `packages/daemon/src/features/obligations/detector.ts` — analyzes messages for
commitments using Claude (via the Agent SDK response stream analysis).

```typescript
export interface DetectedObligation {
  detectedAction: string;
  owner: "nova" | "leo";
  priority: 1 | 2 | 3;
  projectCode: string | null;
  deadline: Date | null;
}

export async function detectObligations(
  message: string,
  response: string,
  channel: string,
): Promise<DetectedObligation[]>
```

The detector sends a focused Claude prompt with both the user message and Nova's response, asking:
"Did Nova make any commitments, agree to do anything, or defer any action? List each as a
structured obligation with owner (nova/leo), priority (1=urgent, 2=normal, 3=low), and project
code if discernible."

The prompt requests JSON output — an array of `DetectedObligation` objects or empty array `[]`.
The detector parses the JSON response; if parsing fails, returns `[]` (never throws).

The detector is called **after** each agent response is sent, so it does not block the reply path.
It uses `anthropic.messages.create` directly (not Agent SDK query) — a single non-streaming call.

### Req-4: Obligation Executor (`executor.ts`)

Create `packages/daemon/src/features/obligations/executor.ts` — idle detection and autonomous
execution via Agent SDK `query()`.

```typescript
export interface ExecutorConfig {
  enabled: boolean;
  timeoutMs: number;        // default 300_000 (5 min)
  cooldownHours: number;    // default 2
  idleDebounceMs: number;   // default 60_000 (60s)
  pollIntervalMs: number;   // default 30_000 (30s)
}

export class ObligationExecutor {
  constructor(
    private store: ObligationStore,
    private agentSdk: AgentSdkClient,
    private telegram: TelegramSender,
    private config: ExecutorConfig,
  ) {}

  start(): void          // starts idle poll loop, non-blocking
  stop(): Promise<void>  // graceful shutdown, awaits in-flight execution
  notifyActivity(): void // called by message handler on each incoming/outgoing message
                         // resets the idle debounce timer
}
```

**Idle detection:**
- `notifyActivity()` updates `lastActivityAt = Date.now()`
- Poll loop runs every `pollIntervalMs`
- Nova is "idle" when `Date.now() - lastActivityAt > idleDebounceMs` AND no execution is in-flight
- On idle detection: call `tryExecuteNext()`

**`tryExecuteNext()`:**
1. Call `store.listReadyForExecution(cooldownHours)` — pick first result
2. If no obligations: do nothing, wait for next poll
3. Set `isExecuting = true`
4. Update obligation status to `in_progress` via `store.updateStatus()`
5. Update `lastAttemptAt` via `store.updateLastAttemptAt()`
6. Build execution prompt (see Req-5)
7. Call `agentSdk.query(prompt, { maxTurns: 30, timeoutMs })` — Agent SDK handles tool loop
8. Handle result (see Req-6)
9. Set `isExecuting = false`

**Safety guards:**
- Only one obligation executes at a time (`isExecuting` flag)
- `timeoutMs` is enforced via `Promise.race` with a timeout rejection
- If Agent SDK query throws, treat as failure (see Req-6 failure path)

### Req-5: Execution Prompt

```typescript
function buildExecutionPrompt(obligation: ObligationRecord): string
```

Builds the system prompt for the Agent SDK query:

```
You are Nova. You have an obligation to complete:

**Action**: {detectedAction}
**Priority**: P{priority}
**Project**: {projectCode ?? "general"}
**Source**: {sourceChannel} — "{sourceMessage ?? "no source message"}"

Use your available tools to fulfill this obligation completely. When finished, provide a concise
summary (3–5 sentences) of what you accomplished and any relevant findings.
```

### Req-6: Result Reporting

**On success** (Agent SDK returns non-empty text):
1. Call `store.appendNote(id, "[Auto-executed {ISO timestamp}] {summary}")`
2. Call `store.updateStatus(id, ObligationStatus.ProposedDone)`
3. Send Telegram message to Leo: brief summary (≤500 chars) + inline keyboard:
   ```
   [Confirm Done]  [Reopen]
   ```
   Callback data: `obligation_confirm:{id}` and `obligation_reopen:{id}`

**On failure** (timeout, error, empty response):
1. Call `store.appendNote(id, "[Attempt failed {ISO timestamp}] {error message}")`
2. Keep status as `in_progress` (do not revert to `open`)
3. Send Telegram message: `"Failed to complete: {detectedAction} — {error summary}"`
4. The `cooldownHours` guard in `listReadyForExecution` prevents immediate retry

### Req-7: Telegram Callback Handlers

The `add-telegram-adapter` spec manages callback routing. This spec defines the handler logic that
the Telegram adapter will call when callbacks arrive.

```typescript
export async function handleObligationConfirm(
  id: string,
  store: ObligationStore,
  telegram: TelegramSender,
  messageId: number,
): Promise<void>
// Transitions proposed_done -> done, edits message to show "Obligation confirmed."

export async function handleObligationReopen(
  id: string,
  store: ObligationStore,
  telegram: TelegramSender,
  messageId: number,
): Promise<void>
// Transitions proposed_done -> open, edits message to show "Reopened — Nova will retry."
```

Callback routing pattern: `obligation_confirm:` and `obligation_reopen:` prefixes.

### Req-8: Config Integration

Add executor config to daemon config:

```toml
# ~/.nv/config/nv.toml — [autonomy] section
[autonomy]
enabled = true
timeout_ms = 300000
cooldown_hours = 2
idle_debounce_ms = 60000
poll_interval_ms = 30000
```

Export `AutonomyConfig` from `packages/daemon/src/config.ts` and wire into `ObligationExecutor`
constructor.

## Scope
- **IN**: `types.ts`, `store.ts`, `detector.ts`, `executor.ts`, callback handlers, config integration
- **OUT**: Implementing the Agent SDK client (owned by `add-agent-sdk-integration`), Telegram adapter callback routing (owned by `add-telegram-adapter`), dashboard visibility improvements (`improve-obligation-visibility`), proactive reminder watcher (separate spec)

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/features/obligations/types.ts` | New: `ObligationStatus` enum, `ObligationRecord` interface |
| `packages/daemon/src/features/obligations/store.ts` | New: Drizzle CRUD for obligations table |
| `packages/daemon/src/features/obligations/detector.ts` | New: commitment detection via Claude |
| `packages/daemon/src/features/obligations/executor.ts` | New: idle detection + autonomous execution |
| `packages/daemon/src/features/obligations/index.ts` | New: barrel export |
| `packages/daemon/src/config.ts` | Add `AutonomyConfig` type + `[autonomy]` config key |
| `~/.nv/config/nv.toml` | Add `[autonomy]` section |

## Risks
| Risk | Mitigation |
|------|-----------|
| Agent SDK timeout leaves obligation stuck in `in_progress` | `Promise.race` + error handler always calls `appendNote` + keeps `in_progress`; cooldown prevents immediate retry |
| Detector noise (false positives) | Low-stakes: spurious obligations can be dismissed; Nova only auto-executes `owner=nova` items |
| `appendNote` clobbers real source_message data | Notes are accumulated in a dedicated accumulator pattern — source_message is the initial seed only; an `obligation_notes` column should be added to the schema to avoid this collision |
| `ObligationStatus.ProposedDone` unknown to DB | DB column is plain `text` — any enum value stores and retrieves correctly; status values are only validated at the TS layer |
| Telegram callback handler integration gap | Handlers are standalone functions; wired in `add-telegram-adapter` — this spec only defines the logic, not the routing |
