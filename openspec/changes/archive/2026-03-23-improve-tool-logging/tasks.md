# Implementation Tasks

<!-- beads:epic:nv-zeo -->

## Code Batch

- [x] [1.1] [P-1] Add tracing::info! at execute_tool entry (tool name + input keys) and exit (success + duration_ms) in mod.rs [owner:api-engineer] [beads:nv-bwa]
- [x] [1.2] [P-1] Add tracing to silent tool handlers: stripe, doppler, teams, resend, posthog, cloudflare, vercel, calendar, check [owner:api-engineer] [beads:nv-e86]
- [x] [1.3] [P-2] Add action_id correlation logging to PendingAction lifecycle in worker.rs, agent.rs, and callbacks.rs [owner:api-engineer] [beads:nv-ukg]
- [x] [1.4] [P-2] Increase SQLite input/output truncation limit from 500 to 2000 chars in messages.rs [owner:api-engineer] [beads:nv-14d]

## E2E Batch

- [x] [2.1] Test execute_tool entry/exit traces include tool name and duration [owner:test-writer] [beads:nv-m0l]
