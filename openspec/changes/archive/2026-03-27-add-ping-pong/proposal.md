# Proposal: Ping/Pong Health Probe

## Change ID
`add-ping-pong`

## Summary
Add a "ping" intercept in the daemon that instantly replies "pong" without touching the agent, queue, conversation history, or fleet services. Include an E2E test script in `packages/e2e/` that sends ping via the Telegram Bot API and validates the pong response.

## Context
- Extends: `packages/daemon/src/index.ts` (cancel phrase pattern at line ~510)
- Related: existing `/health` and `/status` Telegram commands, `GET /health` HTTP endpoint
- New package: `packages/e2e/` for E2E test scripts

## Motivation
Nova has no way to verify end-to-end message processing works without sending a real message that hits the Agent SDK ($0.10-2.00 per call). The thread routing deployment exposed this gap — the daemon was silently failing on every message due to a missing migration, but there was no automated check to catch it.

A synthetic "ping" message proves the full Telegram path works (polling → normalization → routing → response delivery) at zero API cost. The E2E test script provides a scriptable health check for post-deploy verification.

## Requirements

### Req-1: Ping intercept
Detect bare "ping" messages before they enter the routing cascade. Reply with "pong" immediately. No queue entry, no agent call, no conversation save, no obligation detection.

### Req-2: E2E test script
A bash script in `packages/e2e/` that sends "ping" to Nova via the Telegram Bot API, polls for the "pong" response, and exits 0 (success) or 1 (timeout/failure). Reads bot token and chat ID from environment.

### Req-3: Deploy integration
The pre-push hook's post-deploy health check calls the E2E ping test when the daemon was restarted.

## Scope
- **IN**: Ping intercept in daemon, E2E test script, deploy hook integration
- **OUT**: Fleet service health probing (already covered by `/health`), HTTP ping endpoint, dashboard integration

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/index.ts` | Ping intercept before routing cascade |
| `packages/e2e/` | New package with health-check script |
| `deploy/pre-push.sh` | Call E2E ping after daemon restart |

## Risks
| Risk | Mitigation |
|------|-----------|
| Bot token exposure in test script | Read from env vars, never hardcode |
| Telegram rate limiting on rapid ping | Single ping per deploy, not a loop |
| Polling timeout if daemon is slow to start | 30s timeout with clear error message |
