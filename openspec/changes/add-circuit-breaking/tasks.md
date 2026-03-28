# Implementation Tasks

## DB Batch
(No DB tasks)

## API Batch
- [ ] [2.1] [P-1] Create `CircuitBreaker` class with state machine (CLOSED/OPEN/HALF_OPEN), ring buffer sliding window, and `allowRequest()`/`onSuccess()`/`onFailure()`/`forceState()`/`snapshot()` methods in `packages/tools/tool-router/src/circuit-breaker.ts` [owner:api-engineer]
- [ ] [2.2] [P-1] Create `CircuitBreakerConfig` type with defaults and `CircuitBreakerSnapshot` type in `packages/tools/tool-router/src/circuit-breaker.ts` [owner:api-engineer]
- [ ] [2.3] [P-2] Modify `packages/tools/tool-router/src/index.ts` to instantiate `Map<string, CircuitBreaker>` from registry services and pass to route handlers [owner:api-engineer]
- [ ] [2.4] [P-2] Modify `packages/tools/tool-router/src/routes/dispatch.ts` to check `allowRequest()` before forwarding, return 503 with `Retry-After` header when OPEN, call `onSuccess()`/`onFailure()` based on downstream response status [owner:api-engineer]
- [ ] [2.5] [P-2] Extend `packages/tools/tool-router/src/routes/health.ts` to accept breakers map, update breaker states from health check results, and include `circuitBreakerState` per service in GET /health response [owner:api-engineer]
- [ ] [2.6] [P-2] Create `packages/tools/tool-router/src/routes/metrics.ts` with GET /metrics endpoint exposing per-service counters (totalRequests, totalFailures, circuitTrips, lastTripAt, circuitState) and uptime [owner:api-engineer]
- [ ] [2.7] [P-3] Wire `/metrics` route in `packages/tools/tool-router/src/index.ts` [owner:api-engineer]
- [ ] [2.8] [P-3] Add WARN-level logging for circuit state transitions (CLOSED->OPEN, OPEN->HALF_OPEN, HALF_OPEN->CLOSED) [owner:api-engineer]

## UI Batch
(No UI tasks)

## E2E Batch
- [ ] [4.1] Test: circuit breaker transitions CLOSED -> OPEN after consecutive failure threshold [owner:e2e-engineer]
- [ ] [4.2] Test: OPEN circuit returns 503 immediately with correct JSON body and Retry-After header [owner:e2e-engineer]
- [ ] [4.3] Test: OPEN -> HALF_OPEN after cooldown, probe request allowed through [owner:e2e-engineer]
- [ ] [4.4] Test: HALF_OPEN -> CLOSED on probe success, HALF_OPEN -> OPEN on probe failure [owner:e2e-engineer]
- [ ] [4.5] Test: error rate threshold triggers OPEN within sliding window [owner:e2e-engineer]
- [ ] [4.6] Test: 4xx downstream responses are NOT counted as failures [owner:e2e-engineer]
- [ ] [4.7] Test: GET /health response includes circuitBreakerState per service [owner:e2e-engineer]
- [ ] [4.8] Test: GET /metrics returns per-service counters and uptime [owner:e2e-engineer]
