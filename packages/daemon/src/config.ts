import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import * as TOML from "@iarna/toml";
import {
  type ProactiveWatcherConfig,
  defaultProactiveWatcherConfig,
} from "./features/watcher/types.js";
import type { DreamSchedulerConfig } from "./features/dream/types.js";
import type { QueueConfig } from "./queue/index.js";
import { resolveConfig, type ConfigWithSources } from "./config/resolver.js";
import {
  type FleetHealthMonitorConfig,
  defaultFleetHealthMonitorConfig,
} from "./features/fleet-health/types.js";

export type { ProactiveWatcherConfig, DreamSchedulerConfig, QueueConfig };
export type { ConfigWithSources };
export type { FleetHealthMonitorConfig };

export interface DigestConfig {
  enabled: boolean;
  quietStart: string;
  quietEnd: string;
  tier1Hours: number[];
  tier2Day: number;
  tier2Hour: number;
  realtimeIntervalMs: number;
  p0CooldownMs: number;
  p1CooldownMs: number;
  p2CooldownMs: number;
  hashTtlMs: number;
}

export const defaultDigestConfig: DigestConfig = {
  enabled: true,
  quietStart: "22:00",
  quietEnd: "07:00",
  tier1Hours: [7, 12, 17],
  tier2Day: 1,
  tier2Hour: 9,
  realtimeIntervalMs: 300_000,
  p0CooldownMs: 1_800_000,
  p1CooldownMs: 14_400_000,
  p2CooldownMs: 43_200_000,
  hashTtlMs: 172_800_000,
};

export interface AutonomyConfig {
  enabled: boolean;
  timeoutMs: number;
  cooldownHours: number;
  idleDebounceMs: number;
  pollIntervalMs: number;
  dailyBudgetUsd: number;
  autonomyBudgetPct: number;
  maxAttempts: number;
}

export interface McpServerEntry {
  command: string;
  args: string[];
  env?: Record<string, string>;
}

export interface AgentConfig {
  model: string;
  maxTurns: number;
}

export interface Config {
  logLevel: string;
  daemonPort: number;
  configPath: string;
  apiToken: string;
  vercelGatewayKey?: string;
  databaseUrl: string;
  systemPromptPath: string;
  telegramChatId?: string;
  toolRouterUrl: string;
  /** Token for authenticating WebSocket /ws/events connections from the dashboard. */
  dashboardToken?: string;
  mcpServers: Record<string, McpServerEntry>;
  agent: AgentConfig;
  autonomy?: AutonomyConfig;
  proactiveWatcher: ProactiveWatcherConfig;
  dream: DreamSchedulerConfig;
  digest: DigestConfig;
  queue: QueueConfig;
  conversationHistoryDepth: number;
  fleetHealthMonitor: FleetHealthMonitorConfig;
}

const DEFAULT_CONFIG_PATH = join(homedir(), ".nv", "config", "nv.toml");

interface SupplementalToml {
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
  digest?: {
    tier2_day?: number;
    tier2_hour?: number;
    realtime_interval_ms?: number;
  };
  autonomy?: {
    idle_debounce_ms?: number;
    poll_interval_ms?: number;
    autonomy_budget_pct?: number;
    max_attempts?: number;
  };
  tools?: {
    mcp_servers?: Record<
      string,
      { command: string; args: string[]; env?: Record<string, string> }
    >;
  };
  conversation?: {
    history_depth?: number;
  };
  fleet_health_monitor?: {
    enabled?: boolean;
    interval_ms?: number;
    probe_timeout_ms?: number;
    notify_on_critical?: boolean;
  };
}

/**
 * Load and validate configuration from all sources.
 *
 * Delegates to the Zod schema resolver for core sections (daemon, agent,
 * digest, queue, database). Supplemental fields (proactiveWatcher, dream,
 * mcpServers, etc.) are loaded from TOML with defaults.
 *
 * Throws a descriptive error if any required config is invalid.
 */
export async function loadConfig(
  configPath: string = DEFAULT_CONFIG_PATH,
): Promise<Config> {
  // Delegate to schema-validated resolver — throws on invalid config
  const { config: validated } = await resolveConfig(configPath);

  // Load raw TOML for supplemental fields not covered by the Zod schema
  let toml: SupplementalToml = {};
  try {
    const raw = await readFile(configPath, "utf-8");
    toml = TOML.parse(raw) as SupplementalToml;
  } catch (err: unknown) {
    const isNotFound =
      err instanceof Error &&
      "code" in err &&
      (err as NodeJS.ErrnoException).code === "ENOENT";
    if (!isNotFound) throw err;
    // File not found — use defaults for all supplemental fields
  }

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

  // Digest: merge Zod-validated fields with supplemental TOML fields
  const digest: DigestConfig = {
    enabled: validated.digest.enabled,
    quietStart: validated.digest.quietStart,
    quietEnd: validated.digest.quietEnd,
    tier1Hours: validated.digest.tier1Hours,
    tier2Day: toml.digest?.tier2_day ?? defaultDigestConfig.tier2Day,
    tier2Hour: toml.digest?.tier2_hour ?? defaultDigestConfig.tier2Hour,
    realtimeIntervalMs:
      toml.digest?.realtime_interval_ms ?? defaultDigestConfig.realtimeIntervalMs,
    p0CooldownMs: validated.digest.cooldowns.p0Ms,
    p1CooldownMs: validated.digest.cooldowns.p1Ms,
    p2CooldownMs: validated.digest.cooldowns.p2Ms,
    hashTtlMs: validated.digest.cooldowns.hashTtlMs,
  };

  // Autonomy: merge Zod-validated fields with supplemental TOML fields
  const autonomy: AutonomyConfig | undefined = validated.autonomy
    ? {
        enabled: validated.autonomy.enabled,
        timeoutMs: validated.autonomy.timeoutMs,
        cooldownHours: validated.autonomy.cooldownHours,
        dailyBudgetUsd: validated.autonomy.dailyBudgetUsd,
        idleDebounceMs: toml.autonomy?.idle_debounce_ms ?? 60_000,
        pollIntervalMs: toml.autonomy?.poll_interval_ms ?? 30_000,
        autonomyBudgetPct: toml.autonomy?.autonomy_budget_pct ?? 0.20,
        maxAttempts: toml.autonomy?.max_attempts ?? 3,
      }
    : undefined;

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

  const historyDepthRaw = process.env["NV_HISTORY_DEPTH"];
  const conversationHistoryDepth = historyDepthRaw
    ? parseInt(historyDepthRaw, 10)
    : (toml.conversation?.history_depth ?? 20);

  const fleetHealthMonitor: FleetHealthMonitorConfig = {
    enabled:
      toml.fleet_health_monitor?.enabled ??
      defaultFleetHealthMonitorConfig.enabled,
    intervalMs:
      toml.fleet_health_monitor?.interval_ms ??
      defaultFleetHealthMonitorConfig.intervalMs,
    probeTimeoutMs:
      toml.fleet_health_monitor?.probe_timeout_ms ??
      defaultFleetHealthMonitorConfig.probeTimeoutMs,
    notifyOnCritical:
      toml.fleet_health_monitor?.notify_on_critical ??
      defaultFleetHealthMonitorConfig.notifyOnCritical,
  };

  const apiToken = process.env["NV_API_TOKEN"];
  if (!apiToken) {
    throw new Error("NV_API_TOKEN environment variable is required but not set.");
  }

  return {
    logLevel: validated.daemon.logLevel,
    daemonPort: validated.daemon.port,
    configPath,
    apiToken,
    databaseUrl: validated.database.url,
    vercelGatewayKey: process.env["VERCEL_GATEWAY_KEY"],
    dashboardToken: process.env["DASHBOARD_TOKEN"],
    telegramChatId: validated.telegram?.chatId,
    systemPromptPath: validated.agent.systemPromptPath,
    toolRouterUrl: validated.daemon.toolRouterUrl,
    mcpServers,
    agent: {
      model: validated.agent.model,
      maxTurns: validated.agent.maxTurns,
    },
    autonomy,
    proactiveWatcher,
    dream,
    digest,
    queue: {
      concurrency: validated.queue.concurrency,
      maxQueueSize: validated.queue.maxQueueSize,
    },
    conversationHistoryDepth,
    fleetHealthMonitor,
  };
}
