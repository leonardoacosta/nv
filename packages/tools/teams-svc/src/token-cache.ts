/**
 * O365 token cache for teams-svc.
 *
 * Same pattern as graph-svc/token-cache.ts. Reads .graph-token.json from
 * CloudPC via SSH on first call, then serves from memory until TTL expires.
 */

import { execFile } from "node:child_process";

// ── Constants ──────────────────────────────────────────────────────────

const DEFAULT_HOST = "cloudpc";
const TOKEN_FILE_PATH = String.raw`C:\Users\leo.346-CPC-QJXVZ\.graph-token.json`;
const REFRESH_MARGIN_MS = 5 * 60 * 1000;

// ── Token cache ────────────────────────────────────────────────────────

interface CachedToken {
  token: string;
  expiresAt: number;
  acquiredAt: number;
}

let o365Cache: CachedToken | null = null;

// ── O365 Token ─────────────────────────────────────────────────────────

/**
 * Get a valid O365 bearer token for Graph API calls.
 */
export async function getO365Token(cloudpcHost: string = DEFAULT_HOST): Promise<string> {
  const now = Date.now();

  if (o365Cache && now < o365Cache.expiresAt - REFRESH_MARGIN_MS) {
    return o365Cache.token;
  }

  const raw = await sshReadFile(cloudpcHost, TOKEN_FILE_PATH);

  let parsed: { access_token?: string; expires_on?: number; accessToken?: string; expiresOn?: string };
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error(`Failed to parse .graph-token.json from CloudPC: ${raw.slice(0, 200)}`);
  }

  const token = parsed.access_token ?? parsed.accessToken;
  if (!token) {
    throw new Error("O365 token file missing access_token field");
  }

  let expiresAt: number;
  if (parsed.expires_on) {
    expiresAt = parsed.expires_on * 1000;
  } else if (parsed.expiresOn) {
    expiresAt = new Date(parsed.expiresOn).getTime();
  } else {
    expiresAt = now + 60 * 60 * 1000;
  }

  o365Cache = { token, expiresAt, acquiredAt: now };
  return token;
}

/** Force-invalidate the O365 token cache (called on 401). */
export function clearO365TokenCache(): void {
  o365Cache = null;
}

// ── Low-level SSH ──────────────────────────────────────────────────────

const NOISE_PATTERNS = ["WARNING:", "vulnerable", "upgraded", "security fix"];

function sshReadFile(host: string, filePath: string): Promise<string> {
  const cmd = `powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-Content '${filePath}' -Raw"`;
  return sshExec(host, cmd, 15_000);
}

function sshExec(host: string, cmd: string, timeoutMs: number): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile(
      "ssh",
      ["-o", "ConnectTimeout=10", host, cmd],
      { timeout: timeoutMs },
      (error, stdout, stderr) => {
        if (error) {
          if (error.killed || error.code === "ETIMEDOUT") {
            return reject(new Error(`CloudPC SSH timed out after ${Math.round(timeoutMs / 1000)}s`));
          }
          const stderrStr = stderr ?? "";
          if (
            ["Connection refused", "timed out", "No route to host"].some(
              (p) => stderrStr.includes(p),
            )
          ) {
            return reject(new Error("CloudPC unreachable via SSH"));
          }
          return reject(new Error(stderrStr.trim() || error.message));
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
