/**
 * Shared utilities for graph-svc tools.
 */

/**
 * Sanitize a user-supplied string before passing it to SSH/PowerShell.
 * Strips single quotes, semicolons, backticks, and pipe characters to prevent injection.
 */
export function sanitize(value: string): string {
  return value.replace(/[';`|]/g, "");
}

/**
 * Clamp an integer to a range.
 * Useful for limiting user-supplied counts (e.g., limit params).
 */
export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * Parse a string to an integer with a default fallback.
 */
export function parseIntOr(raw: string | undefined, fallback: number): number {
  if (!raw) return fallback;
  const parsed = parseInt(raw, 10);
  return isNaN(parsed) ? fallback : parsed;
}
