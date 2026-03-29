# Spec: Live Tool Ticker

## MODIFIED Requirements

### Requirement: Single-tool display format
The streaming status line MUST show only the current active tool name, its elapsed seconds, and the total elapsed seconds since the first streaming event.

Format: `{toolName} ({toolElapsedSeconds}s) — {totalElapsedSeconds}s total`

When no tools are active but text is streaming, the tool status line MUST be omitted.

#### Scenario: Single tool active
Given a `tool_start` event for "Glob" at T=6s,
When the display updates at T=8s,
Then the status line reads: `Searching files... (2s) — 8s total`

#### Scenario: Tool completes, new tool starts
Given "Glob" completed at T=12s and "Read" started at T=12s,
When the display updates at T=15s,
Then the status line reads: `Reading files... (3s) — 15s total`

#### Scenario: Parallel tools
Given "Glob" started at T=6s and "Grep" started at T=7s,
When the display updates at T=10s,
Then the status line shows the most recently started tool: `Searching files... (3s) — 10s total`

### Requirement: One-second tick interval
A recurring 1-second interval MUST start on the first `tool_start` or `text_delta` event. Each tick calls the existing `scheduleFlush()` to update elapsed times. The interval MUST be cleared when:
- `finalize()` is called
- `abort()` is called
- All active tools complete AND no text is streaming (optional optimization)

#### Scenario: Tick updates display
Given "Bash" started at T=0s,
When 3 ticks fire (T=1s, T=2s, T=3s),
Then 3 flushes are scheduled, each showing incremented elapsed times.

#### Scenario: Cleanup on finalize
Given a tick interval is running,
When `finalize()` is called,
Then the interval is cleared and no further ticks fire.

### Requirement: Remove completed-tools chain
The `completedTools` array and its bookkeeping (push, shift, cap to 3) MUST be removed. The display no longer shows historical tool durations — only the current active tool matters.

#### Scenario: No tool history in display
Given "Glob" completed after 6s and "Read" is active at 2s,
When the display updates,
Then the status line shows only `Reading files... (2s) — 8s total` with no mention of Glob.
