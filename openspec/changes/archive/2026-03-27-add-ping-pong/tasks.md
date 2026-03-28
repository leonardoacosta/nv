# Implementation Tasks

<!-- beads:epic:nv-s779 -->

## DB Batch

(No DB tasks)

## API Batch

- [x] [2.1] [P-1] Add ping intercept in index.ts after cancel phrase check — detect /^ping$/i, reply "pong" with reply_to, return early [owner:api-engineer] [beads:nv-qbur]

## UI Batch

(No UI tasks)

## E2E Batch

- [x] [4.1] [P-1] Create packages/e2e/ package with ping-health-check.sh script — send ping via Bot API, poll getUpdates for pong, exit 0/1 [owner:e2e-engineer] [beads:nv-7l7v]
- [x] [4.2] [P-2] Integrate ping health check into deploy/pre-push.sh post-deploy section [owner:e2e-engineer] [beads:nv-2bmc]
- [x] [4.3] [P-2] Test: send ping, verify pong response and no conversation save [owner:e2e-engineer] [beads:nv-rv0i]
