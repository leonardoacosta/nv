/**
 * Azure DevOps REST API client — resilient token management.
 *
 * Three-tier token strategy:
 *   1. VALIDATE — check cached token expiry (cheap, no I/O)
 *   2. REFRESH  — SSH to CloudPC → `az account get-access-token` → cache new token
 *   3. FALLBACK — if REST call still fails, run equivalent `az` CLI command via SSH
 *
 * Token lifecycle:
 *   - Acquired via SSH (~5s), cached in memory
 *   - Validated against `expiresAt` before every call (zero-cost gate)
 *   - Proactively refreshed when within REFRESH_MARGIN of expiry
 *   - On 401: invalidate → refresh → retry once
 *   - On refresh failure: fall back to CLI for the current request
 *
 * The CLI fallback ensures availability even when the REST path is broken
 * (e.g., token endpoint down, az login expired). It's slower (~5-14s per
 * call) but keeps the system operational while the operator investigates.
 */

import { execFile } from "node:child_process";
import { SshError } from "../ssh.js";

// ── Constants ───────────────────────────────────────────────────────────

/** AAD resource ID for Azure DevOps. */
const ADO_RESOURCE = "499b84ac-1321-427f-aa17-267ca6975798";

/** Azure DevOps organization (no trailing slash). */
export const ADO_ORG = "brownandbrowninc";
const ADO_BASE = `https://dev.azure.com/${ADO_ORG}`;

/** Default REST API version. */
const API_VERSION = "7.1-preview";

// ── Token cache ─────────────────────────────────────────────────────────

interface CachedToken {
  token: string;
  /** Unix ms when the token actually expires (from AAD response). */
  expiresAtReal: number;
  /** Unix ms when we proactively consider it stale and refresh. */
  expiresAtSoft: number;
  /** When we acquired it (for diagnostics). */
  acquiredAt: number;
}

let tokenCache: CachedToken | null = null;

/**
 * How long before real expiry we proactively refresh.
 * 5 minutes = plenty of margin; real AAD tokens last ~60-75 min.
 */
const REFRESH_MARGIN_MS = 5 * 60 * 1000;

/**
 * Hard floor: never use a token with less than this remaining.
 * If we're past soft expiry but before hard, we'll try to refresh
 * in the background but still use the current token.
 */
const HARD_EXPIRY_MARGIN_MS = 60 * 1000; // 1 minute

// ── Token validation ────────────────────────────────────────────────────

/** Token state classification — drives the refresh decision. */
type TokenState = "valid" | "soft_expired" | "hard_expired" | "missing";

function classifyToken(): TokenState {
  if (!tokenCache) return "missing";
  const now = Date.now();
  if (now >= tokenCache.expiresAtReal - HARD_EXPIRY_MARGIN_MS) return "hard_expired";
  if (now >= tokenCache.expiresAtSoft) return "soft_expired";
  return "valid";
}

// ── Token acquisition ───────────────────────────────────────────────────

/**
 * Acquire a fresh AAD bearer token for Azure DevOps via SSH.
 * This is the only function that touches SSH — everything else is pure HTTPS.
 */
async function acquireTokenViaSSH(cloudpcHost: string): Promise<CachedToken> {
  const ps = `az account get-access-token --resource ${ADO_RESOURCE} -o json 2>$null`;
  const cmd = `powershell -NoProfile -ExecutionPolicy Bypass -Command "${ps}"`;

  const raw = await sshExecSimple(cloudpcHost, cmd, 30_000);

  let parsed: { accessToken: string; expiresOn: string };
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new SshError(
      `Failed to parse token response from CloudPC: ${raw.slice(0, 200)}`,
      502,
    );
  }

  if (!parsed.accessToken) {
    throw new SshError("AAD token response missing accessToken field", 502);
  }

  const expiresAtReal = new Date(parsed.expiresOn).getTime();
  const now = Date.now();

  const cached: CachedToken = {
    token: parsed.accessToken,
    expiresAtReal,
    expiresAtSoft: expiresAtReal - REFRESH_MARGIN_MS,
    acquiredAt: now,
  };

  tokenCache = cached;
  return cached;
}

/**
 * Get a valid token — refresh if needed.
 *
 * Decision tree:
 *   valid        → return cached (zero I/O)
 *   soft_expired → try refresh; if fails, return cached (still technically valid)
 *   hard_expired → must refresh; throw if refresh fails
 *   missing      → must refresh; throw if refresh fails
 */
export async function getAdoToken(cloudpcHost: string): Promise<string> {
  const state = classifyToken();

  switch (state) {
    case "valid":
      return tokenCache!.token;

    case "soft_expired": {
      // Best-effort refresh — fall back to current token if SSH fails
      try {
        const fresh = await acquireTokenViaSSH(cloudpcHost);
        return fresh.token;
      } catch {
        // Token is soft-expired but not hard-expired — still usable
        return tokenCache!.token;
      }
    }

    case "hard_expired":
    case "missing": {
      const fresh = await acquireTokenViaSSH(cloudpcHost);
      return fresh.token;
    }
  }
}

/** Force-invalidate the cached token (called on 401). */
export function clearTokenCache(): void {
  tokenCache = null;
}

/** Diagnostic: return cache state without side effects. */
export function tokenDiagnostics(): {
  state: TokenState;
  ageMs: number | null;
  remainingMs: number | null;
} {
  const state = classifyToken();
  if (!tokenCache) return { state, ageMs: null, remainingMs: null };
  const now = Date.now();
  return {
    state,
    ageMs: now - tokenCache.acquiredAt,
    remainingMs: tokenCache.expiresAtReal - now,
  };
}

// ── REST helpers ────────────────────────────────────────────────────────

export interface AdoRestOptions {
  /** Override the base URL (default: dev.azure.com/{org}). */
  baseUrl?: string;
  /** HTTP method (default GET). */
  method?: "GET" | "POST" | "PATCH" | "PUT" | "DELETE";
  /** JSON body for POST/PATCH/PUT. */
  body?: unknown;
  /** Extra query parameters (appended alongside api-version). */
  query?: Record<string, string | number | boolean | undefined>;
  /** Override api-version. */
  apiVersion?: string;
  /**
   * CLI fallback command. If provided and the REST call fails after
   * token refresh, this `az` command runs via SSH as a last resort.
   * Example: "az pipelines list --org https://dev.azure.com/org -o json"
   */
  cliFallback?: string;
}

/**
 * Make an authenticated REST call to Azure DevOps.
 * Returns the parsed JSON response as a string.
 */
export async function adoRest(
  cloudpcHost: string,
  path: string,
  opts: AdoRestOptions = {},
): Promise<string> {
  const token = await getAdoToken(cloudpcHost);
  return executeRest(token, path, opts);
}

/** Pure REST execution — no token logic, just HTTP. */
async function executeRest(
  token: string,
  path: string,
  opts: AdoRestOptions,
): Promise<string> {
  const base = opts.baseUrl ?? ADO_BASE;
  const version = opts.apiVersion ?? API_VERSION;
  const method = opts.method ?? "GET";

  const url = new URL(`${base}/${path}`);
  url.searchParams.set("api-version", version);
  if (opts.query) {
    for (const [k, v] of Object.entries(opts.query)) {
      if (v !== undefined) {
        url.searchParams.set(k, String(v));
      }
    }
  }

  const headers: Record<string, string> = {
    Authorization: `Bearer ${token}`,
    Accept: "application/json",
  };
  if (opts.body !== undefined) {
    headers["Content-Type"] = "application/json";
  }

  const fetchOpts: RequestInit = {
    method,
    headers,
    body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
  };

  let resp: Response;
  try {
    resp = await fetch(url.toString(), fetchOpts);
  } catch (err) {
    throw new SshError(
      `ADO REST fetch failed: ${err instanceof Error ? err.message : String(err)}`,
      502,
    );
  }

  if (resp.status === 401) {
    throw new TokenExpiredError("ADO REST: 401 Unauthorized");
  }

  if (!resp.ok) {
    const body = await resp.text().catch(() => "(no body)");
    throw new SshError(
      `ADO REST ${resp.status} ${resp.statusText}: ${body.slice(0, 500)}`,
      502,
    );
  }

  // 204 No Content (e.g., DELETE success)
  if (resp.status === 204) {
    return JSON.stringify({ status: "success" });
  }

  const json = await resp.json();
  return JSON.stringify(json, null, 2);
}

/** Sentinel error for 401s — triggers the refresh-and-retry path. */
class TokenExpiredError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TokenExpiredError";
  }
}

// ── CLI fallback ────────────────────────────────────────────────────────

/**
 * Execute an `az` CLI command on CloudPC via SSH.
 * Only used as a last resort when REST + token refresh both fail.
 */
async function executeCliFallback(
  cloudpcHost: string,
  cliCommand: string,
): Promise<string> {
  const ps = [
    `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
    `if (-not $token) { Write-Error 'CLI fallback: failed to acquire AAD token'; exit 1 }`,
    `$env:AZURE_DEVOPS_EXT_PAT = $token`,
    cliCommand,
  ].join("; ");

  const cmd = `powershell -NoProfile -ExecutionPolicy Bypass -Command "${ps}"`;
  return sshExecSimple(cloudpcHost, cmd, 45_000);
}

// ── Main entry point: REST with retry + CLI fallback ────────────────────

/**
 * The primary function all tool handlers should call.
 *
 * Flow:
 *   1. Try REST with cached token
 *   2. On 401 → invalidate cache → refresh token → retry REST
 *   3. On any failure after retry → if `cliFallback` provided, run CLI via SSH
 *   4. If CLI also fails → throw the original REST error (more informative)
 */
export async function adoRestWithRetry(
  cloudpcHost: string,
  path: string,
  opts: AdoRestOptions = {},
): Promise<string> {
  // ── Attempt 1: REST with current token ──
  try {
    return await adoRest(cloudpcHost, path, opts);
  } catch (err) {
    // Non-401 errors from adoRest (network, 500, etc.) — skip to fallback
    if (!(err instanceof TokenExpiredError) && !(err instanceof SshError && err.message.includes("401"))) {
      return handleRestFailure(err, cloudpcHost, opts);
    }

    // ── Attempt 2: Refresh token and retry REST ──
    clearTokenCache();
    try {
      return await adoRest(cloudpcHost, path, opts);
    } catch (retryErr) {
      return handleRestFailure(retryErr, cloudpcHost, opts);
    }
  }
}

/**
 * Handle a REST failure: try CLI fallback if available, otherwise throw.
 */
async function handleRestFailure(
  restError: unknown,
  cloudpcHost: string,
  opts: AdoRestOptions,
): Promise<string> {
  if (!opts.cliFallback) {
    throw restError;
  }

  // ── Attempt 3: CLI fallback ──
  try {
    const result = await executeCliFallback(cloudpcHost, opts.cliFallback);
    // CLI succeeded — also try to refresh the REST token in the background
    // so subsequent calls go through the fast path again
    refreshTokenInBackground(cloudpcHost);
    return result;
  } catch (cliErr) {
    // Both REST and CLI failed — throw the REST error (usually more informative)
    // but include CLI error context
    const cliMsg = cliErr instanceof Error ? cliErr.message : String(cliErr);
    const restMsg = restError instanceof Error ? restError.message : String(restError);
    throw new SshError(
      `REST failed: ${restMsg} | CLI fallback also failed: ${cliMsg}`,
      502,
    );
  }
}

/**
 * Best-effort background token refresh. Fire-and-forget.
 * Called after a CLI fallback succeeds, so the next REST call won't need CLI.
 */
function refreshTokenInBackground(cloudpcHost: string): void {
  acquireTokenViaSSH(cloudpcHost).catch(() => {
    // Swallow — if background refresh fails, next call will try again
  });
}

// ── Low-level SSH (only for token acquisition + CLI fallback) ───────────

/** Lines containing these substrings are stripped from SSH stdout. */
const NOISE_PATTERNS = ["WARNING:", "vulnerable", "upgraded", "security fix"];

function sshExecSimple(
  host: string,
  cmd: string,
  timeoutMs: number,
): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile(
      "ssh",
      ["-o", "ConnectTimeout=10", host, cmd],
      { timeout: timeoutMs },
      (error, stdout, stderr) => {
        if (error) {
          if (error.killed || error.code === "ETIMEDOUT") {
            return reject(
              new SshError(
                `CloudPC SSH timed out after ${Math.round(timeoutMs / 1000)}s`,
                504,
              ),
            );
          }
          const stderrStr = stderr ?? "";
          if (
            ["Connection refused", "timed out", "No route to host"].some(
              (p) => stderrStr.includes(p),
            )
          ) {
            return reject(
              new SshError("CloudPC unreachable via SSH", 503),
            );
          }
          return reject(
            new SshError(stderrStr.trim() || error.message, 502),
          );
        }

        const filtered = (stdout ?? "")
          .split("\n")
          .filter(
            (line) =>
              !NOISE_PATTERNS.some((p) => line.includes(p)),
          )
          .join("\n")
          .trim();

        resolve(filtered);
      },
    );
  });
}
