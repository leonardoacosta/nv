# Proposal: Add Dashboard Tool Events via SSE

## Change ID
`add-dashboard-tool-events`

## Summary

Relay `tool_start` and `tool_done` events through the `POST /chat` SSE stream so the dashboard has real-time visibility into which tools Nova is using during processing. The agent already yields these events — they are currently silently discarded at `http.ts:172`. This change removes that suppression and forwards the events as SSE messages using the same JSON envelope pattern as the existing `chunk` and `done` events.

## Context
- Extends: `packages/daemon/src/http.ts` (the `POST /chat` SSE handler)
- Related: `packages/daemon/src/brain/agent.ts` (`processMessageStream` already yields `tool_start` / `tool_done`), `packages/daemon/src/brain/types.ts` (canonical `StreamEvent` union), `packages/daemon/src/channels/stream-writer.ts` (Telegram side already consumes these events), `packages/daemon/src/channels/tool-names.ts` (`humanizeToolName` — reuse for human-readable labels)

## Motivation

The Telegram channel already displays live tool status via `TelegramStreamWriter.onToolStart` / `onToolDone`, which calls `humanizeToolName` to produce labels like "Checking Calendar..." and "Running command...". The dashboard gets nothing — the comment at `http.ts:172` reads:

```
// tool_start and tool_done are ignored in SSE output for now
```

This means a dashboard user sees a spinner with no indication of what Nova is doing during long multi-tool responses. The fix is a single `else if` branch in the existing event loop — the infrastructure (events, humanization, SSE stream) is already in place.

## Requirements

### Req-1: Relay tool_start via SSE

In the `for await` loop in `packages/daemon/src/http.ts`, add an `else if` branch for `event.type === "tool_start"` that calls `humanizeToolName(event.name)` and emits:

```json
{ "type": "tool_start", "name": "Checking Calendar...", "callId": "xxx" }
```

`humanizeToolName` is already exported from `packages/daemon/src/channels/tool-names.ts` — import it.

### Req-2: Relay tool_done via SSE

Add an `else if` branch for `event.type === "tool_done"` that calls `humanizeToolName(event.name)` and emits:

```json
{ "type": "tool_done", "name": "Checking Calendar...", "callId": "xxx", "durationMs": 1200 }
```

The `durationMs` value is already present on the `tool_done` event yielded by `processMessageStream`.

### Req-3: Remove the suppression comment

Delete the `// tool_start and tool_done are ignored in SSE output for now` comment at `http.ts:172` — it is no longer accurate once Req-1 and Req-2 are implemented.

## Scope
- **IN**: `packages/daemon/src/http.ts` — add two `else if` branches and remove the comment
- **OUT**: Dashboard UI changes (consuming the new events is a separate UI concern), changes to `brain/types.ts` or `brain/agent.ts`, changes to the Telegram stream writer, changes to `tool-names.ts`

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/http.ts` | Modified: add `tool_start` + `tool_done` SSE relay, import `humanizeToolName`, remove suppression comment |

## Risks
| Risk | Mitigation |
|------|-----------|
| Dashboard client does not yet handle `tool_start`/`tool_done` event types | Unknown event types are silently ignored by the existing dashboard client — no breakage, UI can be wired up separately |
| High-frequency tool events cause SSE backpressure | `tool_start`/`tool_done` are low-frequency (one pair per tool call, not per token) — no throttling needed |
