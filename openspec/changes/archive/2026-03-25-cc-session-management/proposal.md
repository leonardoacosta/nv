# Proposal: CC Session Management

## Change ID
`cc-session-management`

## Summary

Add Claude Code persistent session management to the Next.js dashboard. The dashboard owns a
long-running CC session running in a Docker container. Telegram messages flow from the daemon
into the dashboard via HTTP, then into the managed CC session, eliminating cold-start subprocess
spawning per message on the daemon side.

## Context
- Phase: Wave 2b — depends on `extract-nextjs-dashboard`
- Related beads: nv-k86 (cc-native-nova), nv-cfg (cc-session-setup)
- Extends: `apps/dashboard/` (new session management pages + API routes)
- Replaces: cold-start `claude -p` path in `crates/nv-daemon/src/claude.rs` (for routed messages)
- Related: `crates/nv-daemon/src/worker.rs` (WorkerPool/SharedDeps), `crates/nv-daemon/src/orchestrator.rs` (message routing)

## Motivation

Every message currently cold-starts a `claude -p` subprocess on the daemon (~8-14s). The CC CLI
supports persistent sessions, but running that subprocess inside the Rust daemon creates lifecycle
complexity (see `PersistentSession` in `claude.rs`, currently `fallback_only: true` due to a
stream-json + hooks bug in CC 2.1.81).

Moving the session into a Docker container managed by the dashboard sidesteps the daemon/subprocess
coupling entirely. The dashboard can use the CC SDK or CLI in an isolated container with a clean
environment. The daemon forwards messages over HTTP and receives responses asynchronously — no
subprocess pipes, no stdin/stdout deadlocks.

Benefits:
1. **Latency** — persistent CC session eliminates cold-start per message
2. **Isolation** — session crash can't take down the daemon
3. **Observability** — dashboard UI shows session state, logs, and health
4. **Control** — start/stop/restart from the dashboard without redeploying the daemon
5. **Auth** — CC OAuth lives in the container's environment, separate from daemon config

## Requirements

### Req-1: Session Container Lifecycle

The dashboard manages one CC session Docker container (`nova-cc-session`). The container runs the
CC CLI in persistent stream-json mode (`claude --input-format stream-json --output-format
stream-json --dangerously-skip-permissions`). The dashboard exposes start/stop/restart controls and
tracks container state (running, stopped, error).

Container configuration:
- Image: builds from `apps/dashboard/docker/cc-session/Dockerfile`
- Named volume for CC auth (`~/.config/claude/` equivalent)
- Named volume for CC sandbox home
- Env vars injected from dashboard environment (ANTHROPIC_MODEL, ANTHROPIC_API_KEY if SDK mode)
- Network: same Docker network as the daemon

### Req-2: Session Health Monitoring

The dashboard polls session health on a 15-second interval:
- Container state via Docker API (running/stopped/restarting/dead)
- Last message timestamp (idle detection — no message in 10 minutes = idle)
- Consecutive error count (3 errors in 60 seconds = unhealthy)

Auto-restart policy:
- Container exits unexpectedly → restart immediately, up to 3 times
- After 3 restarts within 5 minutes → mark as error, require manual restart
- Idle for >30 minutes (no active session) → allow natural idle without restart

Session state enum: `active` | `idle` | `starting` | `stopping` | `error`

### Req-3: Dashboard Session UI

Session management page at `/session` in the dashboard:

- Status badge: current state with color indicator (active=green, idle=yellow, error=red)
- Uptime display: how long the session has been running
- Last activity: timestamp of last message processed
- Message counter: total messages processed this session
- Action buttons: Start, Stop, Restart (disabled based on current state)
- Recent logs: last 50 lines of container stdout/stderr, auto-refreshed every 5s
- Error panel: current error with timestamp if state=error

### Req-4: Message Forwarding API

The dashboard exposes a POST endpoint at `/api/session/message`:

```
POST /api/session/message
Authorization: Bearer <DASHBOARD_SECRET>
Content-Type: application/json

{
  "message_id": "uuid",
  "chat_id": 123456,
  "text": "user message text",
  "context": { ... }   // optional: thread history, user metadata
}
```

Response (synchronous, waits for CC response):
```json
{
  "reply": "Nova's response text",
  "session_state": "active",
  "processing_ms": 2340
}
```

Timeout: 120s. If CC session is not ready (starting/error), returns 503 with `session_state`.

The endpoint streams the message into the CC container's stdin and reads back the response from
stdout, translating the stream-json events into the final reply text.

### Req-5: Daemon Message Routing

The daemon's orchestrator is updated to forward qualifying messages to the dashboard instead of
dispatching a local worker. Routing logic:

- If `DASHBOARD_URL` env var is set and dashboard is reachable: forward via HTTP
- If dashboard is unreachable or returns 503: fall back to local worker dispatch
- Forward: Message triggers classified as Query or Command (same classification as today)
- Keep local: Digest (cron), Callback, BotCommand, Chat, NexusEvent

The daemon sends the message to `/api/session/message`, receives the reply, and routes it back
to Telegram through the existing outbound message path. No changes to Telegram send logic.

### Req-6: Session State on Dashboard Home

The existing dashboard home page gains a session status widget:
- Current state badge
- Time since last message
- Quick restart button
- Link to full `/session` page

## Scope
- **IN**: CC session Docker container, container lifecycle management, health monitoring, session UI
  page, message forwarding API endpoint, daemon HTTP forwarding (with fallback), dashboard home
  session widget
- **OUT**: Multi-session support, CC SDK migration (stays CLI), streaming progressive Telegram
  updates, session replay/history persistence beyond last 50 log lines, auth flow UI (manual setup)

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/docker/cc-session/Dockerfile` | New: CC CLI container image |
| `apps/dashboard/src/app/session/page.tsx` | New: session management UI |
| `apps/dashboard/src/app/api/session/message/route.ts` | New: message forwarding endpoint |
| `apps/dashboard/src/app/api/session/status/route.ts` | New: session state SSE or polling endpoint |
| `apps/dashboard/src/lib/session-manager.ts` | New: Docker lifecycle + health monitoring service |
| `apps/dashboard/src/components/SessionWidget.tsx` | New: home page status widget |
| `crates/nv-daemon/src/orchestrator.rs` | Add dashboard forwarding path with fallback |
| `crates/nv-daemon/src/config.rs` | Add DASHBOARD_URL + DASHBOARD_SECRET config fields |

## Risks
| Risk | Mitigation |
|------|-----------|
| CC container auth expires silently | Health check verifies auth by sending probe message on startup |
| Dashboard unreachable → daemon messages lost | Fallback to local worker pool (existing path) |
| Slow CC response blocks forwarding endpoint | 120s timeout; daemon has its own per-message timeout |
| Docker socket access required by dashboard | Dashboard runs on same host as daemon; mount `/var/run/docker.sock` read/write |
| stream-json + hooks bug (CC 2.1.81) | Run container without SessionStart hooks; use `--no-session-persistence` |
| Message forwarding latency adds overhead | HTTP round-trip to localhost is <5ms; net win vs 8-14s cold start |
