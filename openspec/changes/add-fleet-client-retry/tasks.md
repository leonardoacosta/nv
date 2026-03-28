# Implementation Tasks

<!-- beads:epic:nv-e7yd -->

## DB Batch

(No DB tasks)

## API Batch

- [x] [2.1] [P-2] Refactor fleet-client.ts: extract `fleetRequest()` single-attempt helper, add 5xx retry loop with 500ms backoff and warn logging in both `fleetPost()` and `fleetGet()` [owner:api-engineer]

## UI Batch

(No UI tasks)

## E2E Batch

- [ ] [4.1] [P-2] Test: verify fleetPost/fleetGet retries once on 503 and succeeds, does not retry on 4xx, and throws after two consecutive 5xx failures [owner:e2e-engineer]
