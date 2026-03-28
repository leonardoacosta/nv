# Spec: Agent Query Factory

## ADDED Requirements

### Requirement: query-factory module

The daemon MUST provide `packages/daemon/src/brain/query-factory.ts` exporting two functions: `createAgentQuery()` for callers that need only the final text result, and `createAgentQueryStream()` for callers that need per-event streaming. Both SHALL configure the Vercel AI Gateway via `ANTHROPIC_BASE_URL` and `ANTHROPIC_CUSTOM_HEADERS`, set `permissionMode: "bypassPermissions"` and `allowDangerouslySkipPermissions: true`, and accept `{ prompt, systemPrompt?, model?, maxTurns, timeoutMs, mcpServers?, allowedTools?, gatewayKey? }` as input.

#### Scenario: createAgentQuery returns text on success

Given a valid gateway key in `VERCEL_GATEWAY_KEY` and a prompt of "summarize this",
when `createAgentQuery({ prompt: "summarize this", maxTurns: 1, timeoutMs: 30_000 })` is called,
then it resolves with `{ text: string, inputTokens: number, outputTokens: number }` where `text` is non-empty.

#### Scenario: createAgentQuery throws on SDK failure subtype

Given the SDK emits a result message with `subtype: "error_during_execution"`,
when `createAgentQuery()` consumes that message,
then it throws `Error("Agent query failed: error_during_execution")`.

#### Scenario: createAgentQuery throws on timeout

Given `timeoutMs` is set to `5_000` and the SDK does not emit a result within 5 seconds,
when `createAgentQuery()` is awaited,
then it throws an `Error` containing "timed out" in the message.

#### Scenario: createAgentQuery accepts explicit gatewayKey parameter

Given no `VERCEL_GATEWAY_KEY` environment variable is set,
when `createAgentQuery({ ..., gatewayKey: "explicit-key" })` is called,
then the `ANTHROPIC_CUSTOM_HEADERS` env sent to the SDK uses `"explicit-key"` and no error is thrown for a missing env var.

#### Scenario: createAgentQueryStream yields raw SDKMessages

Given a prompt that causes the SDK to emit `assistant` messages with tool_use blocks,
when `createAgentQueryStream({ prompt, maxTurns: 10, timeoutMs: 60_000 })` is called,
then it returns an `AsyncIterable<SDKMessage>` that yields the raw messages in order, including tool_use blocks before the final result.

### Requirement: gateway env vars always injected

The factory MUST always inject `ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh"` and `ANTHROPIC_CUSTOM_HEADERS: "x-ai-gateway-api-key: Bearer <key>"` into the SDK `env` option, regardless of caller. No caller SHALL set these env vars independently after this refactor.

#### Scenario: Gateway headers present in every call

Given any call to `createAgentQuery()` or `createAgentQueryStream()`,
when the underlying `query()` is invoked,
then its `options.env` object contains exactly `ANTHROPIC_BASE_URL` pointing to the Vercel gateway and `ANTHROPIC_CUSTOM_HEADERS` with the bearer token.

#### Scenario: Missing gateway key throws before calling SDK

Given neither `VERCEL_GATEWAY_KEY` env var nor the `gatewayKey` parameter is provided,
when `createAgentQuery()` is called,
then it throws `Error` containing "gateway key" before invoking the Agent SDK, not after.

### Requirement: token accumulation in createAgentQuery

`createAgentQuery()` MUST accumulate `input_tokens` and `output_tokens` from all `assistant` message usages in the stream and return them in the resolved value. Callers that previously extracted tokens from the stream (e.g., `ObligationExecutor`) SHALL use the returned `inputTokens` and `outputTokens` values.

#### Scenario: Token counts summed across multiple turns

Given an Agent SDK query that produces 3 assistant messages with usages `{ input_tokens: 100, output_tokens: 50 }`, `{ input_tokens: 200, output_tokens: 80 }`, and `{ input_tokens: 50, output_tokens: 20 }`,
when `createAgentQuery()` resolves,
then `inputTokens` equals `350` and `outputTokens` equals `150`.

#### Scenario: Zero token counts when usage is absent

Given an SDK response where no `assistant` message includes a `usage` field,
when `createAgentQuery()` resolves,
then `inputTokens` is `0` and `outputTokens` is `0`.

## MODIFIED Requirements

### Requirement: NovaAgent uses factory for both call sites

`NovaAgent.processMessage()` and `NovaAgent.processMessageStream()` in `packages/daemon/src/brain/agent.ts` MUST each replace their inline `query()` + env configuration with the factory. `processMessage()` SHALL use `createAgentQuery()`. `processMessageStream()` SHALL use `createAgentQueryStream()` to retain per-event `tool_start` / `tool_done` / `text_delta` emission. Neither method SHALL set `ANTHROPIC_BASE_URL` or `ANTHROPIC_CUSTOM_HEADERS` directly.

#### Scenario: processMessage produces equivalent response after refactor

Given a message with content "What time is it?",
when `NovaAgent.processMessage()` is called after the refactor,
then the returned `AgentResponse` has the same shape (`text`, `toolCalls`, `stopReason`) as before the refactor.

#### Scenario: processMessageStream still emits tool events

Given a message that causes the SDK to use the Bash tool,
when `NovaAgent.processMessageStream()` is called after the refactor,
then it still yields `{ type: "tool_start", name: "Bash", callId: string }` and a subsequent `{ type: "tool_done", name: "Bash", ... }` before the final `done` event.

### Requirement: ObligationExecutor drops runAgentQuery and createTimeout

`packages/daemon/src/features/obligations/executor.ts` MUST delete the module-level `runAgentQuery()` function and the `createTimeout()` helper. The `tryExecuteNext()` method SHALL call `createAgentQuery()` from the factory, passing `this.gatewayKey` as `gatewayKey`, `this.config.timeoutMs` as `timeoutMs`, `model` as the selected model, `this.mcpServers` and `this.allowedTools`. Token extraction SHALL use the returned `inputTokens` and `outputTokens` fields.

#### Scenario: Cost tracking still works after executor refactor

Given an obligation with priority 0 (Sonnet model) that produces `inputTokens: 1000` and `outputTokens: 200` from the factory,
when the obligation executes successfully,
then `estimateCost(1000, 200, "claude-sonnet-4-6")` is called and `this.dailySpendUsd` is incremented correctly.

### Requirement: Digest scheduler Tier 2 uses factory

`runTier2Digest()` in `packages/daemon/src/features/digest/scheduler.ts` MUST replace the inline `query()` block and its `Promise.race` with a single `await createAgentQuery(...)` call, passing `TIER2_TIMEOUT_MS` as `timeoutMs`. On factory error (including timeout), it MUST catch and fall back to `buildStaticWeeklySummary(stats)` with the same log warning as before.

#### Scenario: Tier 2 falls back to static summary on factory timeout

Given the factory throws a timeout error after 30 seconds,
when `runTier2Digest()` catches the error,
then `buildStaticWeeklySummary(stats)` is called and the resulting text is sent via Telegram without re-throwing.

### Requirement: Dream orchestrator compressTopic uses factory

`compressTopic()` in `packages/daemon/src/features/dream/orchestrator.ts` MUST replace its inline `query()` stream + `Promise.race([streamPromise, timeoutPromise])` with a single `await createAgentQuery(...)` call. On error or timeout, it MUST return `null` with the same warning log as before.

#### Scenario: compressTopic returns null on factory error

Given the factory throws any error (timeout or SDK failure),
when `compressTopic()` catches the error,
then it returns `null` and logs a warning containing the topic name.

### Requirement: Briefing synthesizer uses factory

`synthesizeBriefing()` in `packages/daemon/src/features/briefing/synthesizer.ts` MUST replace its inline `query()` block inside the `withTimeout()` wrapper with a single `await createAgentQuery(...)` call, passing `SYNTHESIS_TIMEOUT_MS` as `timeoutMs`. The `withTimeout()` wrapper around the inner async IIFE MUST be removed since the factory handles the timeout internally.

#### Scenario: synthesizeBriefing produces briefing content after refactor

Given a populated `GatheredContext` with obligations and messages,
when `synthesizeBriefing()` is called after the refactor,
then it returns a `SynthesisResult` with non-empty `content` and a `suggestedActions` array.

#### Scenario: synthesizeBriefing falls back to static summary on factory error

Given the factory throws any error,
when `synthesizeBriefing()` catches it,
then `buildStaticSummary(context)` is returned and a warning is logged.
