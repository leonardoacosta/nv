# Implementation Tasks

<!-- beads:epic:nv-0fpr -->

## DB Batch

(No DB tasks)

## API Batch

- [x] [2.1] [P-2] Create packages/daemon/src/lib/quiet-hours.ts exporting isQuietHours(now, quietStart, quietEnd) with normal-window, midnight-wrap, and zero-length-window handling [owner:api-engineer]
- [x] [2.2] [P-2] Update proactive.ts: delete local isQuietHours, import from lib/quiet-hours.ts, update call site to pass config.quietStart and config.quietEnd [owner:api-engineer]
- [x] [2.3] [P-2] Update scheduler.ts: delete local isQuietHours, import from lib/quiet-hours.ts, update both call sites to pass new Date() explicitly [owner:api-engineer]
- [x] [2.4] [P-2] Add logger?: Logger param to detectObligations(), log warn on catch with error context, add TODO(extract-agent-query-factory) comment above createClient(), run pnpm tsc --noEmit to verify zero errors [owner:api-engineer]

## UI Batch

(No UI tasks)

## E2E Batch

(No E2E tasks)
