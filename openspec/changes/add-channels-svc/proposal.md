# Proposal: Add Channels Service

## Change ID
`add-channels-svc`

## Summary

Build the channels service (`packages/tools/channels-svc/`) -- a Hono microservice on port 4003
that provides an adapter registry for dispatching messages across platforms (Telegram, Discord,
Teams, email, iMessage). Exposes `list_channels()` and `send_to_channel()` as both HTTP routes
and MCP tools (stdio).

## Context
- Depends on: `scaffold-tool-service` (v10 tool fleet pattern -- no physical template exists yet,
  so this spec defines the full service structure following the daemon's existing Hono patterns)
- Stack: Hono, pino, TypeScript, `@hono/node-server`
- Port: 4003
- Phase: 2 (Core Tools), Wave 3
- Related code:
  - `crates/nv-daemon/src/tools/channels.rs` -- existing Rust implementation of `list_channels` and
    `send_to_channel` with the same adapter registry pattern
  - `crates/nv-daemon/src/channels/` -- Rust channel adapters (telegram, discord, teams, email,
    imessage) implementing `nv_core::Channel` trait
  - `packages/daemon/src/channels/telegram.ts` -- legacy TypeScript `TelegramAdapter` class
  - `packages/daemon/src/types.ts` -- `Channel` type union (`"telegram" | "teams" | "discord" |
    "email" | "imessage"`)

## Motivation

The v10 tool fleet architecture decomposes Nova's monolithic tool surface into independent Hono
microservices. The channels service is the cross-channel routing layer that the daemon (and other
services) call to discover available messaging channels and dispatch outbound messages. Currently
this logic lives in `crates/nv-daemon/src/tools/channels.rs` as Rust code embedded in the daemon
-- extracting it into a standalone TypeScript service enables independent scaling, fault isolation,
and reuse by the dashboard and other tools.

## Requirements

### Req-1: Service Scaffold

Create `packages/tools/channels-svc/` following the daemon's Hono patterns:

```
packages/tools/channels-svc/
  src/
    index.ts          # Entry point: Hono app + MCP stdio server
    server.ts         # Hono app definition with routes
    adapters/
      registry.ts     # ChannelAdapter interface + AdapterRegistry class
      telegram.ts     # Telegram adapter (calls Telegram Bot API)
      discord.ts      # Discord adapter (stub -- calls discord-cli or Bot API)
      teams.ts        # Teams adapter (stub -- calls MS Graph)
      email.ts        # Email adapter (stub -- calls Resend API)
      imessage.ts     # iMessage adapter (stub -- not available on Linux)
    mcp.ts            # MCP stdio JSON-RPC handler
    types.ts          # Shared types (ChannelStatus, SendRequest, etc.)
    logger.ts         # pino logger instance
  tests/
    registry.test.ts  # Adapter registry unit tests
    server.test.ts    # HTTP route tests
  package.json
  tsconfig.json
```

- Entry point starts both the Hono HTTP server on `:4003` and an MCP stdio server (when
  stdin is a TTY or `--mcp` flag is passed)
- `package.json`: `@nova/channels-svc`, private, type: module

### Req-2: Adapter Registry

Define the `ChannelAdapter` interface and `AdapterRegistry`:

```typescript
export type ChannelName = "telegram" | "discord" | "teams" | "email" | "imessage";

export type ChannelStatus = "connected" | "disconnected" | "error";

export type ChannelDirection = "inbound" | "outbound" | "bidirectional";

export interface ChannelAdapter {
  readonly name: ChannelName;
  readonly direction: ChannelDirection;
  status(): ChannelStatus;
  send(target: string, message: string): Promise<void>;
}

export class AdapterRegistry {
  private adapters = new Map<ChannelName, ChannelAdapter>();

  register(adapter: ChannelAdapter): void;
  get(name: ChannelName): ChannelAdapter | undefined;
  list(): Array<{ name: ChannelName; status: ChannelStatus; direction: ChannelDirection }>;
  has(name: ChannelName): boolean;
}
```

- `register()` adds an adapter by name (overwrites if already present)
- `list()` returns all registered adapters with current status
- `get()` returns a specific adapter or undefined
- Direction is a static property per adapter type (matches the Rust `channel_direction()` table):
  telegram=bidirectional, discord=bidirectional, teams=bidirectional, email=outbound,
  imessage=bidirectional

### Req-3: Telegram Adapter

Implement the Telegram adapter as the first real (non-stub) adapter:

- Uses Telegram Bot API via `fetch()` (no `node-telegram-bot-api` dependency -- keep the service
  lightweight)
- Constructor accepts `botToken: string` from `TELEGRAM_BOT_TOKEN` env var
- `send(chatId, message)` calls `POST https://api.telegram.org/bot{token}/sendMessage` with
  `{ chat_id, text, parse_mode: undefined }` (plain text to avoid HTML parse errors -- matches
  the fix from commit 5d5de07)
- `status()` returns `"connected"` if the bot token is set, `"disconnected"` if not
- Does not do long-polling or receive messages -- this is outbound-only for the service; inbound
  message handling stays in nv-daemon's channel poll loop

### Req-4: Stub Adapters

Implement stub adapters for discord, teams, email, and imessage:

- Each returns `"disconnected"` from `status()` by default
- Each throws a descriptive error from `send()`: `"Discord adapter not yet implemented"`
- Direction set correctly per the Rust table
- Stubs are placeholders -- real implementations come in future specs

### Req-5: HTTP Routes

Three HTTP routes on the Hono app:

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Returns `{ status: "ok", service: "channels-svc", port: 4003 }` |
| `GET` | `/channels` | Returns `{ channels: [...] }` with name, status, direction per adapter |
| `POST` | `/send` | Accepts `{ channel, target, message }`, dispatches via adapter registry |

`POST /send` validation:
1. Channel must be registered -- 404 if not found
2. Channel direction must support outbound -- 400 if inbound-only
3. Channel status must be `"connected"` -- 503 if disconnected/error
4. On success, returns `{ ok: true, channel, target }`
5. On adapter error, returns `{ ok: false, error: "..." }` with 502

### Req-6: MCP Tool Definitions

Two MCP tools matching the Rust implementation:

```
list_channels()
  description: "List available messaging channels and their connection status"
  input_schema: { type: "object", properties: {}, required: [] }

send_to_channel(channel, target, message)
  description: "Send a message to a specific channel"
  input_schema: {
    type: "object",
    properties: {
      channel: { type: "string", description: "Channel name" },
      target: { type: "string", description: "Target identifier (chat ID, channel ID, email address)" },
      message: { type: "string", description: "Message body" }
    },
    required: ["channel", "target", "message"]
  }
```

The MCP server reads from stdin (JSON-RPC newline-delimited), dispatches to the same adapter
registry, and writes responses to stdout. Supports `initialize`, `tools/list`, and `tools/call`.

### Req-7: Logging

Use `pino` for structured logging (matches `packages/daemon/src/logger.ts` pattern):

```typescript
import pino from "pino";
export const logger = pino({ name: "channels-svc" });
```

Log on startup: service name, port, registered adapters with status.
Log on each `/send` call: channel, target (first 20 chars), success/failure.

## Scope
- **IN**: Service scaffold, adapter registry, Telegram adapter (send-only), stub adapters (discord,
  teams, email, imessage), HTTP routes, MCP stdio handler, pino logging, unit tests
- **OUT**: Inbound message polling/receiving (stays in nv-daemon), confirmation keyboards (handled
  by daemon), real Discord/Teams/email/iMessage adapter implementations, systemd service file,
  Postgres integration, dashboard integration

## Impact

| Area | Change |
|------|--------|
| `packages/tools/channels-svc/` | New package -- full service implementation |
| `pnpm-workspace.yaml` | Already includes `packages/*` -- no change needed |
| `docker-compose.yml` | Not modified -- service runs standalone for now |

## Risks

| Risk | Mitigation |
|------|-----------|
| Telegram Bot API rate limits on send | Single-request per `/send` call; no batching needed at this scale |
| MCP stdio conflicts with Hono HTTP server | Dual-mode: HTTP always starts; MCP stdio only activates when `--mcp` flag or stdin is piped |
| Stub adapters returning errors confuse Claude | `list_channels` returns `status: "disconnected"` for stubs; Claude sees status before attempting send |
| Port 4003 conflict | Configurable via `PORT` env var with 4003 default |
