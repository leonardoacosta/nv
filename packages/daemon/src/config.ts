import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import * as TOML from "@iarna/toml";
import "dotenv/config";
import {
  type ProactiveWatcherConfig,
  defaultProactiveWatcherConfig,
} from "./features/watcher/types.js";
import type { DreamSchedulerConfig } from "./features/dream/types.js";

export type { ProactiveWatcherConfig, DreamSchedulerConfig };

export interface AutonomyConfig {
  enabled: boolean;
  timeoutMs: number;
  cooldownHours: number;
  idleDebounceMs: number;
  pollIntervalMs: number;
}

export interface McpServerEntry {
  command: string;
  args: string[];
  env?: Record<string, string>;
}

export interface Config {
  logLevel: string;
  daemonPort: number;
  configPath: string;
  vercelGatewayKey?: string;
  databaseUrl: string;
  systemPromptPath: string;
  telegramChatId?: string;
  toolRouterUrl: string;
  mcpServers: Record<string, McpServerEntry>;
  autonomy?: AutonomyConfig;
  proactiveWatcher: ProactiveWatcherConfig;
  dream: DreamSchedulerConfig;
  conversationHistoryDepth: number;
}

const DEFAULT_CONFIG_PATH = join(homedir(), ".nv", "config", "nv.toml");

const DEFAULTS: Omit<Config, "configPath" | "databaseUrl" | "autonomy" | "proactiveWatcher" | "dream" | "vercelGatewayKey"> = {
  logLevel: "info",
  daemonPort: 7700,
  systemPromptPath: "config/system-prompt.md",
  toolRouterUrl: "http://localhost:4100",
  mcpServers: {},
  conversationHistoryDepth: 20,
};

interface TomlConfig {
  daemon?: {
    port?: number;
    health_port?: number;
    log_level?: string;
    tool_router_url?: string;
  };
  telegram?: {
    chat_id?: number | string;
  };
  autonomy?: {
    enabled?: boolean;
    timeout_ms?: number;
    cooldown_hours?: number;
    idle_debounce_ms?: number;
    poll_interval_ms?: number;
  };
  proactive_watcher?: {
    enabled?: boolean;
    interval_minutes?: number;
    stale_threshold_hours?: number;
    approaching_deadline_hours?: number;
    max_reminders_per_interval?: number;
    quiet_start?: string;
    quiet_end?: string;
  };
  dream?: {
    enabled?: boolean;
    cron_hour?: number;
    interaction_threshold?: number;
    size_threshold_kb?: number;
    debounce_hours?: number;
    topic_max_kb?: number;
  };
  conversation?: {
    history_depth?: number;
  };
  tools?: {
    mcp_servers?: Record<
      string,
      { command: string; args: string[]; env?: Record<string, string> }
    >;
  };
}

export async function loadConfig(
  configPath: string = DEFAULT_CONFIG_PATH,
): Promise<Config> {
  let toml: TomlConfig = {};

  try {
    const raw = await readFile(configPath, "utf-8");
    toml = TOML.parse(raw) as TomlConfig;
  } catch (err: unknown) {
    // Fall back to defaults if file does not exist or cannot be parsed
    const isNotFound =
      err instanceof Error &&
      "code" in err &&
      (err as NodeJS.ErrnoException).code === "ENOENT";
    if (!isNotFound) {
      // Re-throw unexpected errors (permissions, parse errors, etc.)
      throw err;
    }
  }

  const logLevel =
    process.env["NV_LOG_LEVEL"] ??
    toml.daemon?.log_level ??
    DEFAULTS.logLevel;

  const daemonPortRaw = process.env["NV_DAEMON_PORT"];
  const daemonPort = daemonPortRaw
    ? parseInt(daemonPortRaw, 10)
    : (toml.daemon?.port ?? toml.daemon?.health_port ?? DEFAULTS.daemonPort);

  const databaseUrl = process.env["DATABASE_URL"];
  if (!databaseUrl) {
    throw new Error(
      "DATABASE_URL environment variable is required but not set. " +
        "Set it to a valid PostgreSQL connection string.",
    );
  }

  const vercelGatewayKey = process.env["VERCEL_GATEWAY_KEY"];

  const telegramChatIdRaw =
    process.env["TELEGRAM_CHAT_ID"] ??
    (toml.telegram?.chat_id !== undefined
      ? String(toml.telegram.chat_id)
      : undefined);
  const telegramChatId = telegramChatIdRaw;

  const systemPromptPath =
    process.env["NV_SYSTEM_PROMPT_PATH"] ?? DEFAULTS.systemPromptPath;

  const toolRouterUrl =
    process.env["TOOL_ROUTER_URL"] ??
    toml.daemon?.tool_router_url ??
    DEFAULTS.toolRouterUrl;

  const autonomy: AutonomyConfig = {
    enabled: toml.autonomy?.enabled ?? true,
    timeoutMs: toml.autonomy?.timeout_ms ?? 300_000,
    cooldownHours: toml.autonomy?.cooldown_hours ?? 2,
    idleDebounceMs: toml.autonomy?.idle_debounce_ms ?? 60_000,
    pollIntervalMs: toml.autonomy?.poll_interval_ms ?? 30_000,
  };

  const proactiveWatcher: ProactiveWatcherConfig = {
    enabled:
      toml.proactive_watcher?.enabled ??
      defaultProactiveWatcherConfig.enabled,
    intervalMinutes:
      toml.proactive_watcher?.interval_minutes ??
      defaultProactiveWatcherConfig.intervalMinutes,
    staleThresholdHours:
      toml.proactive_watcher?.stale_threshold_hours ??
      defaultProactiveWatcherConfig.staleThresholdHours,
    approachingDeadlineHours:
      toml.proactive_watcher?.approaching_deadline_hours ??
      defaultProactiveWatcherConfig.approachingDeadlineHours,
    maxRemindersPerInterval:
      toml.proactive_watcher?.max_reminders_per_interval ??
      defaultProactiveWatcherConfig.maxRemindersPerInterval,
    quietStart:
      toml.proactive_watcher?.quiet_start ??
      defaultProactiveWatcherConfig.quietStart,
    quietEnd:
      toml.proactive_watcher?.quiet_end ??
      defaultProactiveWatcherConfig.quietEnd,
  };

  const dream: DreamSchedulerConfig = {
    enabled: toml.dream?.enabled ?? true,
    cronHour: toml.dream?.cron_hour ?? 3,
    interactionThreshold: toml.dream?.interaction_threshold ?? 50,
    sizeThresholdKb: toml.dream?.size_threshold_kb ?? 60,
    debounceHours: toml.dream?.debounce_hours ?? 12,
    topicMaxKb: toml.dream?.topic_max_kb ?? 4,
  };

  const historyDepthRaw = process.env["NV_HISTORY_DEPTH"];
  const conversationHistoryDepth = historyDepthRaw
    ? parseInt(historyDepthRaw, 10)
    : (toml.conversation?.history_depth ?? 20);

  // Parse [tools.mcp_servers] section — each entry becomes an McpServerEntry
  const mcpServers: Record<string, McpServerEntry> = {};
  const tomlMcpServers = toml.tools?.mcp_servers;
  if (tomlMcpServers) {
    for (const [name, entry] of Object.entries(tomlMcpServers)) {
      mcpServers[name] = {
        command: entry.command,
        args: Array.isArray(entry.args) ? entry.args : [],
        ...(entry.env ? { env: entry.env } : {}),
      };
    }
  }

  return {
    logLevel,
    daemonPort,
    configPath,
    databaseUrl,
    vercelGatewayKey,
    telegramChatId,
    systemPromptPath,
    toolRouterUrl,
    mcpServers,
    autonomy,
    proactiveWatcher,
    dream,
    conversationHistoryDepth,
  };
}
