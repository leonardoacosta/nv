/**
 * Graph Token Refresh — keeps O365 and BBAdmin tokens alive via cron.
 *
 * Runs every 30 minutes. SSHs to CloudPC and calls graph-token-refresh.ps1
 * which checks expiry and refreshes if needed (~0 cost, no LLM).
 *
 * If the refresh token itself expires (90 days), logs a warning —
 * manual re-auth is needed via `ssh cloudpc "powershell -File graph-pim.ps1 -Action Auth"`.
 */

import { createLogger } from "../logger.js";

const log = createLogger("token-refresh");

const INTERVAL_MS = 30 * 60 * 1000; // 30 minutes
const SSH_TIMEOUT_MS = 15_000;

let intervalHandle: ReturnType<typeof setInterval> | null = null;

async function refreshTokens(): Promise<void> {
  const { execFile } = await import("node:child_process");
  const { promisify } = await import("node:util");
  const execFileAsync = promisify(execFile);

  try {
    const { stdout, stderr } = await execFileAsync(
      "ssh",
      [
        "-o", "ConnectTimeout=10",
        "cloudpc",
        "powershell -NoProfile -File C:\\Users\\leo.346-CPC-QJXVZ\\graph-token-refresh.ps1 -Which Both",
      ],
      { timeout: SSH_TIMEOUT_MS },
    );

    const output = stdout.trim().replace(/\r/g, "");
    const lines = output.split("\n").filter(Boolean);

    for (const line of lines) {
      if (line.includes("REFRESHED")) {
        log.info({ token: line.split(" ")[0] }, "Graph token refreshed");
      } else if (line.includes("VALID")) {
        log.debug({ detail: line }, "Graph token still valid");
      } else if (line.includes("MISSING") || line.includes("FAILED")) {
        log.warn({ detail: line }, "Graph token refresh issue");
      }
    }
  } catch (err) {
    log.warn(
      { err: err instanceof Error ? err.message : String(err) },
      "Token refresh failed (CloudPC unreachable?)",
    );
  }
}

export function startTokenRefresh(): () => void {
  log.info({ intervalMinutes: INTERVAL_MS / 60_000 }, "Token refresh cron started");

  // Run immediately on startup
  void refreshTokens();

  // Then every 30 minutes
  intervalHandle = setInterval(() => {
    void refreshTokens();
  }, INTERVAL_MS);

  return () => {
    if (intervalHandle) {
      clearInterval(intervalHandle);
      intervalHandle = null;
      log.info("Token refresh cron stopped");
    }
  };
}
