# Implementation Tasks

<!-- beads:epic:nv-gn1z -->

## DB Batch

(No DB tasks)

## API Batch

- [ ] [2.1] [P-1] Create FleetHealthMonitor class with probe(), start(), stop(), getSnapshot() in src/features/fleet-health/ [owner:api-engineer]
- [ ] [2.2] [P-1] Add FLEET_SERVICES registry constant with ports and critical flags [owner:api-engineer]
- [ ] [2.3] [P-2] Extend /health endpoint to include fleet snapshot in response [owner:api-engineer]
- [ ] [2.4] [P-2] Wire FleetHealthMonitor into daemon startup/shutdown lifecycle in index.ts [owner:api-engineer]
- [ ] [2.5] [P-3] Add Telegram notification on critical service state change [owner:api-engineer]

## UI Batch

(No UI tasks)

## E2E Batch

- [ ] [4.1] Test: FleetHealthMonitor detects service up/down transitions [owner:e2e-engineer]
