# Fleet Client Retry Logic

## ADDED Requirements

### Requirement: Single retry on 5xx responses with 500ms backoff

`fleetPost()` and `fleetGet()` in `packages/daemon/src/fleet-client.ts` MUST retry exactly once after a 500ms delay when the response status is in the 5xx range (500–599), including synthesized 503/504 codes for network errors and timeouts. Requests that receive a 4xx response MUST NOT be retried and SHALL throw `FleetClientError` immediately on the first attempt.

#### Scenario: 503 on first attempt retries and succeeds

Given a fleet service that returns 503 on the first request and 200 on the second,
when `fleetPost()` is called,
then it waits 500ms, retries the request, and returns the successful response without throwing.

#### Scenario: 4xx is not retried

Given a fleet service that returns 404,
when `fleetGet()` is called,
then it throws `FleetClientError` immediately with no retry attempted.

### Requirement: Per-attempt timeout is independent and retry is logged at warn level

Each attempt MUST use its own `AbortController` and 5-second timeout, keeping worst-case latency at 10.5s (5s attempt 1 + 0.5s backoff + 5s attempt 2). When a retry is triggered, the implementation MUST log a `warn` entry containing the `url`, `status`, and attempt number so transient failures are observable.

#### Scenario: Retry warning is emitted with context

Given a fleet service that returns 503 on the first attempt,
when the retry is triggered,
then a warn log entry is emitted containing the target URL, status code `503`, and attempt number `1` before the second request is sent.
