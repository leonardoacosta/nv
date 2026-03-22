# Proposal: Teams Channel

## Change ID
`teams-channel`

## Summary

Native MS Graph API adapter replacing the Python webhook relay. Uses OAuth2 client credentials
for authentication, subscription webhooks for receiving channel messages, and the MS Graph REST
API for sending replies. Implements the existing `Channel` trait.

## Context
- Extends: `crates/nv-core/src/channel.rs` (implements `Channel` trait)
- Related: `crates/nv-daemon/src/telegram/` (reference Channel implementation), `crates/nv-daemon/src/http.rs` (existing axum HTTP server for health + webhooks), `crates/nv-core/src/config.rs` (config structs)

## Motivation

Nova currently receives Teams messages via a Python webhook relay that forwards to Telegram. This
adds latency, a Python runtime dependency, and requires maintaining a separate service. A native
Rust adapter using the MS Graph API eliminates the relay and brings Teams on par with Telegram as
a first-class channel.

Benefits:
1. **Direct integration** -- no relay service, no Python dependency
2. **Bidirectional** -- receive via subscription webhooks, send via Graph REST API
3. **OAuth2 compliance** -- proper token refresh, no hardcoded credentials
4. **Channel trait** -- plugs directly into the agent loop

## Requirements

### Req-1: OAuth2 Client Credentials

Implement MS Graph OAuth2 client credentials flow:
- `POST https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token` with client_id, client_secret, scope (`https://graph.microsoft.com/.default`)
- Cache the access token with expiry tracking
- Auto-refresh before expiry (with buffer, e.g. 5 minutes before)
- Store tokens in memory only (not persisted to disk)

### Req-2: Subscription Webhook Registration

Register a subscription with MS Graph for channel message notifications:
- `POST /subscriptions` with resource `teams/getAllMessages` or per-channel `chats/{id}/messages`
- Subscription has a max lifetime (60 minutes for channel messages) -- auto-renew before expiry
- Handle the validation handshake: MS Graph sends a `validationToken` query param on creation, respond with 200 + token as plain text

### Req-3: REST API Client

HTTP client (reqwest) for sending messages via MS Graph:
- `POST /teams/{team-id}/channels/{channel-id}/messages` for channel replies
- `POST /chats/{chat-id}/messages` for direct messages
- Include `Authorization: Bearer {token}` header
- Handle 429 rate limiting with Retry-After

### Req-4: Channel Trait Implementation

`TeamsChannel` implements the `Channel` trait:
- `name()` returns `"teams"`
- `connect()` acquires OAuth token, registers subscription webhook
- `poll_messages()` drains buffered webhook-delivered messages, converts to `InboundMessage`
- `send_message()` sends via Graph REST API
- `disconnect()` deletes the subscription, drops token

### Req-5: Configuration

Add `[teams]` section to `nv.toml`:
- `tenant_id`: Azure AD tenant ID
- `team_ids`: Vec of team IDs to watch
- `channel_ids`: Vec of channel IDs to watch

Add secrets to `Secrets` struct (from environment variables):
- `MS_GRAPH_CLIENT_ID`
- `MS_GRAPH_CLIENT_SECRET`

### Req-6: Webhook HTTP Endpoint

Add a route to the existing axum HTTP server (`http.rs`) for receiving MS Graph subscription
notifications. The endpoint:
- Handles validation handshake (return `validationToken` as plain text)
- Parses change notification payloads
- Extracts message content and metadata
- Buffers as `InboundMessage` for `poll_messages()`

### Req-7: Daemon Wiring

Register `TeamsChannel` in the daemon's channel map. Spawn as a tokio task. Inject into agent
loop trigger channel via mpsc. The webhook endpoint is added to the existing axum router.

## Scope
- **IN**: OAuth2 client credentials, subscription webhooks, REST send, Channel trait impl, config, axum webhook route, daemon wiring
- **OUT**: Delegated permissions (user-level auth), adaptive cards (rich formatting), file attachments, meeting integration, presence/status

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/teams/mod.rs` | New module: TeamsChannel implementing Channel trait |
| `crates/nv-daemon/src/teams/oauth.rs` | New: OAuth2 client credentials flow + token refresh |
| `crates/nv-daemon/src/teams/client.rs` | New: MS Graph REST client for sending messages |
| `crates/nv-daemon/src/teams/webhook.rs` | New: subscription management + notification parsing |
| `crates/nv-core/src/config.rs` | Add TeamsConfig struct, add `teams: Option<TeamsConfig>` to Config |
| `crates/nv-core/src/config.rs` | Add `ms_graph_client_id`, `ms_graph_client_secret` to Secrets |
| `crates/nv-daemon/src/http.rs` | Add `/webhooks/teams` route for subscription notifications |
| `crates/nv-daemon/src/main.rs` | Conditionally spawn TeamsChannel, register in channel map |

## Risks
| Risk | Mitigation |
|------|-----------|
| Subscription expiry (60 min max for channel messages) | Auto-renew timer with buffer; re-register on 404 |
| OAuth token refresh race condition | Mutex around token state; refresh 5 min before expiry |
| Webhook endpoint must be publicly reachable (HTTPS) | Tailscale + reverse proxy; or use ngrok for dev |
| MS Graph API changes / deprecations | Pin Graph API version (v1.0); defensive serde parsing |
| Shared OAuth with future email-channel spec | Design token cache as reusable MsGraphAuth struct from the start |
