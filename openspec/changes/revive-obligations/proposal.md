# Proposal: Revive Obligations

## Change ID
`revive-obligations`

## Summary

Fix the obligation pipeline so Nova actually creates and executes obligations autonomously. The detector and executor exist as dead code (never wired into the message flow or daemon init). This spec debugs the wiring, adds budget controls ($5/day cap, model routing by priority), escalation after repeated failures, quiet hours enforcement, a manual `/obligation` Telegram command, and watcher-to-obligation bridging.

## Context
- Depends on: obligation schema (`packages/db/src/schema/obligations.ts`), ObligationStore, ObligationExecutor, detectObligations -- all exist but are not wired
- Conflicts with: none
- Current state: The `obligations` Postgres table exists but is empty. The detector (`detector.ts`) calls claude-opus-4-5 to classify messages but is never invoked from the message flow. The executor (`executor.ts`) has a poll loop with idle detection but is never instantiated in `index.ts`. The ObligationStore has full CRUD. Callbacks for confirm/reopen are wired in `index.ts`. The `/ob` command lists obligations but always shows "No active obligations."
- Obligation schema: uuid id, text detected_action, text owner, text status, integer priority, text project_code, text source_channel, text source_message, timestamp deadline, timestamp last_attempt_at, timestamp created_at, timestamp updated_at
- Obligation statuses: open, in_progress, proposed_done, done, dismissed
- Missing from schema: attempt_count (needed for escalation), escalated status
- Watcher quiet hours: `isQuietHours(now, config)` in `packages/daemon/src/features/watcher/proactive.ts` -- reusable
- Agent: `packages/daemon/src/brain/agent.ts` -- processes messages, returns AgentResponse with text/toolCalls
- Config: `config/nv.toml` has `[autonomy]` section with enabled, timeout_ms, cooldown_hours, idle_debounce_ms, poll_interval_ms
- Executor uses Agent SDK `query()` with MCP servers and builtin tools, already handles timeout via Promise.race

## Motivation

Nova's obligation system was built end-to-end (schema, store, detector, executor, callbacks) but never connected. Two critical wiring gaps prevent it from working:

1. **Detection gap**: `detectObligations()` is never called after `agent.processMessage()` in the message routing loop (`index.ts`). Obligations are never created from conversations.
2. **Execution gap**: `ObligationExecutor` is never instantiated or started in `index.ts`. Even if obligations existed, they would never be picked up for autonomous work.

Beyond wiring, the system lacks cost controls for autonomous work (unbounded API spend), has no escalation path for repeated failures, ignores quiet hours, and provides no manual creation mechanism.

## Requirements

### Req-1: Wire Obligation Detection into Message Flow

In `packages/daemon/src/index.ts`, after `agent.processMessage()` returns and the response is sent to Telegram:

- Call `detectObligations(msg.content, response.text, msg.channel, config.vercelGatewayKey)` fire-and-forget
- For each detected obligation, call `obligationStore.create()` with status `ObligationStatus.Open` and sourceChannel `telegram`
- Send a Telegram notification for nova-owned obligations with the obligation keyboard (approve/snooze/dismiss)
- Log detected obligations at info level

Detection uses Haiku (not Opus) to minimize cost -- change the model in `detector.ts` from `claude-opus-4-5` to `claude-haiku-3-5` since classification does not require deep reasoning.

### Req-2: Wire Obligation Executor into Daemon Init

In `packages/daemon/src/index.ts`:

- Instantiate `ObligationExecutor` with the obligation store, gateway key, telegram adapter, chat ID, and autonomy config
- Call `executor.start()` after agent initialization
- Call `executor.notifyActivity()` on every inbound message (resets idle timer)
- Call `executor.stop()` in the shutdown handler
- Pass the full `Config` so the executor can build MCP servers

### Req-3: Budget Gate

Add daily budget tracking to the executor to prevent runaway spending:

- Add `dailyBudgetUsd` (default 5.0) and `autonomyBudgetPct` (default 0.20, meaning 20% of weekly budget reserved for autonomous work) to `AutonomyConfig`
- Track token usage per execution in the executor (sum input_tokens + output_tokens from Agent SDK responses)
- Maintain a daily spend counter (reset at midnight UTC) using estimated cost: input tokens * $3/M + output tokens * $15/M (Sonnet pricing) or input * $0.80/M + output * $4/M (Haiku pricing)
- Before executing, check if daily spend would likely exceed `dailyBudgetUsd`. If so, skip and log "Daily budget exhausted"
- Store daily spend in memory (resets on daemon restart, which is acceptable for a soft cap)

### Req-4: Model Routing by Priority

Route obligation execution to different models based on priority to control costs:

- P0-P1 (urgent/critical): Use Sonnet (`claude-sonnet-4-6`) -- best reasoning for important work
- P2+ (normal/low): Use Haiku (`claude-haiku-3-5`) -- 4x cheaper for routine tasks
- Add `model` parameter to `runAgentQuery()` or configure it in the executor based on obligation priority
- The model selection affects both the agent query and the cost calculation in the budget gate

### Req-5: Escalation After Repeated Failures

Add escalation logic when an obligation fails repeatedly:

- Add `attempt_count` column (integer, default 0) to the obligations table and `attemptCount` to the store/types
- Add `Escalated = "escalated"` to `ObligationStatus` enum
- Add `maxAttempts` (default 3) to `AutonomyConfig`
- In the executor's `handleFailure()`: increment attempt count. If `attemptCount >= maxAttempts`, set status to `escalated` and send a Telegram notification with an inline keyboard: [Retry] [Dismiss] [Take Over]
- "Take Over" changes owner to "leo" and status to "open"
- Add callback handlers for the escalation keyboard in `callbacks.ts`

### Req-6: Quiet Hours Enforcement

Prevent autonomous obligation execution during quiet hours:

- Import `isQuietHours` from the watcher module
- In the executor's `tick()`, check `isQuietHours(new Date(), proactiveWatcherConfig)` before calling `tryExecuteNext()`
- Reuse the same `quietStart`/`quietEnd` from `[proactive_watcher]` config (22:00-07:00)
- Pass the `ProactiveWatcherConfig` to the executor constructor

### Req-7: Manual Obligation Creation via Telegram

Add `/obligation` command for manual obligation creation:

- Create `packages/daemon/src/telegram/commands/obligation.ts` with `buildObligationReply(text)` that:
  - Parses format: `/obligation <action text>` (priority defaults to P2, owner defaults to "nova")
  - Optional flags: `p1`, `p2`, `p3` at the end for priority override
  - Creates obligation via the daemon's HTTP endpoint or directly via ObligationStore
  - Returns confirmation message with obligation ID
- Register `/obligation` in `telegram.ts` command handlers
- Add to bot commands list in `registerCommands()`

### Req-8: Watcher Findings to Obligations

Bridge the proactive watcher's findings into the obligation system:

- When the watcher detects a stale obligation or approaching deadline and sends a reminder, also create or update an obligation if one doesn't already exist for that item
- This is a lightweight bridge -- the watcher already scans obligations, so this adds a "create if missing" step for watcher-detected action items from other sources (e.g., stale Jira tickets could become obligations)
- Implement as an optional callback from the watcher scan to the ObligationStore

### Req-9: Config Expansion

Expand `[autonomy]` in `config/nv.toml`:

```toml
[autonomy]
enabled = true
timeout_ms = 300000
cooldown_hours = 2
idle_debounce_ms = 60000
poll_interval_ms = 30000
daily_budget_usd = 5.0
autonomy_budget_pct = 0.20
max_attempts = 3
```

Update `AutonomyConfig` interface and `TomlConfig` type in `config.ts` to parse these new fields with defaults.

### Req-10: DB Migration for attempt_count

Add `attempt_count` integer column (default 0) to the `obligations` table:

- Update `packages/db/src/schema/obligations.ts` to include `attemptCount: integer("attempt_count").notNull().default(0)`
- Generate migration via `pnpm drizzle-kit generate`
- Update `ObligationRow` and `ObligationRecord` in the store/types to include the new field

## Risks

1. **Cost overrun**: Even with the $5/day cap, a misconfigured detector could create many obligations. Mitigated by the budget gate and Haiku for detection.
2. **Agent SDK errors**: The executor already handles timeouts and errors gracefully. Escalation after 3 failures provides an escape hatch.
3. **Quiet hours drift**: Using local system time. The daemon runs in a fixed timezone (CDT), so this is acceptable.

## Alternatives Considered

1. **Skip budget gate, rely on manual oversight**: Rejected -- autonomous work can run while the user sleeps, so cost controls are essential.
2. **Use a separate cost-tracking table**: Overkill for v1 -- in-memory daily counter is sufficient since the daemon rarely restarts.
3. **Per-obligation cost tracking in the DB**: Future enhancement. For now, aggregate daily tracking is enough.
