# Implementation Tasks

<!-- beads:epic:nv-hmjk -->

## API Batch

- [x] [1.1] [P-1] Refactor `buildDisplayText()` in `stream-writer.ts` — show only the most recently started active tool with format `{name} ({elapsed}s) — {total}s total`, remove `completedTools` array and `|`-joined chain [owner:api-engineer] [beads:nv-zi4t]
- [x] [1.2] [P-1] Add 1-second tick interval to `stream-writer.ts` — start `setInterval(1000)` on first `onToolStart`/`onTextDelta`, each tick calls `scheduleFlush()`, store interval handle as class field [owner:api-engineer] [beads:nv-tpjjk]
- [x] [1.3] [P-1] Clean up tick interval in `finalize()` and `abort()` — call `clearInterval()` alongside existing `clearTimeout(flushTimer)` cleanup [owner:api-engineer] [beads:nv-p0gf0]
- [x] [1.4] [P-2] Remove `completedTools` tracking — delete the array field, remove push/shift logic from `onToolDone()`, simplify class to only track `activeTools` + `firstEventAt` [owner:api-engineer] [beads:nv-ovat8]

## E2E Batch

- [ ] [2.1] [deferred] Manual test — send a message that triggers tool use, verify Telegram shows single-tool ticker updating every second with correct total [owner:user] [beads:nv-b35xd]
