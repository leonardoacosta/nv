# Implementation Tasks

<!-- beads:epic:pending -->

## DB Batch

(no database changes)

## API Batch

### Group A: Error module

- [x] [2.1] Create `packages/daemon/src/brain/agent-error.ts` -- define `AgentErrorCategory` as a string enum with members `AUTH_FAILURE`, `RATE_LIMITED`, `MODEL_UNAVAILABLE`, `TIMEOUT`, `BINARY_NOT_FOUND`, `UNKNOWN`. Define `AgentError extends Error` with `readonly category: AgentErrorCategory`, an optional `cause` forwarded to the `Error` super options, and `this.name = "AgentError"` set in the constructor. Export `classifyAgentError(err: unknown): AgentErrorCategory` that returns an existing `AgentError`'s category unchanged, matches error message strings (case-insensitive) for `"401"` / `"authentication_error"` / `"unauthorized"` / `"invalid_api_key"` → `AUTH_FAILURE`; `"429"` / `"rate_limit"` / `"too many requests"` → `RATE_LIMITED`; `"529"` / `"overloaded"` / `"model_unavailable"` → `MODEL_UNAVAILABLE`; `"ETIMEDOUT"` / `"ECONNABORTED"` / `"timed out"` / `"timeout"` → `TIMEOUT`; `"ENOENT"` with a path containing `"claude"` → `BINARY_NOT_FOUND`; and falls through to `UNKNOWN`. Export `USER_MESSAGES: Record<AgentErrorCategory, string>` with the exact copy from Req-4 in the proposal. [owner:api-engineer]

### Group B: Throw-site updates

- [x] [2.2] Modify `packages/daemon/src/brain/agent.ts` -- import `AgentError` and `AgentErrorCategory` from `./agent-error.js`. Replace the plain `throw new Error("Vercel AI Gateway key is required...")` at line ~98 (in `processMessage`) with `throw new AgentError(AgentErrorCategory.AUTH_FAILURE, "Vercel AI Gateway key is required but not configured.")`. Replace the plain `throw new Error(\`Agent query failed: ${sdkMsg.subtype}\`)` at line ~153 (in `processMessage`) with `throw new AgentError(classifyAgentError(new Error(sdkMsg.subtype)), \`Agent query failed: ${sdkMsg.subtype}\`)`. Apply the same two replacements at the corresponding sites in `processMessageStream` (~lines 198 and ~286). [owner:api-engineer]

### Group C: Error surface updates

- [x] [2.3] Modify `packages/daemon/src/index.ts` queue `failed` handler -- import `classifyAgentError` and `USER_MESSAGES` from `./brain/agent-error.js`. In `queue.on("failed", (event) => { ... })` (around line 269), extract the error via `event.job.error`, call `classifyAgentError` on it, add `errorCategory: category` to the `log.error` call's structured fields, and replace the hardcoded Telegram message string with `USER_MESSAGES[category]`. [owner:api-engineer]
- [x] [2.4] Modify `packages/daemon/src/index.ts` inner streaming catch -- in the `catch (err: unknown)` block inside the streaming job handler (around line 702), call `classifyAgentError(err)`, add `errorCategory: category` to the `log.error` call's structured fields, and replace `writer.abort("Sorry, something went wrong.")` with `writer.abort(USER_MESSAGES[category])`. [owner:api-engineer]

## UI Batch

(no UI changes)

## E2E Batch

- [x] [4.1] Verify daemon compiles after changes -- run `pnpm typecheck` in `packages/daemon`. Confirm zero type errors introduced by the new module and the four throw-site replacements. Confirm `classifyAgentError` is reachable from both import sites in `index.ts`. [owner:e2e-engineer]
