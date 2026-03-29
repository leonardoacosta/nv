# Spec: Structured Agent Error Categories

## ADDED Requirements

### Requirement: AgentErrorCategory enum

The daemon MUST define an `AgentErrorCategory` string enum at `packages/daemon/src/brain/agent-error.ts` with exactly six members: `AUTH_FAILURE`, `RATE_LIMITED`, `MODEL_UNAVAILABLE`, `TIMEOUT`, `BINARY_NOT_FOUND`, and `UNKNOWN`. No other category values SHALL exist.

#### Scenario: Enum members are string-typed

Given the `AgentErrorCategory` enum,
when a category value is logged as a structured field,
then the logged value is a human-readable string (e.g., `"AUTH_FAILURE"`) not a numeric index.

### Requirement: AgentError class

The daemon MUST provide an `AgentError` class extending `Error` that carries a `readonly category: AgentErrorCategory` field. Constructing an `AgentError` SHALL require both a `category` and a `message`. An optional `cause` SHALL be forwarded to the base `Error` constructor. The `name` property SHALL be `"AgentError"`.

#### Scenario: AgentError preserves category on throw and catch

Given a throw site that executes `throw new AgentError(AgentErrorCategory.RATE_LIMITED, "rate limit hit")`,
when the error is caught,
then `err instanceof AgentError` is `true`, `err.category === "RATE_LIMITED"` is `true`, and `err.message === "rate limit hit"` is `true`.

#### Scenario: AgentError cause is accessible

Given a throw site that wraps a raw SDK error `cause`,
when the `AgentError` is caught,
then `err.cause === cause` is `true`.

### Requirement: classifyAgentError function

The daemon MUST export a `classifyAgentError(err: unknown): AgentErrorCategory` function that inspects the error and returns the most specific matching category. Classification SHALL be deterministic and pure. If the input is already an `AgentError`, its existing category SHALL be returned unchanged. If no pattern matches, the function SHALL return `UNKNOWN`.

#### Scenario: Existing AgentError passes through unchanged

Given an `AgentError` with category `TIMEOUT`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.TIMEOUT` without re-inspection.

#### Scenario: 401 message string maps to AUTH_FAILURE

Given an `Error` whose message contains `"401"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.AUTH_FAILURE`.

#### Scenario: authentication_error string maps to AUTH_FAILURE

Given an `Error` whose message contains `"authentication_error"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.AUTH_FAILURE`.

#### Scenario: 429 message string maps to RATE_LIMITED

Given an `Error` whose message contains `"429"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.RATE_LIMITED`.

#### Scenario: rate_limit message string maps to RATE_LIMITED

Given an `Error` whose message contains `"rate_limit"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.RATE_LIMITED`.

#### Scenario: 529 message string maps to MODEL_UNAVAILABLE

Given an `Error` whose message contains `"529"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.MODEL_UNAVAILABLE`.

#### Scenario: overloaded message string maps to MODEL_UNAVAILABLE

Given an `Error` whose message contains `"overloaded"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.MODEL_UNAVAILABLE`.

#### Scenario: ETIMEDOUT maps to TIMEOUT

Given an `Error` whose message contains `"ETIMEDOUT"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.TIMEOUT`.

#### Scenario: ENOENT on claude path maps to BINARY_NOT_FOUND

Given an `Error` whose message contains `"ENOENT"` and whose stack or message includes a path containing `"claude"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.BINARY_NOT_FOUND`.

#### Scenario: Unrecognized error maps to UNKNOWN

Given an `Error` whose message is `"unexpected internal error"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.UNKNOWN`.

#### Scenario: Non-Error values map to UNKNOWN

Given a thrown value that is a plain string `"oops"`,
when passed to `classifyAgentError`,
then the return value is `AgentErrorCategory.UNKNOWN` without throwing.

### Requirement: USER_MESSAGES map

The daemon MUST export a `USER_MESSAGES` constant typed as `Record<AgentErrorCategory, string>` that provides a distinct user-facing message for every category. Every category key SHALL be present; no category SHALL map to an empty string.

#### Scenario: All categories have a non-empty message

Given the `USER_MESSAGES` map,
when iterated over all `AgentErrorCategory` values,
then every value is a non-empty string.

#### Scenario: RATE_LIMITED message mentions retry timing

Given `USER_MESSAGES[AgentErrorCategory.RATE_LIMITED]`,
when read,
then the string contains a reference to waiting or retrying with a time hint (e.g., "30 seconds").

#### Scenario: BINARY_NOT_FOUND message indicates installation issue

Given `USER_MESSAGES[AgentErrorCategory.BINARY_NOT_FOUND]`,
when read,
then the string communicates that the Claude binary is missing and cannot be resolved by retrying.

### Requirement: Typed throws in agent.ts

The `NovaAgent` class MUST throw `AgentError` instead of plain `Error` at all four throw sites: the two missing-gateway-key guards (in `processMessage` and `processMessageStream`) and the two non-success result subtype sites (in `processMessage` and `processMessageStream`). No plain `new Error(...)` SHALL remain at these locations.

#### Scenario: Missing gateway key throws AUTH_FAILURE

Given a `NovaAgent` configured without a `VERCEL_GATEWAY_KEY`,
when `processMessage` or `processMessageStream` is called,
then the thrown error is an `AgentError` with category `AUTH_FAILURE`.

#### Scenario: Non-success SDK result subtype throws classified error

Given the Agent SDK returning a result message with `subtype: "error_during_execution"`,
when `processMessage` processes that result,
then the thrown error is an `AgentError` whose category is determined by `classifyAgentError` applied to the subtype string.

### Requirement: Category-aware queue failed handler

The `queue.on("failed", ...)` handler in `packages/daemon/src/index.ts` MUST classify the job error with `classifyAgentError`, include `errorCategory` in the structured log fields, and send `USER_MESSAGES[category]` to Telegram. The hardcoded string `"Sorry, something went wrong processing your message. Please try again."` SHALL be removed from this site.

#### Scenario: Rate-limited job failure sends rate-limit message

Given a job that fails because the Vercel AI Gateway returns a rate-limit error,
when the queue emits `"failed"`,
then the Telegram message sent to the user contains the `RATE_LIMITED` user message and `log.error` is called with `errorCategory: "RATE_LIMITED"`.

#### Scenario: Unknown job failure sends generic message

Given a job that fails with an unrecognized error,
when the queue emits `"failed"`,
then the Telegram message sent to the user contains the `UNKNOWN` user message.

### Requirement: Category-aware streaming catch block

The `catch (err: unknown)` block inside the streaming job handler in `packages/daemon/src/index.ts` MUST classify `err` with `classifyAgentError`, include `errorCategory` in the structured log fields, and call `writer.abort(USER_MESSAGES[category])`. The hardcoded string `"Sorry, something went wrong."` SHALL be removed from this site.

#### Scenario: Auth failure during stream surfaces auth message

Given a streaming job that throws an `AgentError` with category `AUTH_FAILURE`,
when the inner catch block fires,
then `writer.abort` is called with the `AUTH_FAILURE` user message and the log entry includes `errorCategory: "AUTH_FAILURE"`.

#### Scenario: Timeout during stream surfaces timeout message

Given a streaming job that throws with `ETIMEDOUT` in the message,
when the inner catch block fires,
then `writer.abort` is called with the `TIMEOUT` user message.
