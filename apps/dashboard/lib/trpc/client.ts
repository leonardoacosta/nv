/**
 * tRPC client configuration with httpBatchLink.
 *
 * Uses Better Auth session cookies (sent automatically).
 * Redirects to /login on 401 responses.
 */

import { createTRPCClient, httpBatchLink, loggerLink } from "@trpc/client";
import superjson from "superjson";
import type { DashboardRouter } from "@/lib/trpc/router";

function getBaseUrl() {
  if (typeof window !== "undefined") return "";
  // Server-side: use localhost
  return `http://localhost:${process.env.PORT ?? 3000}`;
}

/**
 * Vanilla tRPC client (no React hooks).
 * Useful for server-side calls and non-React contexts.
 */
export const trpcClient = createTRPCClient<DashboardRouter>({
  links: [
    loggerLink({
      enabled: (opts) =>
        process.env.NODE_ENV === "development" ||
        (opts.direction === "down" && opts.result instanceof Error),
    }),
    httpBatchLink({
      url: `${getBaseUrl()}/api/trpc`,
      transformer: superjson,
      fetch(url, options) {
        return fetch(url, {
          ...options,
          credentials: "include",
        }).then((response) => {
          if (response.status === 401 && typeof window !== "undefined") {
            window.location.href = "/login";
          }
          return response;
        });
      },
    }),
  ],
});
