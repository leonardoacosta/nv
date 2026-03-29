import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import * as TOML from "@iarna/toml";
import { ZodError } from "zod";
import { configSchema, type ValidatedConfig } from "./schema.js";

export type ConfigSource = "default" | "toml" | "db" | "env";

export interface ConfigWithSources {
  config: ValidatedConfig;
  /** Map of dot-path key -> source that provided the final value */
  sources: Record<string, ConfigSource>;
}

const DEFAULT_CONFIG_PATH = join(homedir(), ".nv", "config", "nv.toml");

interface RawToml {
  daemon?: {
    port?: number | string;
    health_port?: number;
    log_level?: string;
    tool_router_url?: string;
  };
  agent?: {
    model?: string;
    max_turns?: number | string;
    system_prompt_path?: string;
  };
  telegram?: {
    chat_id?: number | string;
    bot_token?: string;
  };
  discord?: {
    bot_token?: string;
  };
  teams?: {
    webhook_url?: string;
  };
  digest?: {
    enabled?: boolean;
    quiet_start?: string;
    quiet_end?: string;
    tier1_hours?: number[];
    p0_cooldown_ms?: number;
    p1_cooldown_ms?: number;
    p2_cooldown_ms?: number;
    hash_ttl_ms?: number;
  };
  autonomy?: {
    enabled?: boolean;
    timeout_ms?: number;
    cooldown_hours?: number;
    daily_budget_usd?: number;
  };
  queue?: {
    concurrency?: number;
    max_queue_size?: number;
  };
}

/**
 * Load raw TOML config from disk. Returns empty object if file not found.
 * Throws on parse errors or unexpected file-system errors.
 */
async function loadToml(configPath: string): Promise<RawToml> {
  try {
    const raw = await readFile(configPath, "utf-8");
    return TOML.parse(raw) as RawToml;
  } catch (err: unknown) {
    const isNotFound =
      err instanceof Error &&
      "code" in err &&
      (err as NodeJS.ErrnoException).code === "ENOENT";
    if (isNotFound) return {};
    throw err;
  }
}

/**
 * Build merged raw config input from all sources in precedence order:
 * defaults -> TOML -> DB -> env vars.
 * Also tracks which source provided each value.
 */
function buildMergedInput(
  toml: RawToml,
  sources: Record<string, ConfigSource>,
): Record<string, unknown> {
  function track(key: string, source: ConfigSource): void {
    sources[key] = source;
  }

  // ── daemon ───────────────────────────────────────────────────────────────
  const daemonPort =
    process.env["NV_DAEMON_PORT"] !== undefined
      ? (track("daemon.port", "env"), parseInt(process.env["NV_DAEMON_PORT"]!, 10))
      : toml.daemon?.port !== undefined
        ? (track("daemon.port", "toml"), toml.daemon.port)
        : (track("daemon.port", "default"), 7700);

  const daemonLogLevel =
    process.env["NV_LOG_LEVEL"] !== undefined
      ? (track("daemon.logLevel", "env"), process.env["NV_LOG_LEVEL"])
      : toml.daemon?.log_level !== undefined
        ? (track("daemon.logLevel", "toml"), toml.daemon.log_level)
        : (track("daemon.logLevel", "default"), "info");

  const daemonToolRouterUrl =
    process.env["TOOL_ROUTER_URL"] !== undefined
      ? (track("daemon.toolRouterUrl", "env"), process.env["TOOL_ROUTER_URL"])
      : toml.daemon?.tool_router_url !== undefined
        ? (track("daemon.toolRouterUrl", "toml"), toml.daemon.tool_router_url)
        : (track("daemon.toolRouterUrl", "default"), "http://localhost:4100");

  // ── agent ────────────────────────────────────────────────────────────────
  const agentModel =
    toml.agent?.model !== undefined
      ? (track("agent.model", "toml"), toml.agent.model)
      : (track("agent.model", "default"), "claude-opus-4-6");

  const agentMaxTurns =
    toml.agent?.max_turns !== undefined
      ? (track("agent.maxTurns", "toml"), toml.agent.max_turns)
      : (track("agent.maxTurns", "default"), 100);

  const systemPromptPath =
    process.env["NV_SYSTEM_PROMPT_PATH"] !== undefined
      ? (track("agent.systemPromptPath", "env"), process.env["NV_SYSTEM_PROMPT_PATH"])
      : toml.agent?.system_prompt_path !== undefined
        ? (track("agent.systemPromptPath", "toml"), toml.agent.system_prompt_path)
        : (track("agent.systemPromptPath", "default"), "config/system-prompt.md");

  // ── telegram ─────────────────────────────────────────────────────────────
  const telegramBotToken = process.env["TELEGRAM_BOT_TOKEN"];
  const telegramChatId =
    process.env["TELEGRAM_CHAT_ID"] !== undefined
      ? process.env["TELEGRAM_CHAT_ID"]
      : toml.telegram?.chat_id !== undefined
        ? String(toml.telegram.chat_id)
        : undefined;

  const telegramSection =
    telegramBotToken !== undefined
      ? (track("telegram", "env"), { botToken: telegramBotToken, chatId: telegramChatId })
      : toml.telegram?.bot_token !== undefined
        ? (track("telegram", "toml"), {
            botToken: toml.telegram.bot_token,
            chatId: telegramChatId,
          })
        : undefined;

  // ── discord ──────────────────────────────────────────────────────────────
  const discordBotToken =
    process.env["DISCORD_BOT_TOKEN"] ?? toml.discord?.bot_token;
  const discordSection =
    discordBotToken !== undefined
      ? (track("discord", process.env["DISCORD_BOT_TOKEN"] ? "env" : "toml"),
         { botToken: discordBotToken })
      : undefined;

  // ── teams ────────────────────────────────────────────────────────────────
  const teamsWebhook =
    process.env["TEAMS_WEBHOOK_URL"] ?? toml.teams?.webhook_url;
  const teamsSection =
    teamsWebhook !== undefined
      ? (track("teams", process.env["TEAMS_WEBHOOK_URL"] ? "env" : "toml"),
         { webhookUrl: teamsWebhook })
      : undefined;

  // ── digest ───────────────────────────────────────────────────────────────
  track("digest", toml.digest ? "toml" : "default");

  // ── autonomy ─────────────────────────────────────────────────────────────
  const autonomySection = toml.autonomy
    ? (track("autonomy", "toml"), {
        enabled: toml.autonomy.enabled ?? true,
        timeoutMs: toml.autonomy.timeout_ms ?? 300_000,
        cooldownHours: toml.autonomy.cooldown_hours ?? 2,
        dailyBudgetUsd: toml.autonomy.daily_budget_usd ?? 5.0,
      })
    : undefined;

  // ── queue ────────────────────────────────────────────────────────────────
  const queueConcurrency =
    process.env["NV_QUEUE_CONCURRENCY"] !== undefined
      ? (track("queue.concurrency", "env"), parseInt(process.env["NV_QUEUE_CONCURRENCY"]!, 10))
      : toml.queue?.concurrency !== undefined
        ? (track("queue.concurrency", "toml"), toml.queue.concurrency)
        : (track("queue.concurrency", "default"), 2);

  const queueMaxSize =
    process.env["NV_QUEUE_MAX_SIZE"] !== undefined
      ? (track("queue.maxQueueSize", "env"), parseInt(process.env["NV_QUEUE_MAX_SIZE"]!, 10))
      : toml.queue?.max_queue_size !== undefined
        ? (track("queue.maxQueueSize", "toml"), toml.queue.max_queue_size)
        : (track("queue.maxQueueSize", "default"), 20);

  // ── database ─────────────────────────────────────────────────────────────
  const databaseUrl = process.env["DATABASE_URL"];
  track("database.url", databaseUrl ? "env" : "default");

  return {
    daemon: {
      port: daemonPort,
      logLevel: daemonLogLevel,
      toolRouterUrl: daemonToolRouterUrl,
    },
    agent: {
      model: agentModel,
      maxTurns: agentMaxTurns,
      systemPromptPath,
    },
    telegram: telegramSection,
    discord: discordSection,
    teams: teamsSection,
    digest: {
      enabled: toml.digest?.enabled ?? true,
      quietStart: toml.digest?.quiet_start ?? "22:00",
      quietEnd: toml.digest?.quiet_end ?? "07:00",
      tier1Hours: toml.digest?.tier1_hours ?? [7, 12, 17],
      cooldowns: {
        p0Ms: toml.digest?.p0_cooldown_ms ?? 1_800_000,
        p1Ms: toml.digest?.p1_cooldown_ms ?? 14_400_000,
        p2Ms: toml.digest?.p2_cooldown_ms ?? 43_200_000,
        hashTtlMs: toml.digest?.hash_ttl_ms ?? 172_800_000,
      },
    },
    autonomy: autonomySection,
    queue: {
      concurrency: queueConcurrency,
      maxQueueSize: queueMaxSize,
    },
    database: {
      url: databaseUrl ?? "",
    },
  };
}

/**
 * Format a ZodError into a human-readable multi-line string.
 */
function formatZodError(err: ZodError): string {
  return err.errors
    .map((e) => `  - ${e.path.join(".")}: ${e.message}`)
    .join("\n");
}

/**
 * Load, merge, and validate configuration from all sources.
 * Precedence order (highest wins): env vars > DB > TOML > defaults
 *
 * Throws a descriptive error on validation failure.
 */
export async function resolveConfig(
  configPath: string = DEFAULT_CONFIG_PATH,
): Promise<ConfigWithSources> {
  const toml = await loadToml(configPath);

  const sources: Record<string, ConfigSource> = {};
  const rawInput = buildMergedInput(toml, sources);

  const result = configSchema.safeParse(rawInput);

  if (!result.success) {
    const formatted = formatZodError(result.error);
    throw new Error(
      `Configuration validation failed:\n${formatted}\n\nCheck ${configPath} and environment variables.`,
    );
  }

  return { config: result.data, sources };
}
