/**
 * SOCKS5 HTTP client — routes requests through localhost:1080 via curl.
 *
 * Uses `curl --socks5-hostname` via child_process. No npm dependency needed.
 * Proven to deliver 0.4-0.6s per Graph/ADO call vs 5-10s via SSH+PowerShell.
 */

import { execFile } from "node:child_process";

const SOCKS_PROXY = "localhost:1080";

/** How long (ms) to cache the SOCKS availability probe result. */
const PROBE_TTL_MS = 60_000;

let socksAvailableCache: { available: boolean; checkedAt: number } | null = null;

export class SocksError extends Error {
  constructor(
    message: string,
    public readonly httpStatus: 502 | 503 | 504,
  ) {
    super(message);
    this.name = "SocksError";
  }
}

/**
 * Check if the SOCKS proxy at localhost:1080 is reachable.
 * Result is cached for 60 seconds to avoid repeated probes.
 */
export async function isSocksAvailable(): Promise<boolean> {
  const now = Date.now();
  if (socksAvailableCache && now - socksAvailableCache.checkedAt < PROBE_TTL_MS) {
    return socksAvailableCache.available;
  }

  try {
    // Quick connectivity check — try to reach graph.microsoft.com through the proxy
    await socksExec("GET", "https://graph.microsoft.com/v1.0/$metadata", undefined, undefined, 5_000);
    socksAvailableCache = { available: true, checkedAt: now };
    return true;
  } catch {
    socksAvailableCache = { available: false, checkedAt: now };
    return false;
  }
}

/** Invalidate the cached probe so the next call re-checks. */
export function resetSocksProbe(): void {
  socksAvailableCache = null;
}

/**
 * HTTP GET through the SOCKS5 proxy.
 */
export async function socksGet(
  url: string,
  token: string,
  timeoutMs: number = 10_000,
): Promise<string> {
  return socksExec("GET", url, token, undefined, timeoutMs);
}

/**
 * HTTP POST through the SOCKS5 proxy (JSON body).
 */
export async function socksPost(
  url: string,
  token: string,
  body: unknown,
  timeoutMs: number = 10_000,
): Promise<string> {
  return socksExec("POST", url, token, body, timeoutMs);
}

/**
 * HTTP PATCH through the SOCKS5 proxy (JSON body).
 */
export async function socksPatch(
  url: string,
  token: string,
  body: unknown,
  timeoutMs: number = 10_000,
): Promise<string> {
  return socksExec("PATCH", url, token, body, timeoutMs);
}

/**
 * HTTP PUT through the SOCKS5 proxy (JSON body).
 */
export async function socksPut(
  url: string,
  token: string,
  body: unknown,
  timeoutMs: number = 10_000,
): Promise<string> {
  return socksExec("PUT", url, token, body, timeoutMs);
}

/**
 * HTTP DELETE through the SOCKS5 proxy.
 */
export async function socksDelete(
  url: string,
  token: string,
  timeoutMs: number = 10_000,
): Promise<string> {
  return socksExec("DELETE", url, token, undefined, timeoutMs);
}

/**
 * Low-level curl execution through the SOCKS5 proxy.
 */
function socksExec(
  method: string,
  url: string,
  token: string | undefined,
  body: unknown | undefined,
  timeoutMs: number,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const args = [
      "--socks5-hostname", SOCKS_PROXY,
      "-sf",
      "--max-time", String(Math.ceil(timeoutMs / 1000)),
      "-X", method,
    ];

    if (token) {
      args.push("-H", `Authorization: Bearer ${token}`);
    }

    if (body !== undefined) {
      args.push(
        "-H", "Content-Type: application/json",
        "-d", JSON.stringify(body),
      );
    }

    args.push(url);

    execFile("curl", args, { timeout: timeoutMs + 2_000 }, (err, stdout, stderr) => {
      if (err) {
        // curl exit codes: 7 = connection refused, 28 = timeout
        if (err.code === 7 || stderr?.includes("connect to")) {
          return reject(
            new SocksError("SOCKS proxy unreachable at localhost:1080", 503),
          );
        }
        if (err.code === 28 || err.killed) {
          return reject(
            new SocksError(
              `Request timed out after ${Math.ceil(timeoutMs / 1000)}s: ${url}`,
              504,
            ),
          );
        }
        return reject(
          new SocksError(
            `SOCKS request failed: ${stderr?.trim() || err.message}`,
            502,
          ),
        );
      }

      resolve(stdout);
    });
  });
}
