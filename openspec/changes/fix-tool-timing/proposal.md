# Proposal: Fix Tool Timing Display

## Change ID
`fix-tool-timing`

## Summary
Replace the always-zero tool timer in Telegram streaming with completed tool durations and a running total elapsed time. Currently `(0s)` shows because `tool_done` fires before the throttled flush renders the active timer.

## Context
- Extends: `packages/daemon/src/channels/stream-writer.ts`
- Related: `add-response-streaming` (completed — introduced TelegramStreamWriter)

## Motivation
The streaming status line shows "Searching files... (0s)" and "Reading files... (0s)" because the Agent SDK yields `tool_start` and `tool_done` in rapid succession — the tool completes before the 300ms draft throttle fires a Telegram update. The elapsed timer computes from `activeTools` which is already cleared by `tool_done`. Users see zero-second timers for every tool, making the status line misleading.

The fix tracks completed tools with their actual durations and adds a running total elapsed time from the first event.

## Requirements

### Req-1: Completed tool durations
Show each tool's actual duration after it completes, instead of a live counter that's always stale.

### Req-2: Running total elapsed time
Show total time since the first streaming event, updated on each flush.

### Req-3: Compact Telegram display
Keep the status line compact for mobile Telegram — one line for tools, one line for total.

## Scope
- **IN**: `stream-writer.ts` `buildDisplayText()`, `onToolDone()`, tool tracking data structures
- **OUT**: Agent SDK event timing changes, `tool-names.ts` changes, HTTP/dashboard streaming

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/channels/stream-writer.ts` | Track completed tools, show real durations + total elapsed |

## Risks
| Risk | Mitigation |
|------|-----------|
| Status line too long with many tools | Cap display to last 3 completed tools |
| Frequent edits hitting Telegram rate limit | Existing throttle (300ms draft / 1000ms edit) unchanged |
