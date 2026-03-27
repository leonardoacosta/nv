/**
 * tRPC client configuration with httpBatchLink.
 *
 * Uses the dashboard_token cookie for bearer auth.
 * Redirects to /login on 401 responses.
 */

import { createTRPCClient, httpBatchLink, loggerLink } from "@trpc/client";
import superjson from "superjson";
import type { AppRouter } from "@nova/api";

const AUTH_COOKIE_NAME = "dashboard_token";

function getTokenFromCookie(): string | null {
  if (typeof document === "undefined") return null;
  const match = document.cookie
    .split("; ")
    .find((row) => row.startsWith(`${AUTH_COOKIE_NAME}=`));
  if (!match) return null;
  return decodeURIComponent(match.split("=")[1] ?? "");
}

function getBaseUrl() {
  if (typeof window !== "undefined") return "";
  // Server-side: use localhost
  return `http://localhost:${process.env.PORT ?? 3000}`;
}

/**
 * Vanilla tRPC client (no React hooks).
 * Useful for server-side calls and non-React contexts.
 */
export const trpcClient = createTRPCClient<AppRouter>({
  links: [
    loggerLink({
      enabled: (opts) =>
        process.env.NODE_ENV === "development" ||
        (opts.direction === "down" && opts.result instanceof Error),
    }),
    httpBatchLink({
      url: `${getBaseUrl()}/api/trpc`,
      transformer: superjson,
      headers() {
        const token = getTokenFromCookie();
        const headers: Record<string, string> = {};
        if (token) {
          headers["Authorization"] = `Bearer ${token}`;
        }
        return headers;
      },
      fetch(url, options) {
        return fetch(url, {
          ...options,
          credentials: "include",
        }).then((response) => {
          if (response.status === 401 && typeof window !== "undefined") {
            document.cookie = `${AUTH_COOKIE_NAME}=; path=/; max-age=0; samesite=strict`;
            window.location.href = "/login";
          }
          return response;
        });
      },
    }),
  ],
});
