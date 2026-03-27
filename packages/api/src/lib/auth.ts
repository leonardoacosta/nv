/**
 * Standalone timing-safe token verification for the API package.
 *
 * Replicates the logic from apps/dashboard/lib/auth.ts without
 * creating a cross-dependency. Both implementations are <15 lines
 * and share the same constant-time comparison approach.
 */

import { timingSafeEqual } from "crypto";

export const AUTH_COOKIE_NAME = "dashboard_token";

/** Read the configured token from the environment. */
export function getToken(): string | undefined {
  return process.env.DASHBOARD_TOKEN;
}

/** Returns true if DASHBOARD_TOKEN is set and non-empty. */
export function isAuthEnabled(): boolean {
  const token = getToken();
  return typeof token === "string" && token.length > 0;
}

/**
 * Constant-time comparison of a candidate token against the stored token.
 * Returns false if auth is disabled (no DASHBOARD_TOKEN set).
 */
export function verifyToken(candidate: string): boolean {
  const token = getToken();
  if (!token) return false;

  const a = Buffer.from(candidate);
  const b = Buffer.from(token);

  if (a.length !== b.length) {
    // Compare against self to maintain constant-time behavior
    timingSafeEqual(b, b);
    return false;
  }

  return timingSafeEqual(a, b);
}
