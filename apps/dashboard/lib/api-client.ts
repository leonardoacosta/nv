/**
 * Authenticated fetch wrapper for client-side API calls.
 *
 * Better Auth session cookies are sent automatically on same-origin requests.
 * This wrapper only handles 401 redirect-to-login behavior.
 */

/**
 * Fetch wrapper that redirects to /login on 401 responses.
 *
 * @param path - URL path (e.g. "/api/obligations") or full URL
 * @param init - Standard RequestInit options
 * @returns The fetch Response
 */
export async function apiFetch(
  path: string,
  init?: RequestInit,
): Promise<Response> {
  const response = await fetch(path, {
    ...init,
    credentials: "include",
  });

  if (response.status === 401) {
    window.location.href = "/login";
    return response;
  }

  return response;
}
