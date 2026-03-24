# Test Ping Endpoint

## ADDED Requirements

### Requirement: Req-1: Test ping endpoint

The daemon MUST expose a `GET /test/ping` endpoint on the existing HTTP server that injects a
synthetic message trigger into the worker pipeline and returns the result as JSON.

#### Scenario: Successful ping
Given the daemon is running and Claude CLI is functional
When `GET /test/ping` is called
Then a synthetic `Trigger::Message` with content "ping" is injected into the trigger channel
And the worker processes it through the full Claude cold-start pipeline
And the response is returned as JSON with `ok: true`
And `elapsed_ms` is less than 60000
And `prompt_bytes` is less than 10000

#### Scenario: Pipeline failure
Given the daemon is running but Claude CLI is unavailable
When `GET /test/ping` is called
Then the response is returned as JSON with `ok: false`
And an `error` field describes the failure

### Requirement: Req-2: Timeout protection

The endpoint MUST enforce a 60-second timeout to prevent hanging requests.

#### Scenario: Worker timeout
Given the daemon is running but the worker hangs
When `GET /test/ping` is called and 60 seconds elapse without completion
Then the response is `{"ok": false, "error": "timeout", "elapsed_ms": 60000}`

### Requirement: Req-3: Concurrency guard

The endpoint MUST reject concurrent test pings to prevent resource exhaustion.

#### Scenario: Concurrent ping rejected
Given a test ping is already in progress
When a second `GET /test/ping` is called
Then the response is `{"ok": false, "error": "test already in progress"}` with HTTP 429
