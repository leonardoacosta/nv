# Proposal: Add Fleet Client Retry Logic

## Change ID
`add-fleet-client-retry`

## Summary
Add a single retry with 500ms backoff to `fleetPost()` and `fleetGet()` for 5xx responses. Prevents transient fleet service restarts from falling through to Tier 3 (Agent SDK), where a query costs $0.10–2.00 instead of $0.00.

## Context
- Extends: `packages/daemon/src/fleet-client.ts`
- Related: `add-tool-router` (completed — introduced fleet routing tiers), `add-smart-routing` (completed — Tier 1/2/3 dispatch logic)

## Motivation
Fleet services (Hono microservices on ports 4100–4109) restart periodically during deploys and config changes. During the brief window between process exit and readiness, any in-flight `fleetPost()` or `fleetGet()` call receives a 503 and throws `FleetClientError` immediately — no retry attempted. The caller in the smart-routing tier interprets the error as a definitive failure and falls through to Tier 3 (Agent SDK). A query that should cost $0.00 via a fleet tool instead costs $0.10–2.00.

A single retry after 500ms recovers the vast majority of transient restart windows without meaningfully increasing worst-case latency (10s total vs 5s current on a hard failure).

## Requirements

### Req-1: Retry on 5xx responses
`fleetPost()` and `fleetGet()` must retry once after 500ms when the response status is 5xx (500–599), including synthesized 503/504 codes thrown for network errors and timeouts. 4xx responses (client errors) must NOT be retried.

### Req-2: Per-attempt timeout preserved
Each attempt uses the existing 5-second timeout independently. The retry attempt gets its own `AbortController` and timer. Total worst-case latency for a hard failure is 10.5s (5s attempt 1 + 0.5s backoff + 5s attempt 2).

### Req-3: Retry logged at warn level
When a retry is triggered, log a `warn` entry with the url, status, and attempt number so failures are observable without being noisy on success.

## Scope
- **IN**: Retry logic in `fleetPost()` and `fleetGet()`, retry logging
- **OUT**: Configurable retry count, exponential backoff, circuit breaker, changes to caller routing logic, changes to fleet service implementations

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/fleet-client.ts` | Extract single-attempt helper, wrap in retry loop for 5xx, add warn logging on retry |

## Risks
| Risk | Mitigation |
|------|-----------|
| Retry amplifies load during a full fleet outage | Single retry only — at most 2x requests; no cascading retry loops possible |
| 10.5s worst-case blocks caller for too long | Existing callers already accept up to 5s; the additional 5.5s is bounded and rare |
| 4xx retried by mistake | Explicit `status >= 500` guard before retry; 4xx throws immediately as before |
