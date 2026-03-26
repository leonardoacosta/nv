# Proposal: Register MCP Servers

## Change ID
`register-mcp-servers`

## Summary
Register all Nova tool fleet services as MCP servers so the daemon's Agent SDK `query()` call discovers tools natively via the MCP protocol, replacing the static `ALLOWED_TOOLS` list. Each tool service runs a dual-transport Hono+MCP server (per the scaffold template); this spec wires the MCP stdio transport into the daemon's agent configuration and provides a registration script for the user-level `~/.claude/mcp.json`.

## Context
- Phase: 5 -- Daemon Refactor | Wave: 6
- Depends on: `add-memory-svc`, `add-messages-svc`, `add-channels-svc` (services must exist to register)
- Depends on: `scaffold-tool-service` (provides the `--mcp` stdio transport in each service)
- Depends on: `slim-daemon` (daemon no longer has inline tools; needs MCP for tool access)
- Related: `add-fleet-deploy` (deploys services as systemd units with HTTP transport)
- Related: `add-tool-router` (HTTP-based dispatch alternative at :4000)
- Feature area: infrastructure
- Agent SDK: `@anthropic-ai/claude-agent-sdk@0.2.84` supports `mcpServers` option in `query()` -- accepts `Record<string, McpServerConfig>` where each entry can be `McpStdioServerConfig` (`{ command, args, env }`) or `McpHttpServerConfig` (`{ type: "http", url }`)

## Motivation
The daemon's `NovaAgent` currently passes a hardcoded `ALLOWED_TOOLS` list (`["Read", "Write", "Bash", "Glob", "Grep", "WebSearch", "WebFetch"]`) to the Agent SDK's `query()` call. This provides no access to the 47+ Nova-specific tools (memory, messages, channels, schedules, etc.) that are being ported from Rust to the TypeScript tool fleet.

The Agent SDK natively supports MCP server discovery via its `mcpServers` option. Each tool service already implements a stdio MCP transport (via the scaffold template's `--mcp` flag). By passing MCP server configs to `query()`, the agent automatically discovers and can invoke all tools exposed by the fleet -- no manual tool-name lists, no HTTP proxy layer needed.

This also enables registering the fleet services in `~/.claude/mcp.json` so interactive Claude Code sessions (outside the daemon) can access Nova's tools directly, replacing the current monolithic Rust `nv-tools` binary.

## Requirements

### Req-1: Daemon MCP server configuration

Update `NovaAgent` in `packages/daemon/src/brain/agent.ts` to pass `mcpServers` in the `query()` options. The MCP servers should be loaded from config rather than hardcoded.

Add a new config section `[tools.mcp_servers]` in `nv.toml` (and corresponding `Config` interface fields) that maps service names to their MCP launch commands:

```toml
[tools.mcp_servers]
nova-memory = { command = "node", args = ["packages/tools/memory-svc/dist/index.js", "--mcp"] }
nova-messages = { command = "node", args = ["packages/tools/messages-svc/dist/index.js", "--mcp"] }
nova-channels = { command = "node", args = ["packages/tools/channels-svc/dist/index.js", "--mcp"] }
nova-discord = { command = "node", args = ["packages/tools/discord-svc/dist/index.js", "--mcp"] }
nova-teams = { command = "node", args = ["packages/tools/teams-svc/dist/index.js", "--mcp"] }
nova-schedule = { command = "node", args = ["packages/tools/schedule-svc/dist/index.js", "--mcp"] }
nova-graph = { command = "node", args = ["packages/tools/graph-svc/dist/index.js", "--mcp"] }
nova-meta = { command = "node", args = ["packages/tools/meta-svc/dist/index.js", "--mcp"] }
```

At runtime, the daemon resolves relative paths against the install directory (`~/.local/lib/nova-ts/`). Each entry becomes a `McpStdioServerConfig` passed to the Agent SDK.

### Req-2: Agent SDK query integration

Modify the `query()` call in `NovaAgent.processMessage()` to include the loaded MCP servers:

```typescript
const queryStream = query({
  prompt: message.content,
  options: {
    systemPrompt: this.systemPrompt,
    allowedTools: [...BUILTIN_TOOLS, "mcp__nova-memory__*", "mcp__nova-messages__*", ...],
    permissionMode: "bypassPermissions",
    allowDangerouslySkipPermissions: true,
    maxTurns: 30,
    mcpServers: this.mcpServers,
    env: { ... },
  },
});
```

The `ALLOWED_TOOLS` list keeps the existing built-in tools (Read, Write, Bash, etc.) and adds wildcard patterns for each registered MCP server's tools (`mcp__<name>__*`). This ensures the agent can call both built-in tools and fleet tools without permission prompts.

### Req-3: MCP server config loading

Add to `packages/daemon/src/config.ts`:

- A new `McpServerEntry` type: `{ command: string; args: string[]; env?: Record<string, string> }`
- A new `mcpServers` field on the `Config` interface: `Record<string, McpServerEntry>`
- TOML parsing for the `[tools.mcp_servers]` section
- Path resolution: if `args[0]` is a relative path, resolve it against `NOVA_INSTALL_DIR` env var (default `~/.local/lib/nova-ts/`)
- Environment variable passthrough: each MCP server process inherits `DATABASE_URL`, `NODE_ENV`, and any service-specific env vars from Doppler

### Req-4: Registration script for Claude Code

Create `scripts/register-mcp-servers.sh` that updates `~/.claude/mcp.json` to include entries for each tool fleet service. The script:

1. Reads the existing `~/.claude/mcp.json` (preserving all non-Nova entries)
2. Adds/updates entries for each Nova tool service using the stdio transport:
   ```json
   {
     "nova-memory": {
       "command": "node",
       "args": ["/home/<user>/.local/lib/nova-ts/packages/tools/memory-svc/dist/index.js", "--mcp"]
     }
   }
   ```
3. Removes the legacy `nv-tools` entry (the monolithic Rust binary being replaced)
4. Writes the updated JSON back
5. Prints a summary of added/updated/removed entries

The script uses `jq` for JSON manipulation. It resolves the install path from `NOVA_INSTALL_DIR` or defaults to `~/.local/lib/nova-ts/`.

### Req-5: Update fleet deploy to run registration

Update `deploy/install-tools.sh` (from `add-fleet-deploy`) to call `scripts/register-mcp-servers.sh` after health checks pass. This ensures MCP registration happens automatically on every deploy.

### Req-6: Update project MCP permissions

Update `/home/nyaptor/dev/nv/.claude/settings.json` to add permission entries for each Nova MCP server. Currently it allows `mcp__nv-tools__*` (the monolithic Rust binary). Add wildcard allow entries for each fleet service:

```json
"allow": [
  "mcp__nova-memory__*",
  "mcp__nova-messages__*",
  "mcp__nova-channels__*",
  "mcp__nova-discord__*",
  "mcp__nova-teams__*",
  "mcp__nova-schedule__*",
  "mcp__nova-graph__*",
  "mcp__nova-meta__*"
]
```

Keep the existing `mcp__nv-tools__*` entry until the Rust binary is fully deprecated.

### Req-7: Briefing and obligation executor MCP passthrough

The briefing synthesizer (`features/briefing/synthesizer.ts`) and obligation executor (`features/obligations/executor.ts`) also call `query()`. Update both to pass the same `mcpServers` config so they can access fleet tools too. Extract the MCP server config construction into a shared helper (e.g., `brain/mcp-config.ts`) to avoid duplication across the three `query()` call sites.

## Scope
- **IN**: Daemon agent MCP config, config loading, `mcp.json` registration script, settings.json permissions, briefing/obligation executor updates, deploy integration
- **OUT**: Tool service implementation (per-service specs), MCP SDK stdio server implementation (scaffold-tool-service), tool-router HTTP dispatch (add-tool-router), Traefik routing, Claude Code plugin system

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/brain/agent.ts` | Add `mcpServers` to `query()` options, update `ALLOWED_TOOLS` |
| `packages/daemon/src/brain/mcp-config.ts` | New -- shared MCP config builder |
| `packages/daemon/src/config.ts` | Add `mcpServers` field, TOML parsing for `[tools.mcp_servers]` |
| `packages/daemon/src/features/briefing/synthesizer.ts` | Pass `mcpServers` to `query()` |
| `packages/daemon/src/features/obligations/executor.ts` | Pass `mcpServers` to `query()` |
| `scripts/register-mcp-servers.sh` | New -- updates `~/.claude/mcp.json` |
| `.claude/settings.json` | Add MCP permission wildcards for fleet services |
| `config/nv.toml` | Add `[tools.mcp_servers]` section |

## Risks
| Risk | Mitigation |
|------|-----------|
| Tool services not yet deployed when daemon starts | Agent SDK handles MCP server launch failures gracefully -- tools from unavailable servers are simply not registered. Daemon logs a warning per failed server. |
| MCP stdio server startup time adds latency | Agent SDK spawns MCP servers once per session, not per tool call. First message may be slower (server boot); subsequent calls reuse the running process. |
| Too many MCP servers overwhelm agent context | Start with the 3 core services (memory, messages, channels); add remaining services progressively. Config-driven -- operators can enable/disable per service. |
| `jq` not installed for registration script | Script checks for `jq` at start, exits with install instructions if missing. Alternatively, use Node.js one-liner as fallback. |
| MCP server process env missing secrets | Each MCP server process must inherit DATABASE_URL and service-specific env vars. Config supports per-server `env` overrides; deploy script ensures Doppler injects secrets. |
| Legacy `nv-tools` removal breaks existing sessions | Registration script only removes `nv-tools` if `--remove-legacy` flag is passed. Default is to keep both during transition. |

## Dependencies
- `scaffold-tool-service` -- provides the `--mcp` flag and stdio MCP server in each service
- `add-memory-svc`, `add-messages-svc`, `add-channels-svc` -- first services to register
- `slim-daemon` -- removes inline tool handling, making MCP the primary tool access path
- `add-fleet-deploy` -- provides `install-tools.sh` that this spec hooks into
