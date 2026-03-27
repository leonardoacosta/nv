/**
 * O365 token cache — reads token from CloudPC via SSH, caches in memory.
 *
 * The O365 token is stored in `.graph-token.json` on the CloudPC and auto-refreshed
 * every 30 minutes by the daemon cron. This module reads it once via SSH, then
 * serves it from memory until TTL expires or a 401 forces a refresh.
 *
 * ADO tokens are handled separately by ado-rest.ts which already has its own
 * resilient token cache with three-tier refresh strategy.
 */

import { execFile } from "node:child_process";
import { SshError } from "./ssh.js";

// ── Constants ──────────────────────────────────────────────────────────

/** Default CloudPC SSH host. */
const DEFAULT_HOST = "cloudpc";

/** Path to the O365 token file on the CloudPC. */
const TOKEN_FILE_PATH = String.raw`C:\Users\leo.346-CPC-QJXVZ\.graph-token.json`;

/**
 * Proactive refresh margin — refresh 5 minutes before real expiry.
 * The daemon cron refreshes every 30min, so tokens have ~25-30min remaining.
 */
const REFRESH_MARGIN_MS = 5 * 60 * 1000;

/** Azure management API resource for PIM operations. */
const MGMT_RESOURCE = "https://management.azure.com";

// ── Token cache ────────────────────────────────────────────────────────

interface CachedToken {
  token: string;
  /** Unix ms when the token expires (from the token file / az response). */
  expiresAt: number;
  /** When we acquired it (for diagnostics). */
  acquiredAt: number;
}

let o365Cache: CachedToken | null = null;
let mgmtCache: CachedToken | null = null;

// ── O365 Token ─────────────────────────────────────────────────────────

/**
 * Get a valid O365 bearer token for Graph API calls.
 * Reads from CloudPC on first call or when expired. Cached in memory.
 */
export async function getO365Token(cloudpcHost: string = DEFAULT_HOST): Promise<string> {
  const now = Date.now();

  if (o365Cache && now < o365Cache.expiresAt - REFRESH_MARGIN_MS) {
    return o365Cache.token;
  }

  // Token expired or missing — fetch from CloudPC
  const raw = await sshReadFile(cloudpcHost, TOKEN_FILE_PATH);

  let parsed: { access_token?: string; expires_on?: number; accessToken?: string; expiresOn?: string };
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new SshError(
      `Failed to parse .graph-token.json from CloudPC: ${raw.slice(0, 200)}`,
      502,
    );
  }

  // Support both formats: {access_token, expires_on} and {accessToken, expiresOn}
  const token = parsed.access_token ?? parsed.accessToken;
  if (!token) {
    throw new SshError("O365 token file missing access_token field", 502);
  }

  let expiresAt: number;
  if (parsed.expires_on) {
    // Unix seconds -> ms
    expiresAt = parsed.expires_on * 1000;
  } else if (parsed.expiresOn) {
    expiresAt = new Date(parsed.expiresOn).getTime();
  } else {
    // No expiry info — assume 1 hour from now
    expiresAt = now + 60 * 60 * 1000;
  }

  o365Cache = { token, expiresAt, acquiredAt: now };
  return token;
}

/** Force-invalidate the O365 token cache (called on 401). */
export function clearO365TokenCache(): void {
  o365Cache = null;
}

// ── Management Token (for PIM) ─────────────────────────────────────────

/**
 * Get a bearer token for management.azure.com (used by PIM operations).
 * Acquired via `az account get-access-token --resource https://management.azure.com`.
 */
export async function getMgmtToken(cloudpcHost: string = DEFAULT_HOST): Promise<string> {
  const now = Date.now();

  if (mgmtCache && now < mgmtCache.expiresAt - REFRESH_MARGIN_MS) {
    return mgmtCache.token;
  }

  const cmd = `az account get-access-token --resource ${MGMT_RESOURCE} -o json 2>$null`;
  const ps = `powershell -NoProfile -ExecutionPolicy Bypass -Command "${cmd}"`;
  const raw = await sshExec(cloudpcHost, ps, 30_000);

  let parsed: { accessToken: string; expiresOn: string };
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new SshError(
      `Failed to parse mgmt token response: ${raw.slice(0, 200)}`,
      502,
    );
  }

  if (!parsed.accessToken) {
    throw new SshError("Management token response missing accessToken field", 502);
  }

  const expiresAt = new Date(parsed.expiresOn).getTime();
  mgmtCache = { token: parsed.accessToken, expiresAt, acquiredAt: now };
  return parsed.accessToken;
}

/** Force-invalidate the management token cache. */
export function clearMgmtTokenCache(): void {
  mgmtCache = null;
}

// ── Low-level SSH helpers ──────────────────────────────────────────────

/** Lines containing these substrings are stripped from SSH stdout. */
const NOISE_PATTERNS = ["WARNING:", "vulnerable", "upgraded", "security fix"];

/**
 * Read a file from the CloudPC via SSH + PowerShell.
 */
function sshReadFile(host: string, filePath: string): Promise<string> {
  const cmd = `powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-Content '${filePath}' -Raw"`;
  return sshExec(host, cmd, 15_000);
}

/**
 * Execute a command on the CloudPC via SSH.
 */
function sshExec(host: string, cmd: string, timeoutMs: number): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile(
      "ssh",
      ["-o", "ConnectTimeout=10", host, cmd],
      { timeout: timeoutMs },
      (error, stdout, stderr) => {
        if (error) {
          if (error.killed || error.code === "ETIMEDOUT") {
            return reject(
              new SshError(`CloudPC SSH timed out after ${Math.round(timeoutMs / 1000)}s`, 504),
            );
          }
          const stderrStr = stderr ?? "";
          if (
            ["Connection refused", "timed out", "No route to host"].some(
              (p) => stderrStr.includes(p),
            )
          ) {
            return reject(new SshError("CloudPC unreachable via SSH", 503));
          }
          return reject(new SshError(stderrStr.trim() || error.message, 502));
        }

        const filtered = (stdout ?? "")
          .split("\n")
          .filter((line) => !NOISE_PATTERNS.some((p) => line.includes(p)))
          .join("\n")
          .trim();

        resolve(filtered);
      },
    );
  });
}
