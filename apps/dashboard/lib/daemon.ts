/**
 * Daemon connection helpers.
 *
 * All server-side code that needs to call the NV daemon directly
 * (e.g. API route handlers) should use `daemonFetch` rather than
 * constructing the URL manually.
 */

export const DAEMON_URL =
  process.env.DAEMON_URL ?? "http://127.0.0.1:3443";

/**
 * Fetch a path from the NV daemon.
 *
 * @param path - Must start with `/`, e.g. `/api/sessions`
 * @param init - Standard `RequestInit` options (method, body, headers, etc.)
 * @returns The raw `Response` from the daemon — callers handle status/parsing.
 */
export function daemonFetch(path: string, init?: RequestInit): Promise<Response> {
  const url = `${DAEMON_URL}${path}`;
  return fetch(url, init);
}
