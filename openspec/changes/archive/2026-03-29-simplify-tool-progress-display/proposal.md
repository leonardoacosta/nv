# Proposal: Simplify Tool Progress Display

## Change ID
`simplify-tool-progress-display`

## Summary
Replace the chained multi-tool status line in Telegram with a single-tool live ticker that updates every second, showing only the current tool's elapsed time and a running total.

## Context
- Extends: `packages/daemon/src/channels/stream-writer.ts`
- Related: `fix-tool-timing` (completed — fixed zero-second timers by tracking completed durations)

## Motivation
The current streaming status line chains all completed and active tools into one growing line:
```
Working... (6s) | Searching files... (6s) | Searching files...(4s) | Running command... (0s) — 20s total
```

This is hard to parse on mobile Telegram — the line grows with every tool and becomes noisy. The user only cares about what's happening *now* and how long it's been. The new format shows a single updating line:
```
Searching files... (4s) — 16s total
```

The display ticks every second for a live feel, and the tool name swaps when a new tool starts.

## Requirements

### Req-1: Single-tool display
Show only the most recently started active tool with its elapsed time and the total elapsed time since the first event. Drop the completed-tools chain and `|` separator entirely.

### Req-2: One-second tick interval
Start a 1-second recurring timer when the first tool starts. Each tick triggers a flush to update the displayed elapsed times. Clear the interval when all tools complete, on finalize, or on abort.

### Req-3: Parallel tool handling
When multiple tools are active simultaneously (parallel Agent SDK tool calls), display only the most recently started tool. The total elapsed time reflects wall-clock time from the first event, not per-tool cumulative time.

## Scope
- **IN**: `stream-writer.ts` — `buildDisplayText()`, tick interval lifecycle, `completedTools` removal
- **OUT**: `tool-names.ts`, HTTP/dashboard streaming, agent SDK event timing, Telegram adapter

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/channels/stream-writer.ts` | Replace multi-tool chain with single-tool ticker, add 1s interval |

## Risks
| Risk | Mitigation |
|------|-----------|
| 1s interval causes excess Telegram API calls | Existing throttle (300ms draft / 1000ms edit) gates actual sends |
| Interval not cleaned up on error paths | Clear in both `finalize()` and `abort()`, plus guard in flush |
