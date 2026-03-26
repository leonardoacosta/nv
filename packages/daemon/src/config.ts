import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import * as TOML from "@iarna/toml";
import "dotenv/config";

export interface Config {
  logLevel: string;
  daemonPort: number;
  configPath: string;
  vercelGatewayKey?: string;
  databaseUrl: string;
  systemPromptPath: string;
}

const DEFAULT_CONFIG_PATH = join(homedir(), ".nv", "config", "nv.toml");

const DEFAULTS: Omit<Config, "configPath" | "databaseUrl"> = {
  logLevel: "info",
  daemonPort: 7700,
  systemPromptPath: "config/system-prompt.md",
};

interface TomlConfig {
  daemon?: {
    port?: number;
    log_level?: string;
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
    : (toml.daemon?.port ?? DEFAULTS.daemonPort);

  const databaseUrl = process.env["DATABASE_URL"];
  if (!databaseUrl) {
    throw new Error(
      "DATABASE_URL environment variable is required but not set. " +
        "Set it to a valid PostgreSQL connection string.",
    );
  }

  const vercelGatewayKey = process.env["VERCEL_GATEWAY_KEY"];

  const systemPromptPath =
    process.env["NV_SYSTEM_PROMPT_PATH"] ?? DEFAULTS.systemPromptPath;

  return {
    logLevel,
    daemonPort,
    configPath,
    databaseUrl,
    vercelGatewayKey,
    systemPromptPath,
  };
}
