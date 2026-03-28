# Proposal: Add Structured Agent Error Categories

## Change ID
`add-structured-agent-errors`

## Summary
Replace the single generic error path in the Agent SDK call stack with a typed `AgentError` class carrying an `AgentErrorCategory` enum. Classification logic inspects the raw error signal at the throw site and maps it to one of six categories. The `failed` queue event handler and the inner `catch` block in the streaming handler both read the category and surface a user-friendly Telegram message instead of the current opaque fallback.

## Context
- Depends on: none (modifies daemon internals only)
- Conflicts with: none
- Roadmap: Wave 5 (daemon reliability)
- Error throw sites: `packages/daemon/src/brain/agent.ts` lines 97-101 (missing gateway key), 152-155 (non-success result subtype), 197-201 (missing gateway key in streaming path), 285-288 (non-success result subtype in streaming path)
- Error surface sites: `packages/daemon/src/index.ts` line 277 (queue `failed` event handler, sends generic message), line 710 (inner `catch` in streaming handler, calls `writer.abort` with generic message)

## Motivation
When the Agent SDK fails today, every failure path collapses to one of two strings:

- `"Sorry, something went wrong processing your message. Please try again."` — emitted by the queue `failed` event handler (`index.ts:277`)
- `"Sorry, something went wrong."` — emitted by the inner `catch` block that aborts the stream writer (`index.ts:710`)

Neither string gives the user actionable information. The underlying cause — a 401 from the Vercel AI Gateway, a 429 rate-limit response, a 529 model overload, a process timeout, or a missing `claude` binary — is swallowed into a pino `log.error` call and never reaches the user.

Six distinct failure modes are observable at the throw site and each has a different remediation:

| Failure | Signal | User action |
|---------|--------|-------------|
| Auth failure | 401 / `authentication_error` in message or status | Check gateway key configuration |
| Rate limited | 429 / `rate_limited` | Wait and retry |
| Model unavailable | 529 / `overloaded_error` | Try again shortly |
| Timeout | `ETIMEDOUT`, `ECONNABORTED`, agent process exit after deadline | Try again |
| Binary not found | `ENOENT` on `claude` executable path | Installation problem |
| Unknown | Any other error | Generic fallback |

Without category tagging the daemon cannot choose the right message. With it, users receive specific guidance and the structured log gains a `errorCategory` field that future alerting and metrics can use.

## Requirements

### Req-1: AgentErrorCategory enum
Create `packages/daemon/src/brain/agent-error.ts` defining `AgentErrorCategory` as a string enum with members `AUTH_FAILURE`, `RATE_LIMITED`, `MODEL_UNAVAILABLE`, `TIMEOUT`, `BINARY_NOT_FOUND`, and `UNKNOWN`.

### Req-2: AgentError class
In the same file, create an `AgentError` class extending `Error`. Constructor accepts `category: AgentErrorCategory`, `message: string`, and an optional `cause: unknown`. The instance MUST expose `readonly category: AgentErrorCategory` and set `this.name = "AgentError"`. Pass `cause` to the `Error` super options when present.

### Req-3: classifyAgentError function
In the same file, export a `classifyAgentError(err: unknown): AgentErrorCategory` function. Classification rules in priority order:

1. If `err` is an `AgentError`, return its existing `category`.
2. Inspect `err.message` (case-insensitive) and HTTP status codes:
   - `"401"`, `"authentication_error"`, `"unauthorized"`, `"invalid api key"`, `"invalid_api_key"` → `AUTH_FAILURE`
   - `"429"`, `"rate_limit"`, `"rate limited"`, `"too many requests"` → `RATE_LIMITED`
   - `"529"`, `"overloaded"`, `"model_unavailable"` → `MODEL_UNAVAILABLE`
   - `"ETIMEDOUT"`, `"ECONNABORTED"`, `"timed out"`, `"timeout"` → `TIMEOUT`
   - `"ENOENT"` combined with a path containing `"claude"` → `BINARY_NOT_FOUND`
3. Fall through to `UNKNOWN`.

### Req-4: USER_MESSAGES map
In the same file, export a `USER_MESSAGES: Record<AgentErrorCategory, string>` constant with these exact values:

- `AUTH_FAILURE`: `"Nova cannot reach the AI gateway — authentication failed. Check the gateway key configuration."`
- `RATE_LIMITED`: `"Nova is rate-limited. Please try again in 30 seconds."`
- `MODEL_UNAVAILABLE`: `"The model is temporarily overloaded. Please try again in a moment."`
- `TIMEOUT`: `"Nova timed out waiting for a response. Please try again."`
- `BINARY_NOT_FOUND`: `"The Claude binary is missing. Nova cannot process messages until it is reinstalled."`
- `UNKNOWN`: `"Sorry, something went wrong processing your message. Please try again."`

### Req-5: Throw AgentError at classification sites in agent.ts
Modify `packages/daemon/src/brain/agent.ts`:

- At lines 97-101 (missing gateway key before `processMessage`): throw `new AgentError(AgentErrorCategory.AUTH_FAILURE, "Vercel AI Gateway key is required but not configured.", undefined)` instead of a plain `Error`.
- At lines 152-155 (non-success result subtype in `processMessage`): classify the subtype string with `classifyAgentError` and throw the resulting `AgentError`.
- At lines 197-201 (missing gateway key before `processMessageStream`): same as the first site above.
- At lines 285-288 (non-success result subtype in `processMessageStream`): same as the second site above.

### Req-6: Surface category in index.ts failed event handler
Modify the `queue.on("failed", ...)` handler in `packages/daemon/src/index.ts` (around line 269):

- Extract the error from `event.job.error`. Classify it with `classifyAgentError`.
- Add `errorCategory` to the `log.error` structured fields.
- Send `USER_MESSAGES[category]` to Telegram instead of the hardcoded string.

### Req-7: Surface category in index.ts inner catch block
Modify the `catch (err: unknown)` block inside the streaming handler (`index.ts` around line 702):

- Classify `err` with `classifyAgentError`.
- Add `errorCategory` to the `log.error` structured fields.
- Call `writer.abort(USER_MESSAGES[category])` instead of the hardcoded string.

### Req-8: Barrel export
Export `AgentError`, `AgentErrorCategory`, `classifyAgentError`, and `USER_MESSAGES` from `packages/daemon/src/brain/agent-error.ts`. No changes to `packages/daemon/src/brain/agent.ts` exports are needed beyond the import.

## Scope
- **IN**: `AgentError` class, `AgentErrorCategory` enum, `classifyAgentError` function, `USER_MESSAGES` map, updated throw sites in `agent.ts`, updated error surfaces in `index.ts` (queue failed handler and inner streaming catch)
- **OUT**: HTTP error response parsing from raw Fetch responses (gateway responses that embed status in a JSON body require a separate integration), persistent error metrics, Telegram inline retry buttons, alerting rules on error categories, retry-with-backoff logic

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/brain/agent-error.ts` | NEW — `AgentErrorCategory`, `AgentError`, `classifyAgentError`, `USER_MESSAGES` |
| `packages/daemon/src/brain/agent.ts` | MODIFY — import `AgentError`/`AgentErrorCategory`; replace four `throw new Error(...)` sites with typed `AgentError` throws |
| `packages/daemon/src/index.ts` | MODIFY — import `classifyAgentError` and `USER_MESSAGES`; update queue `failed` handler and inner streaming `catch` block to classify and use category message |

## Risks
| Risk | Mitigation |
|------|-----------|
| Classification false-positives (e.g., a message coincidentally containing "401") | All pattern matches require the substring to appear in the error message from the SDK, not in user content. The classification runs on thrown `Error` objects only, not on user input. |
| Gateway error format changes (new SDK version wraps status differently) | `classifyAgentError` falls through to `UNKNOWN` when no pattern matches, preserving the existing generic message. The function is the single change point when patterns need updating. |
| `event.job.error` is typed as `string` in the queue types, not `Error` | `classifyAgentError` accepts `unknown` and handles string inputs — string matching applies directly. No type change to the queue is needed. |
| Missing gateway key throws before the SDK is invoked | Auth failure category is applied at that site explicitly (Req-5), not by pattern matching, so it is never misclassified. |
