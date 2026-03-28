# Proposal: Add Circuit Breaking and Health-Aware Routing

## Change ID
`add-circuit-breaking`

## Summary

Add circuit-breaking and health-aware routing to the tool-router. Currently, the dispatcher in
`routes/dispatch.ts` routes to services regardless of health state, returning 502 when services
are down. The health endpoint in `routes/health.ts` aggregates status but the dispatcher ignores
it entirely.

## Context
- Package: `packages/tools/tool-router/` (Hono on port 4100)
- `routes/dispatch.ts` looks up tool in the static registry (`registry.ts`), forwards HTTP request
  to the target service, returns 502 on fetch failure
- `routes/health.ts` checks all 9 services in parallel (3s timeout each), returns aggregate
  status (`healthy` / `degraded` / `unhealthy`)
- There is NO connection between health data and dispatch decisions
- When a service is down, every request to that service's tools fails with 502 after a full
  fetch timeout
- Related: `add-fleet-health-monitor` (daemon-side fleet monitoring — separate concern),
  `add-fleet-client-retry` (completed — retry logic in daemon's fleet client)

## Motivation

In a 10-service fleet, individual service failures are inevitable. Without circuit-breaking, the
user experiences slow failures (full fetch timeout per call) instead of fast failures with
actionable error responses. The health check data already exists in the router but is not used
for routing decisions.

A circuit breaker gives three benefits:
1. **Fast failure** -- OPEN circuit returns 503 immediately instead of waiting for a timeout
2. **Self-healing** -- HALF_OPEN state automatically tests recovery without manual intervention
3. **Observability** -- circuit state transitions logged at WARN level, exposed in /health

## Requirements

### Req-1: Circuit Breaker State Machine

Add a per-service circuit breaker in `packages/tools/tool-router/src/circuit-breaker.ts`.

States and transitions:

```
CLOSED (normal operation)
  |-- N consecutive failures OR error rate > threshold within window --> OPEN

OPEN (failing, reject immediately)
  |-- cooldown period elapsed --> HALF_OPEN

HALF_OPEN (testing recovery, allow single probe request)
  |-- probe succeeds --> CLOSED
  |-- probe fails --> OPEN
```

Default thresholds:
- Consecutive failure threshold: 3
- Error rate threshold: 50% within a 60-second sliding window
- Cooldown period: 30 seconds (configurable)

The breaker tracks requests in a fixed-size ring buffer (sliding window) to bound memory growth.
On startup, all services default to CLOSED (optimistic -- no assumed failures).

Exported interface:

```typescript
export type CircuitState = "CLOSED" | "OPEN" | "HALF_OPEN";

export interface CircuitBreakerConfig {
  failureThreshold: number;      // consecutive failures to trip (default: 3)
  errorRateThreshold: number;    // rate 0-1 to trip (default: 0.5)
  errorRateWindowMs: number;     // sliding window size (default: 60_000)
  cooldownMs: number;            // OPEN -> HALF_OPEN delay (default: 30_000)
  ringBufferSize: number;        // max entries in sliding window (default: 100)
}

export interface CircuitBreakerSnapshot {
  state: CircuitState;
  failures: number;
  successes: number;
  lastFailureAt: string | null;  // ISO 8601
  lastStateChange: string;       // ISO 8601
}

export class CircuitBreaker {
  constructor(serviceName: string, config?: Partial<CircuitBreakerConfig>);

  get state(): CircuitState;
  get serviceName(): string;

  /** Check if a request should be allowed through. */
  allowRequest(): boolean;

  /** Record a successful request. */
  onSuccess(): void;

  /** Record a failed request. */
  onFailure(): void;

  /** Force state update from external health data. */
  forceState(state: CircuitState): void;

  /** Return current state for observability. */
  snapshot(): CircuitBreakerSnapshot;
}
```

`allowRequest()` behavior:
- CLOSED: always returns `true`
- OPEN: returns `false` unless cooldown has elapsed, in which case transitions to HALF_OPEN
  and returns `true`
- HALF_OPEN: returns `true` for exactly one request (the probe), then blocks until the probe
  result is recorded

### Req-2: Health-Aware Dispatch

Modify `routes/dispatch.ts` to check the circuit breaker state before forwarding:

1. After registry lookup succeeds, call `breaker.allowRequest()` for the target service
2. If `false` (OPEN): return 503 immediately with:
   ```json
   {
     "error": "service_unavailable",
     "service": "<serviceName>",
     "tool": "<toolName>",
     "circuitState": "OPEN",
     "retryAfter": <seconds until cooldown expires>
   }
   ```
   Include `Retry-After` HTTP header with the same value.
3. If `true` (CLOSED or HALF_OPEN): forward request as normal
4. On successful downstream response (2xx): call `breaker.onSuccess()`
5. On failed downstream response (5xx or fetch error): call `breaker.onFailure()`
6. 4xx responses from downstream are NOT counted as failures (client errors, not service failures)

### Req-3: Health Integration

Extend `routes/health.ts` to integrate with circuit breaker state:

1. After each health check cycle completes, update circuit breaker states from live health data:
   - Service responded 200: call `breaker.onSuccess()` (may transition HALF_OPEN -> CLOSED)
   - Service unreachable or non-200: call `breaker.onFailure()` (may transition CLOSED -> OPEN)

2. Expose circuit breaker state per service in the GET /health response:

   ```json
   {
     "status": "degraded",
     "services": {
       "memory-svc": {
         "status": "healthy",
         "url": "http://127.0.0.1:4101",
         "latency_ms": 4,
         "circuitBreakerState": "CLOSED"
       },
       "schedule-svc": {
         "status": "unreachable",
         "url": "http://127.0.0.1:4106",
         "latency_ms": null,
         "circuitBreakerState": "OPEN"
       }
     },
     "healthy_count": 8,
     "total_count": 9
   }
   ```

The health route needs access to the `CircuitBreaker` instances. Pass them via a shared
`Map<string, CircuitBreaker>` created in `index.ts` and injected into both `dispatchRoute` and
`healthRoute`.

### Req-4: Metrics

Track in-memory per-service metrics and expose via a new `GET /metrics` endpoint.

Per-service counters:

```typescript
interface ServiceMetrics {
  totalRequests: number;
  totalFailures: number;
  circuitTrips: number;        // number of times CLOSED -> OPEN
  lastTripAt: string | null;   // ISO 8601
  circuitState: CircuitState;
}
```

Response format (JSON):

```json
{
  "uptime_secs": 3600,
  "services": {
    "memory-svc": {
      "totalRequests": 142,
      "totalFailures": 3,
      "circuitTrips": 1,
      "lastTripAt": "2026-03-28T10:15:00.000Z",
      "circuitState": "CLOSED"
    }
  }
}
```

Log circuit state transitions at WARN level:

```
WARN  Circuit OPEN for schedule-svc: 3 consecutive failures
WARN  Circuit HALF_OPEN for schedule-svc: cooldown elapsed, allowing probe
WARN  Circuit CLOSED for schedule-svc: probe succeeded
```

Add `metricsRoute(app: Hono, breakers: Map<string, CircuitBreaker>)` in
`routes/metrics.ts` and wire it in `index.ts`.

## Scope
- **IN**: `packages/tools/tool-router/src/circuit-breaker.ts` (new -- state machine),
  `packages/tools/tool-router/src/routes/dispatch.ts` (modified -- check breaker before forwarding),
  `packages/tools/tool-router/src/routes/health.ts` (extended -- expose breaker states),
  `packages/tools/tool-router/src/routes/metrics.ts` (new -- /metrics endpoint),
  `packages/tools/tool-router/src/index.ts` (extended -- create breakers map, wire /metrics route)
- **OUT**: Individual tool services (no changes to ports 4101-4109), daemon code
  (`packages/daemon/`), dashboard (`apps/`), persistent metrics storage, Prometheus format

## Impact

| Area | Change |
|------|--------|
| `packages/tools/tool-router/src/circuit-breaker.ts` | New -- `CircuitBreaker` class, `CircuitBreakerConfig`, state machine, ring buffer |
| `packages/tools/tool-router/src/routes/dispatch.ts` | Modified -- check `allowRequest()` before forwarding, record success/failure |
| `packages/tools/tool-router/src/routes/health.ts` | Extended -- update breaker states from health data, add `circuitBreakerState` to response |
| `packages/tools/tool-router/src/routes/metrics.ts` | New -- `GET /metrics` endpoint exposing per-service counters |
| `packages/tools/tool-router/src/index.ts` | Extended -- instantiate `Map<string, CircuitBreaker>`, pass to routes, add /metrics |

No changes to registry.ts (tool-to-service mapping unchanged). No DB schema changes.

## Risks

| Risk | Mitigation |
|------|-----------|
| False positives (healthy service marked as failing) | Conservative thresholds (3 consecutive failures, not 1) + HALF_OPEN recovery path allows automatic recovery |
| Memory growth from request tracking | Sliding window uses a fixed-size ring buffer (default 100 entries); oldest entries evicted automatically |
| Startup: all services initially unknown | Default to CLOSED (optimistic) on startup -- first real failures trigger the breaker, not assumptions |
| Health check and dispatch race on breaker state | Both paths call `onSuccess()`/`onFailure()` on the same breaker instance; state machine transitions are idempotent so concurrent updates converge correctly |
| Cooldown too aggressive (30s default) | Configurable per `CircuitBreakerConfig`; can be tuned without code changes |
