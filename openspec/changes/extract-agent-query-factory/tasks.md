# Implementation Tasks
<!-- beads:epic:nv-spls -->

## DB Batch

(No DB tasks)

## API Batch

- [ ] [2.1] [P-1] Create `packages/daemon/src/brain/query-factory.ts` — export `createAgentQuery({ prompt, systemPrompt?, model?, maxTurns, timeoutMs, mcpServers?, allowedTools?, gatewayKey? })` returning `Promise<{ text: string; inputTokens: number; outputTokens: number }>` and `createAgentQueryStream({ ... })` returning `AsyncIterable<SDKMessage>`. Inject `ANTHROPIC_BASE_URL` + `ANTHROPIC_CUSTOM_HEADERS` gateway env vars, set `permissionMode: "bypassPermissions"` + `allowDangerouslySkipPermissions: true`, apply `Promise.race` with `setTimeout` reject for `timeoutMs`, accumulate token counts from all assistant messages, throw normalized `Error` on non-success subtype or missing gateway key [owner:api-engineer]
- [ ] [2.2] [P-1] Refactor `packages/daemon/src/brain/agent.ts` — replace `processMessage()` inline `query()` call with `createAgentQuery()` from factory; replace `processMessageStream()` inline `query()` call with `createAgentQueryStream()` and iterate the raw `AsyncIterable<SDKMessage>` for `tool_start`/`tool_done`/`text_delta` events as before; remove all inline gateway env var construction from both methods [owner:api-engineer]
- [ ] [2.3] [P-1] Refactor `packages/daemon/src/features/obligations/executor.ts` — delete module-level `runAgentQuery()` function and `createTimeout()` helper; replace `runAgentQuery(...)` call in `tryExecuteNext()` with `createAgentQuery({ prompt, gatewayKey: this.gatewayKey, timeoutMs: this.config.timeoutMs, mcpServers: this.mcpServers, allowedTools: this.allowedTools, model })` from factory; use returned `inputTokens`/`outputTokens` for cost tracking [owner:api-engineer]
- [ ] [2.4] [P-1] Refactor `packages/daemon/src/features/digest/scheduler.ts` — replace the inline `query()` block and `Promise.race` in `runTier2Digest()` with `await createAgentQuery({ prompt, systemPrompt: TIER2_SYSTEM_PROMPT, maxTurns: 1, timeoutMs: TIER2_TIMEOUT_MS, allowedTools: [] })`; catch factory errors and fall back to `buildStaticWeeklySummary(stats)` with the existing log warning [owner:api-engineer]
- [ ] [2.5] [P-1] Refactor `packages/daemon/src/features/dream/orchestrator.ts` — replace the inline `query()` stream, `streamPromise`, and `timeoutPromise` in `compressTopic()` with `await createAgentQuery({ prompt: content, systemPrompt, maxTurns: 1, timeoutMs: 60_000, allowedTools: [] })`; catch errors and return `null` with the existing topic warning log [owner:api-engineer]
- [ ] [2.6] [P-1] Refactor `packages/daemon/src/features/briefing/synthesizer.ts` — replace the inline `query()` block and its surrounding `withTimeout()` async IIFE wrapper in `synthesizeBriefing()` with `await createAgentQuery({ prompt: contextPrompt, systemPrompt: BRIEFING_SYSTEM_PROMPT, maxTurns: 1, timeoutMs: SYNTHESIS_TIMEOUT_MS, mcpServers, allowedTools })`; catch factory errors and fall back to `buildStaticSummary(context)` [owner:api-engineer]
- [ ] [2.7] [P-1] Run `pnpm tsc --noEmit` in `packages/daemon/` and confirm zero type errors across all six modified files [owner:api-engineer]

## UI Batch

(No UI tasks)

## E2E Batch

- [ ] [4.1] Verify the refactor is structurally correct — grep that `@anthropic-ai/claude-agent-sdk` `query` is imported only in `query-factory.ts` and nowhere else under `packages/daemon/src/`; confirm `createTimeout` and `runAgentQuery` identifiers no longer exist in `executor.ts`; confirm `ANTHROPIC_BASE_URL` string literal does not appear outside `query-factory.ts` [owner:e2e-engineer]
