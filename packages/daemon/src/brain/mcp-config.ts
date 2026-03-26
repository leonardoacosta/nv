import { homedir } from "node:os";
import { resolve, isAbsolute } from "node:path";
import type { Config } from "../config.js";
import { logger } from "../logger.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface McpStdioServerConfig {
  command: string;
  args: string[];
  env?: Record<string, string>;
}

// ─── buildMcpServers ──────────────────────────────────────────────────────────

/**
 * Converts `Config.mcpServers` into the `Record<string, McpStdioServerConfig>`
 * format expected by the Agent SDK's `query()` `mcpServers` option.
 *
 * - Resolves relative paths in `args[0]` against `NOVA_INSTALL_DIR`
 *   (default `~/.local/lib/nova-ts/`).
 * - Passes through any per-server `env` overrides.
 */
export function buildMcpServers(
  config: Config,
): Record<string, McpStdioServerConfig> {
  const entries = config.mcpServers;
  if (!entries || Object.keys(entries).length === 0) {
    return {};
  }

  const installDir =
    process.env["NOVA_INSTALL_DIR"] ??
    resolve(homedir(), ".local", "lib", "nova-ts");

  const result: Record<string, McpStdioServerConfig> = {};

  for (const [name, entry] of Object.entries(entries)) {
    const args = [...entry.args];

    // Resolve relative path in first arg against install directory
    if (args.length > 0 && args[0] && !isAbsolute(args[0])) {
      args[0] = resolve(installDir, args[0]);
    }

    result[name] = {
      command: entry.command,
      args,
      ...(entry.env ? { env: entry.env } : {}),
    };

    logger.debug(
      { name, command: entry.command, args },
      "Registered MCP server",
    );
  }

  return result;
}

// ─── buildAllowedTools ────────────────────────────────────────────────────────

/**
 * Builds the `allowedTools` array for `query()` by combining built-in tools
 * with `mcp__<server-name>__*` wildcard patterns for each registered MCP server.
 */
export function buildAllowedTools(
  mcpServers: Record<string, McpStdioServerConfig>,
  builtinTools: string[],
): string[] {
  const mcpWildcards = Object.keys(mcpServers).map(
    (name) => `mcp__${name}__*`,
  );

  return [...builtinTools, ...mcpWildcards];
}
