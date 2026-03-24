# Proposal: Add test ping endpoint for e2e pipeline validation

## Change ID
`add-test-ping-endpoint`

## Summary
Add a `GET /test/ping` HTTP endpoint that injects a synthetic message into the worker pipeline,
waits for the Claude response, and returns pass/fail with timing and prompt size metrics.

## Context
- Extends: `crates/nv-daemon/src/http.rs` (add route), `crates/nv-daemon/src/worker.rs` (response capture)
- Related: tonight's debugging — prompt bloat (53KB) went undetected until manual log inspection

## Motivation
There is no automated way to verify the full message-to-response pipeline works. Tonight's
debugging required manually sending Telegram messages and parsing journalctl output. A local
HTTP endpoint that exercises the same worker+Claude path would catch regressions (prompt bloat,
subprocess crashes, timeout issues) immediately after deploy.

## Requirements

### Req-1: Test ping endpoint
`GET /test/ping` on the existing HTTP server (localhost:8400) injects a synthetic
`Trigger::Message` with content "ping" into the trigger channel, waits for the worker to
complete, and returns a JSON response with pass/fail status, elapsed time, and prompt size.

### Req-2: Response format
Returns JSON: `{"ok": bool, "elapsed_ms": u64, "prompt_bytes": u64, "response_preview": string}`
where `response_preview` is the first 200 chars of Claude's response. On timeout or error,
`ok` is false and an `error` field is included.

### Req-3: Timeout
The endpoint has a 60-second timeout. If the worker doesn't complete within 60s, returns
`{"ok": false, "error": "timeout", "elapsed_ms": 60000}`.

## Scope
- **IN**: HTTP endpoint, synthetic trigger injection, response capture, JSON result
- **OUT**: Telegram integration testing, multi-turn conversation testing, load testing

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/http.rs` | Add `GET /test/ping` route handler |
| `crates/nv-daemon/src/main.rs` | Pass trigger_tx to HTTP server for injection |

## Risks
| Risk | Mitigation |
|------|-----------|
| Test pings consume Claude API credits | Single short turn (~$0.01), no tool calls expected |
| Concurrent test pings could overload workers | Rate-limit to 1 concurrent test (mutex or semaphore) |
