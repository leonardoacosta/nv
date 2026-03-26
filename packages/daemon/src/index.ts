import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { loadConfig } from "./config.js";
import { createLogger } from "./logger.js";
import { startApiServer } from "./api/server.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

function readVersion(): string {
  try {
    const pkgPath = join(__dirname, "..", "package.json");
    const pkg = JSON.parse(readFileSync(pkgPath, "utf-8")) as {
      version: string;
    };
    return pkg.version;
  } catch {
    return "unknown";
  }
}

export async function main(): Promise<void> {
  const config = await loadConfig();
  const log = createLogger("nova-daemon");

  const version = readVersion();

  log.info(
    {
      service: "nova-daemon",
      version,
      configPath: config.configPath,
      daemonPort: config.daemonPort,
    },
    "Nova daemon starting",
  );

  // TODO(channels): Wire up channel listeners (telegram, teams, discord, email, imessage)

  // TODO(agent-loop): Initialize Anthropic Agent SDK and start the agent loop

  const apiPort = Number(process.env["API_PORT"] ?? 3443);
  await startApiServer(apiPort);
  log.info({ service: "nova-daemon", port: apiPort }, `API server listening on :${apiPort}`);

  log.info({ service: "nova-daemon" }, "Nova daemon ready");
}

main().catch((err: unknown) => {
  console.error("Fatal error during startup:", err);
  process.exit(1);
});
