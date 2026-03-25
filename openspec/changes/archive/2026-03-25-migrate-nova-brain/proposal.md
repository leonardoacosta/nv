# Proposal: Migrate Nova's Brain to CC Session

## Change ID
`migrate-nova-brain`

## Summary

Move Nova's conversation engine from the Rust daemon's cold-start Claude CLI subprocess model
into a persistent CC session managed by the Next.js dashboard. The daemon becomes a thin message
broker: Telegram long-poll receives messages, forwards them to a dashboard API endpoint, the CC
session handles all reasoning and tool calls, and the response is sent back to Telegram. Target
latency: under 10s from Telegram message received to reply sent.

## Context

- Phase: Wave 2c (depends on `cc-session-management`)
- Extends: `crates/nv-daemon/src/orchestrator.rs`, `crates/nv-daemon/src/worker.rs`,
  `crates/nv-daemon/src/claude.rs`, `crates/nv-daemon/src/conversation.rs`
- Depends on: `cc-session-management` (dashboard exposes `/api/nova/message` endpoint backed by
  a managed CC session)
- No downstream dependents

## Motivation

### The Latency Problem

Current cold-start path:

```
Telegram message received
  --> orchestrator classify (~1ms)
  --> worker spawned
  --> build_system_context() (~5ms)
  --> spawn `claude -p` subprocess (~2-5s process start)
  --> CC session init / SessionStart hooks (~3-8s)
  --> tool loop (0-N rounds × 3-8s each)
  --> response delivered
Total: 18-30s for a tool-using response, 8-12s for a simple reply
```

The 18-30s figure is confirmed by production observations. The `PersistentSession` in
`claude.rs` was intended to avoid cold-start overhead but is currently hard-disabled
(`fallback_only: true`) due to a CC 2.1.81 stream-json bug where response data is never
returned. So every request pays the full cold-start cost.

### The Architecture Mismatch

The current architecture conflates responsibilities:
- Rust daemon owns the conversation loop (tool dispatch, history management, system prompt)
- `ConversationStore` holds session state in-memory with a 10-minute timeout
- `build_system_context()` reads identity/soul/user files from `~/.nv/` on every turn
- `send_messages_cold_start_with_image()` spawns a `claude -p` subprocess per turn

Moving reasoning to a CC session managed by the Next.js dashboard separates these concerns
cleanly: the daemon handles channel I/O, the dashboard handles intelligence.

### Target Architecture

```
Telegram (long-poll)
  --> nv-daemon (classify + forward, ~1ms)
  --> POST /api/nova/message (dashboard Next.js API)
  --> CC Session (persistent, Docker)
  --> nv-tools MCP (existing, unchanged)
  --> response JSON
  --> nv-daemon sends Telegram reply
Total: 3-8s (CC session already warm, no subprocess spawn)
```

## Requirements

### Req-1: Dashboard Message API Endpoint

The `cc-session-management` spec creates a Next.js API route that accepts a forwarded message
and returns Nova's response. This spec consumes that endpoint from the daemon side.

Expected contract (established by `cc-session-management`):

```
POST /api/nova/message
Authorization: Bearer <NOVA_DASHBOARD_TOKEN>
Content-Type: application/json

{
  "message": "<user text>",
  "chat_id": 12345678,
  "message_id": 99,
  "channel": "telegram",
  "system_context": "<full system prompt string>"
}

Response 200:
{
  "reply": "<Nova's text response>",
  "session_id": "<cc-session-id>"
}

Response 503:
{
  "error": "cc_session_unavailable"
}
```

### Req-2: Daemon Forwards Messages to Dashboard

The orchestrator's worker dispatch path is replaced with an HTTP forward when the dashboard is
reachable. The worker pool is retained but repurposed: workers now make the HTTP call and handle
the response, rather than running a local tool loop.

The system context (system prompt + identity + soul + user files) is still built by
`build_system_context()` in Rust and forwarded with each message. The CC session on the
dashboard uses it as the system prompt for that turn. This keeps personality and operational
rules in Rust (no dashboard config required).

### Req-3: ConversationStore Moves to CC Session

The in-memory `ConversationStore` (20 turns, 50K chars, 10-min timeout) is deprecated. The CC
session on the dashboard maintains conversation continuity natively — the persistent CC process
accumulates context across turns. The daemon no longer manages conversation history.

The `conversation_store` field in `SharedDeps` is retained as `Option<Arc<Mutex<ConversationStore>>>`
but set to `None` when dashboard forwarding is active. This allows the cold-start fallback path
to continue using it unchanged.

### Req-4: Cold-Start Fallback

If the dashboard API is unreachable (503, timeout, connection refused), the daemon falls back to
the existing cold-start path. The fallback is transparent to the user — they receive the same
Telegram reply, just with the old latency.

Fallback triggers:
- HTTP connection to dashboard times out (>5s connect timeout)
- Dashboard returns 5xx or connection refused
- `NOVA_DASHBOARD_URL` env var is unset or empty

Fallback does NOT trigger on:
- 4xx errors (auth failure, bad request) — these are bugs, not availability issues; log + alert

### Req-5: System Prompt Forwarding

`build_system_context()` in `agent.rs` currently builds the full system prompt by reading
files from `~/.nv/`. This function is called once per worker invocation. The built context
string is included in the forwarded request body so the CC session on the dashboard can apply
it as the system prompt for the turn.

No changes to `agent.rs` content — the same prompt construction logic applies. The result is
serialized and sent over HTTP instead of being passed directly to a local subprocess.

### Req-6: Authentication

The daemon authenticates to the dashboard API using a shared secret token stored in the `nv.toml`
config under `[dashboard]` section:

```toml
[dashboard]
url = "http://nova-dashboard.lan"    # or Tailscale hostname
token = ""                           # set via env var NOVA_DASHBOARD_TOKEN
```

The token is injected as `Authorization: Bearer <token>` on each request. If `token` is empty,
forwarding is disabled and cold-start is used.

### Req-7: Telegram Reactions Unchanged

The existing reaction flow (`👀` on receive, `⏳` on worker start, `✅` on complete, `❌` on
error) continues unchanged. From the orchestrator's perspective, "worker" still means "something
that processes the message and produces a response" — the internals of the worker change, not
the event model.

### Req-8: nv.toml Config Extension

Add a `[dashboard]` section to the config schema:

```toml
[dashboard]
# URL of the Next.js dashboard. Empty = forwarding disabled, use cold-start.
url = ""
# Bearer token for /api/nova/message. Set via NOVA_DASHBOARD_TOKEN env var.
token = ""
# Connect timeout for the dashboard API in seconds. Default 5.
connect_timeout_secs = 5
# Request timeout (total) in seconds. Default 30.
request_timeout_secs = 30
```

## Scope

**IN**:
- HTTP forwarding client in `nv-daemon` (new `src/dashboard.rs` module)
- Worker refactor: forward to dashboard instead of running local tool loop
- `nv.toml` config extension for `[dashboard]` section
- Cold-start fallback when dashboard unavailable
- `ConversationStore` effectively bypassed when dashboard active (retained for fallback)
- System context forwarded with each message

**OUT**:
- The dashboard API endpoint itself (owned by `cc-session-management`)
- Changes to `build_system_context()` content or file loading logic
- Changes to `nv-tools` MCP server
- Removal of `ConversationStore` or `claude.rs` (retained for fallback, removed in Wave 3+)
- Any changes to the Telegram long-poll or channel management
- Multi-channel forwarding (Teams, email) — Telegram only in this spec

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/dashboard.rs` | New: HTTP client for forwarding messages to dashboard |
| `crates/nv-daemon/src/worker.rs` | Refactor: forward path replaces local tool loop |
| `crates/nv-daemon/src/config.rs` | Add `DashboardConfig` struct + `[dashboard]` section |
| `crates/nv-daemon/src/main.rs` | Wire `DashboardConfig` into `SharedDeps` |
| `config/nv.toml` | Add `[dashboard]` section |
| `crates/nv-daemon/Cargo.toml` | Add `reqwest` with `json` + `rustls-tls` features (if not present) |
| `crates/nv-daemon/src/conversation.rs` | No change — retained for fallback path |
| `crates/nv-daemon/src/claude.rs` | No change — retained for fallback path |
| `crates/nv-daemon/src/agent.rs` | No change — `build_system_context()` used unchanged |

## Data Flow After Migration

```
[Telegram] --long-poll--> [nv-daemon orchestrator]
                                    |
                            classify_trigger()
                                    |
                        +---------- v -----------+
                        | WorkerPool::dispatch() |
                        +---------- v -----------+
                                    |
                         Worker::run_forward()
                                    |
                         build_system_context()
                                    |
                     +--- DashboardClient::forward() ---+
                     |                                  |
               [Dashboard /api/nova/message]     [cold-start fallback]
                     |                                  |
               CC Session (warm)              claude -p subprocess
                     |                                  |
               response JSON                     ApiResponse
                     |                                  |
                     +------------- v ------------------+
                                    |
                          send Telegram reply
                          react ✅ / ❌
                          log to DiaryWriter + MessageStore
```

## Risks

| Risk | Mitigation |
|------|-----------|
| CC session on dashboard crashes mid-conversation | CC session manager (cc-session-management) handles restart; daemon falls back to cold-start for the current turn |
| Network latency homelab → dashboard (Tailscale) | Same LAN or Tailscale subnet; expect <5ms RTT |
| System context too large for HTTP body | Current max context ~8KB; set 64KB body limit on dashboard API |
| Dashboard auth token leaked | Token in Doppler, never in git; `nv.toml` reads from env var |
| ConversationStore diverges from CC session state | `ConversationStore` bypassed when dashboard active; no sync needed |
| Dashboard not ready before this spec lands | Spec depends on `cc-session-management`; gated in wave ordering |

## Migration Path

1. Deploy `cc-session-management` first (dashboard API live, CC session running)
2. Deploy this spec (daemon forwards to dashboard)
3. Verify sub-10s latency on Telegram in production
4. Optionally: remove `ConversationStore` and `PersistentSession` in a future cleanup spec
   (Wave 3 or later, once cold-start fallback is no longer needed)
