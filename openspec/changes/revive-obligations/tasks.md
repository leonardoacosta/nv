# Implementation Tasks

<!-- beads:epic:TBD -->

## DB: Schema + Migration

- [ ] [1.1] [P-1] Modify packages/db/src/schema/obligations.ts -- add attemptCount: integer("attempt_count").notNull().default(0) column [owner:db-engineer]
- [ ] [1.2] [P-1] Run pnpm drizzle-kit generate to create migration for attempt_count column [owner:db-engineer]
- [ ] [1.3] [P-1] Modify packages/daemon/src/features/obligations/types.ts -- add Escalated = "escalated" to ObligationStatus enum, add attemptCount: number to ObligationRecord and CreateObligationInput [owner:db-engineer]
- [ ] [1.4] [P-1] Modify packages/daemon/src/features/obligations/store.ts -- add attemptCount to ObligationRow, rowToRecord mapping, create() INSERT, and new incrementAttemptCount(id) method that does UPDATE obligations SET attempt_count = attempt_count + 1 RETURNING attempt_count [owner:db-engineer]

## Config: Expand AutonomyConfig

- [ ] [2.1] [P-1] Modify packages/daemon/src/config.ts -- add dailyBudgetUsd: number, autonomyBudgetPct: number, maxAttempts: number to AutonomyConfig interface. Add daily_budget_usd, autonomy_budget_pct, max_attempts to TomlConfig.autonomy. Parse with defaults (5.0, 0.20, 3) in loadConfig() [owner:api-engineer]
- [ ] [2.2] [P-1] Modify config/nv.toml -- add daily_budget_usd = 5.0, autonomy_budget_pct = 0.20, max_attempts = 3 to [autonomy] section [owner:api-engineer]

## API: Wire Detection into Message Flow

- [ ] [3.1] [P-1] Modify packages/daemon/src/features/obligations/detector.ts -- change model from "claude-opus-4-5" to "claude-haiku-3-5" to reduce detection cost [owner:api-engineer]
- [ ] [3.2] [P-1] Modify packages/daemon/src/index.ts -- import detectObligations from obligations. After agent.processMessage() returns and response is sent to Telegram, call detectObligations(msg.content, response.text, msg.channel, config.vercelGatewayKey) fire-and-forget. For each detected obligation, call obligationStore.create() with status Open, sourceChannel "telegram", sourceMessage msg.content. Log at info level. For nova-owned obligations, send Telegram notification with obligationKeyboard [owner:api-engineer]
- [ ] [3.3] [P-2] Import obligationKeyboard from telegram.ts in the detection wiring to send inline keyboard with approve/snooze/dismiss for newly detected obligations [owner:api-engineer]

## API: Wire Executor into Daemon Init

- [ ] [4.1] [P-1] Modify packages/daemon/src/index.ts -- import ObligationExecutor. After NovaAgent is ready, instantiate ObligationExecutor with (obligationStore, gatewayKey, telegram, telegramChatId, autonomyConfig, config). Call executor.start(). In the shutdown handler, call executor.stop(). On every inbound telegram message (before routing to agent), call executor.notifyActivity() [owner:api-engineer]

## API: Budget Gate

- [ ] [5.1] [P-1] Modify packages/daemon/src/features/obligations/executor.ts -- add private dailySpendUsd: number = 0 and dailyResetDate: string (YYYY-MM-DD) fields. Add resetDailySpendIfNeeded() that checks if current UTC date differs from dailyResetDate and resets counter. Add estimateCost(tokensIn, tokensOut, model) method using Sonnet pricing ($3/$15 per M) for P0-P1 and Haiku pricing ($0.80/$4 per M) for P2+ [owner:api-engineer]
- [ ] [5.2] [P-1] In tryExecuteNext(), call resetDailySpendIfNeeded(), then check if dailySpendUsd >= config.dailyBudgetUsd. If so, log "Daily budget exhausted" and return. After execution, parse token usage from agent response and add estimated cost to dailySpendUsd [owner:api-engineer]

## API: Model Routing

- [ ] [6.1] [P-1] Modify packages/daemon/src/features/obligations/executor.ts -- add selectModel(priority: number) method returning "claude-sonnet-4-6" for P0-P1 and "claude-haiku-3-5" for P2+. Pass model to runAgentQuery(). Update runAgentQuery() signature to accept model parameter and use it in the query() options or ANTHROPIC_MODEL env [owner:api-engineer]

## API: Escalation

- [ ] [7.1] [P-1] Modify packages/daemon/src/features/obligations/executor.ts -- in handleFailure(), call store.incrementAttemptCount(). If returned count >= config.maxAttempts, call store.updateStatus(id, ObligationStatus.Escalated) and send Telegram message with escalation keyboard: [Retry] [Dismiss] [Take Over] [owner:api-engineer]
- [ ] [7.2] [P-1] Modify packages/daemon/src/features/obligations/callbacks.ts -- add OBLIGATION_ESCALATION_RETRY_PREFIX, OBLIGATION_ESCALATION_DISMISS_PREFIX, OBLIGATION_ESCALATION_TAKEOVER_PREFIX constants. Add handleEscalationRetry (resets attempt_count to 0, sets status to open), handleEscalationDismiss (sets status to dismissed), handleEscalationTakeover (sets owner to "leo", status to open, resets attempt_count) [owner:api-engineer]
- [ ] [7.3] [P-1] Modify packages/daemon/src/index.ts -- wire escalation callback routing for the three new prefixes, following the existing obligation_confirm/reopen pattern [owner:api-engineer]

## API: Quiet Hours

- [ ] [8.1] [P-1] Modify packages/daemon/src/features/obligations/executor.ts -- import isQuietHours from watcher module. Add proactiveWatcherConfig to constructor. In tick(), check isQuietHours(new Date(), proactiveWatcherConfig) and skip if true [owner:api-engineer]
- [ ] [8.2] [P-1] Modify packages/daemon/src/index.ts -- pass config.proactiveWatcher to ObligationExecutor constructor [owner:api-engineer]

## API: Manual /obligation Command

- [ ] [9.1] [P-2] Create packages/daemon/src/telegram/commands/obligation.ts -- buildObligationReply(argsText, storeFn) that parses "/obligation <action text> [p1|p2|p3]", defaults to P2 and owner "nova". Calls storeFn to create obligation. Returns confirmation with obligation ID and detected action. Returns usage hint if argsText is empty [owner:api-engineer]
- [ ] [9.2] [P-2] Modify packages/daemon/src/channels/telegram.ts -- import buildObligationReply. Add bot.onText regex for /obligation command. Register handler that calls buildObligationReply with a store function wrapping obligationStore.create(). Note: store access requires passing the store instance or using a factory pattern [owner:api-engineer]
- [ ] [9.3] [P-2] Modify packages/daemon/src/channels/telegram.ts -- add { command: "obligation", description: "Create an obligation manually" } to registerCommands() bot command list [owner:api-engineer]

## API: Watcher-to-Obligation Bridge

- [ ] [10.1] [P-2] Modify packages/daemon/src/features/watcher/proactive.ts -- accept optional ObligationStore in constructor. After sending a reminder for a stale or approaching-deadline item, check if an obligation already exists for that action (by detected_action text match). If not, create one with source_channel "watcher" and priority based on scan type (approaching=P1, stale=P2, overdue=P1) [owner:api-engineer]
- [ ] [10.2] [P-2] Modify packages/daemon/src/index.ts -- pass obligationStore to ProactiveWatcher constructor [owner:api-engineer]

## Barrel Exports

- [ ] [11.1] [P-1] Modify packages/daemon/src/features/obligations/index.ts -- ensure new exports are included: Escalated status, escalation callback handlers and prefixes, any new types [owner:api-engineer]

## Verify

- [ ] [12.1] tsc --noEmit passes for packages/db (schema change) [owner:api-engineer]
- [ ] [12.2] tsc --noEmit passes for packages/daemon (all obligation changes) [owner:api-engineer]
- [ ] [12.3] Existing obligation tests still pass (obligations.test.ts) [owner:api-engineer]
- [ ] [12.4] [user] Manual test: send a message to Nova via Telegram that contains a commitment (e.g., "remind me to check the deploy tomorrow"). Verify an obligation is created and appears in /ob output [owner:api-engineer]
- [ ] [12.5] [user] Manual test: verify executor starts, logs "ObligationExecutor started" on daemon boot [owner:api-engineer]
- [ ] [12.6] [user] Manual test: send /obligation "Review Jira backlog" p1 in Telegram, verify obligation created with P1 priority [owner:api-engineer]
- [ ] [12.7] [user] Manual test: verify no autonomous execution during quiet hours (22:00-07:00) [owner:api-engineer]
