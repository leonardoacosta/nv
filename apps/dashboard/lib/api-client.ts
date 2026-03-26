/**
 * Authenticated fetch wrapper for client-side API calls.
 *
 * Reads the dashboard_token cookie and injects an Authorization header.
 * On 401 responses, clears the cookie and redirects to /login.
 */

const AUTH_COOKIE_NAME = "dashboard_token";

function getTokenFromCookie(): string | null {
  if (typeof document === "undefined") return null;
  const match = document.cookie
    .split("; ")
    .find((row) => row.startsWith(`${AUTH_COOKIE_NAME}=`));
  if (!match) return null;
  return decodeURIComponent(match.split("=")[1] ?? "");
}

/**
 * Fetch wrapper that injects Authorization: Bearer header.
 *
 * @param path - URL path (e.g. "/api/obligations") or full URL
 * @param init - Standard RequestInit options
 * @returns The fetch Response
 */
export async function apiFetch(
  path: string,
  init?: RequestInit,
): Promise<Response> {
  const token = getTokenFromCookie();

  const headers = new Headers(init?.headers);
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }

  const response = await fetch(path, { ...init, headers });

  if (response.status === 401) {
    // Clear cookie and redirect to login
    document.cookie = `${AUTH_COOKIE_NAME}=; path=/; max-age=0; samesite=strict`;
    window.location.href = "/login";
    // Return the response in case callers need it before redirect completes
    return response;
  }

  return response;
}
