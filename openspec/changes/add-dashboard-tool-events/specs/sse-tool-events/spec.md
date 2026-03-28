# SSE Tool Events

## MODIFIED Requirements

### Requirement: tool_start events are forwarded through the SSE stream

The `for await` loop in the `POST /chat` handler in `packages/daemon/src/http.ts` MUST add an `else if` branch for `event.type === "tool_start"` that calls `humanizeToolName(event.name)` (imported from `channels/tool-names.ts`) and emits a JSON SSE message with the shape `{ type: "tool_start", name: "<human label>", callId: "<id>" }`. The suppression comment `// tool_start and tool_done are ignored in SSE output for now` MUST be deleted.

#### Scenario: tool_start event is relayed to SSE client

Given a chat request that triggers a tool call,
when `processMessageStream` yields a `tool_start` event with `name: "get_calendar_events"`,
then the SSE stream emits `{ "type": "tool_start", "name": "Checking Calendar...", "callId": "<id>" }` before the next chunk event arrives.

### Requirement: tool_done events are forwarded through the SSE stream with duration

The `for await` loop MUST add an `else if` branch for `event.type === "tool_done"` that calls `humanizeToolName(event.name)` and emits a JSON SSE message with the shape `{ type: "tool_done", name: "<human label>", callId: "<id>", durationMs: <number> }`. The `durationMs` value SHALL be taken directly from the `tool_done` event yielded by `processMessageStream`.

#### Scenario: tool_done event includes elapsed duration

Given a tool call that takes 1200ms to complete,
when `processMessageStream` yields a `tool_done` event with `durationMs: 1200`,
then the SSE stream emits `{ "type": "tool_done", "name": "Checking Calendar...", "callId": "<id>", "durationMs": 1200 }`.

#### Scenario: Unknown event types from existing clients are silently ignored

Given an existing dashboard client that does not handle `tool_start` or `tool_done` event types,
when the SSE stream emits these new event types,
then the client ignores them without error and continues receiving `chunk` and `done` events normally.
