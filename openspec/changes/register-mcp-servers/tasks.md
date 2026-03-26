# Implementation Tasks

## Phase 1: Config and Shared MCP Builder

- [ ] [1.1] Add `McpServerEntry` type and `mcpServers` field to `Config` interface in `packages/daemon/src/config.ts`. Type: `Record<string, { command: string; args: string[]; env?: Record<string, string> }>`. Parse the `[tools.mcp_servers]` section from `nv.toml` TOML config. Resolve relative paths in `args[0]` against `NOVA_INSTALL_DIR` env var (default `~/.local/lib/nova-ts/`). Default to empty record if section is missing. [owner:api-engineer]

- [ ] [1.2] Add `[tools.mcp_servers]` section to `config/nv.toml` with entries for all 8 tool services (memory-svc, messages-svc, channels-svc, discord-svc, teams-svc, schedule-svc, graph-svc, meta-svc). Each entry: `{ command = "node", args = ["packages/tools/{name}/dist/index.js", "--mcp"] }`. Include inline comments explaining the path resolution and `--mcp` flag. [owner:api-engineer]

- [ ] [1.3] Create `packages/daemon/src/brain/mcp-config.ts` -- shared helper that converts `Config.mcpServers` into the `Record<string, McpStdioServerConfig>` format expected by the Agent SDK's `query()` options. Function signature: `buildMcpServers(config: Config): Record<string, McpStdioServerConfig>`. Also exports `buildAllowedTools(mcpServers: Record<string, McpStdioServerConfig>, builtinTools: string[]): string[]` which appends `mcp__<name>__*` wildcard for each registered server. [owner:api-engineer]

## Phase 2: Wire MCP into Agent Query Calls

- [ ] [2.1] Update `NovaAgent` in `packages/daemon/src/brain/agent.ts`: import `buildMcpServers` and `buildAllowedTools` from `mcp-config.ts`. In the constructor, build and store the MCP server config. In `processMessage()`, pass `mcpServers` and the expanded `allowedTools` list to the `query()` options. Keep existing built-in tools (`Read`, `Write`, `Bash`, `Glob`, `Grep`, `WebSearch`, `WebFetch`) alongside the MCP wildcards. [owner:api-engineer]

- [ ] [2.2] Update briefing synthesizer in `packages/daemon/src/features/briefing/synthesizer.ts`: find the `query()` call, add `mcpServers` from the shared config builder. The synthesizer needs access to the `Config` object -- thread it through from the caller or accept it as a constructor parameter. Add MCP tool wildcards to `allowedTools` (currently empty array -- expand it). [owner:api-engineer]

- [ ] [2.3] Update obligation executor in `packages/daemon/src/features/obligations/executor.ts`: find the `query()` call, add `mcpServers` from the shared config builder. Thread `Config` through from the caller. Add MCP tool wildcards to the existing `allowedTools` array. [owner:api-engineer]

## Phase 3: Claude Code MCP Registration

- [ ] [3.1] Create `scripts/register-mcp-servers.sh`: bash script that reads `~/.claude/mcp.json`, adds/updates entries for each Nova tool service as MCP stdio servers. Use `jq` for JSON manipulation. Resolve install path from `NOVA_INSTALL_DIR` or default `~/.local/lib/nova-ts/`. Define the 8 services as a bash array. For each: add `{ "command": "node", "args": ["{install_dir}/packages/tools/{name}/dist/index.js", "--mcp"] }`. Preserve all existing non-nova entries. Only remove `nv-tools` entry if `--remove-legacy` flag is passed. Print summary (added/updated/kept/removed). Exit 0 on success, 1 on error. Require `jq` with a clear error message if missing. Make executable (`chmod +x`). [owner:devops-engineer]

- [ ] [3.2] Update `.claude/settings.json`: add `mcp__nova-memory__*`, `mcp__nova-messages__*`, `mcp__nova-channels__*`, `mcp__nova-discord__*`, `mcp__nova-teams__*`, `mcp__nova-schedule__*`, `mcp__nova-graph__*`, `mcp__nova-meta__*` to the `permissions.allow` array. Keep existing `mcp__nv-tools__*` during transition. [owner:devops-engineer]

## Phase 4: Deploy Integration

- [ ] [4.1] Update `deploy/install-tools.sh` (from `add-fleet-deploy`): after the health check section, add a call to `bash "${REPO_DIR}/scripts/register-mcp-servers.sh"`. Capture exit code but don't fail the deploy if registration fails -- warn instead. Log the registration output. [owner:devops-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Config | `pnpm --filter @nova/daemon typecheck` passes. Config loads correctly with and without `[tools.mcp_servers]` section (empty defaults). |
| 2 Agent | `pnpm --filter @nova/daemon typecheck` passes. `NovaAgent.processMessage()` accepts MCP server config without runtime errors. Agent SDK receives `mcpServers` in options (verify via log output or debug breakpoint). |
| 3 Registration | `bash scripts/register-mcp-servers.sh` runs without error. `~/.claude/mcp.json` contains entries for all 8 services. Existing non-Nova entries are preserved. `jq` validation: `jq '.mcpServers["nova-memory"]' ~/.claude/mcp.json` returns the expected config. |
| 4 Deploy | `deploy/install-tools.sh` completes with registration step logged. |
| **Final** | Daemon starts with MCP servers configured. Agent can discover tools from at least one running service (e.g., `nova-memory`). `claude` CLI session in the `nv` project can access Nova tools via MCP. |
