# Proposal: Extract Agent Query Factory

## Change ID
`extract-agent-query-factory`

## Summary
Extract the duplicated `query()` call pattern from 6 files into a single `createAgentQuery()` factory in `packages/daemon/src/brain/query-factory.ts`. All callers receive a consistently configured stream with gateway routing, permission bypass, and error normalization from one place.

## Context
- Depends on: none
- Conflicts with: none
- Roadmap: standalone refactor, no wave dependency
- Files with duplication: `packages/daemon/src/brain/agent.ts` (2 call sites), `packages/daemon/src/features/obligations/executor.ts`, `packages/daemon/src/features/digest/scheduler.ts`, `packages/daemon/src/features/dream/orchestrator.ts`, `packages/daemon/src/features/briefing/synthesizer.ts`

## Motivation
The Agent SDK `query()` call is copy-pasted across 6 files. Each copy independently wires:

1. `ANTHROPIC_BASE_URL` to `https://ai-gateway.vercel.sh`
2. `ANTHROPIC_CUSTOM_HEADERS` with the Vercel gateway bearer token
3. `permissionMode: "bypassPermissions"` and `allowDangerouslySkipPermissions: true`
4. A timeout implemented four different ways (`Promise.race` with inline `setTimeout`, a `createTimeout` helper, a `withTimeout` helper, a bare `setTimeout` resolve)
5. A result extraction loop that iterates `SDKMessage` looking for `type === "result"` and `subtype === "success"`

The consequence is that any change to gateway routing, auth headers, error handling, or the result-extraction protocol requires coordinated edits across 6 files. The digest scheduler's Tier 2 runner, dream orchestrator's `compressTopic`, briefing synthesizer, and obligation executor each implement subtly different timeout and error-surfacing patterns, making the behavior inconsistent. A single factory eliminates drift and reduces the surface area for auth regressions.

## Requirements

### Req-1: Create `createAgentQuery()` factory
Add `packages/daemon/src/brain/query-factory.ts` exporting `createAgentQuery(options)`. The factory MUST accept `{ model?, maxTurns, timeoutMs, mcpServers?, allowedTools?, systemPrompt? }`, configure gateway env vars from `VERCEL_GATEWAY_KEY` (or a passed `gatewayKey` parameter), set `permissionMode: "bypassPermissions"` and `allowDangerouslySkipPermissions: true`, apply a `timeoutMs` deadline via `Promise.race`, and return `{ text, inputTokens, outputTokens }` on success. It MUST throw a normalized `Error` on `subtype !== "success"` or on timeout.

### Req-2: Refactor `NovaAgent` in `brain/agent.ts` to use the factory
Both `processMessage()` and `processMessageStream()` MUST replace their inline `query()` call with the factory. The streaming variant still iterates the raw SDK stream for `tool_start`/`tool_done` events — the factory MUST expose the underlying `AsyncIterable<SDKMessage>` via an optional streaming mode (e.g., `createAgentQueryStream()`) so `processMessageStream` can use it without losing event granularity.

### Req-3: Refactor `ObligationExecutor` in `features/obligations/executor.ts`
The private `runAgentQuery()` function MUST be deleted and its call replaced with `createAgentQuery()` from the factory. The `createTimeout` helper MUST be deleted. Gateway key passing MUST be handled inside the factory.

### Req-4: Refactor `runTier2Digest` in `features/digest/scheduler.ts`
The inline `query()` block with its manual `Promise.race` + inline `setTimeout` rejection MUST be replaced with `createAgentQuery()`. The `TIER2_TIMEOUT_MS` constant SHALL be passed as `timeoutMs` to the factory.

### Req-5: Refactor `compressTopic` in `features/dream/orchestrator.ts`
The inline `query()` + `Promise.race([streamPromise, timeoutPromise])` pattern MUST be replaced with `createAgentQuery()`. The 60-second timeout constant SHALL be passed as `timeoutMs`.

### Req-6: Refactor `synthesizeBriefing` in `features/briefing/synthesizer.ts`
The inline `query()` block inside the `withTimeout()` wrapper MUST be replaced with `createAgentQuery()`. The `SYNTHESIS_TIMEOUT_MS` constant SHALL be passed as `timeoutMs` to the factory.

### Req-7: TypeScript must pass after refactor
Running `pnpm tsc --noEmit` in `packages/daemon/` MUST produce zero errors after all six refactors are complete.

## Scope
- **IN**: Create `query-factory.ts`, refactor 6 call sites in `packages/daemon/src/`, delete dead timeout helpers, delete `runAgentQuery` in executor
- **OUT**: Changes to MCP server configuration logic (`mcp-config.ts`), changes to diary write-back, changes to obligation store or Telegram notification logic, changes to any file outside `packages/daemon/src/`

## Impact
| File | Change |
|------|--------|
| `packages/daemon/src/brain/query-factory.ts` | NEW — exports `createAgentQuery()` and `createAgentQueryStream()` |
| `packages/daemon/src/brain/agent.ts` | Replace 2 `query()` call sites; `processMessageStream` uses `createAgentQueryStream()` |
| `packages/daemon/src/features/obligations/executor.ts` | Delete `runAgentQuery()` + `createTimeout()`, replace call with factory |
| `packages/daemon/src/features/digest/scheduler.ts` | Replace inline `query()` block and `Promise.race` in `runTier2Digest` |
| `packages/daemon/src/features/dream/orchestrator.ts` | Replace inline `query()` block and `Promise.race` in `compressTopic` |
| `packages/daemon/src/features/briefing/synthesizer.ts` | Replace inline `query()` block in `synthesizeBriefing` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Streaming event granularity lost for `processMessageStream` | Expose a separate `createAgentQueryStream()` that returns the raw `AsyncIterable<SDKMessage>` so the stream variant retains per-event handling |
| Different callers expect different error shapes | Factory throws a normalized `Error` with the SDK subtype in the message; callers that caught `Error` already handle this shape |
| `executor.ts` passes gateway key explicitly; factory must not break that | Factory accepts optional `gatewayKey` parameter that overrides `process.env.VERCEL_GATEWAY_KEY` — executor passes its stored key; other callers omit it and fall back to env |
| Timeout semantics differ (reject vs resolve-null) | Factory always rejects on timeout; dream and digest callers currently resolve to `null` on timeout and fall back to static output — update callers to `catch` the timeout error and apply the same fallback |
