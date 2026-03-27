/**
 * tRPC React integration with queryOptions/mutationOptions pattern.
 *
 * Usage:
 *   import { trpc } from "@/lib/trpc/react";
 *   import { useQuery, useMutation } from "@tanstack/react-query";
 *
 *   // Query
 *   const { data } = useQuery(trpc.obligation.list.queryOptions({ status: "open" }));
 *
 *   // Mutation
 *   const { mutate } = useMutation(trpc.obligation.create.mutationOptions({
 *     onSuccess: () => queryClient.invalidateQueries({ queryKey: trpc.obligation.list.queryKey() }),
 *   }));
 */

"use client";

import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createTRPCReact } from "@trpc/react-query";
import { httpBatchLink, loggerLink } from "@trpc/client";
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
  return `http://localhost:${process.env.PORT ?? 3000}`;
}

/**
 * tRPC React proxy with queryOptions(), mutationOptions(), queryKey() methods.
 *
 * This is the primary interface for client components.
 * Always use via useQuery(trpc.x.queryOptions()) -- never trpc.x.useQuery().
 */
export const trpc = createTRPCReact<AppRouter>();

/**
 * Create tRPC client with httpBatchLink and auth.
 */
function createTRPCClient() {
  return trpc.createClient({
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
}

/**
 * TRPCProvider wrapping QueryClientProvider.
 *
 * Integrates with the existing QueryClientProvider from add-tanstack-query.
 * Wrap your app with this provider to enable tRPC React hooks.
 */
export function TRPCProvider({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 30_000,
            gcTime: 5 * 60_000,
            retry: 3,
            retryDelay: (attempt) => Math.min(1000 * 2 ** attempt, 10_000),
            refetchOnWindowFocus: true,
          },
          mutations: {
            retry: 0,
          },
        },
      }),
  );
  const [trpcClient] = useState(() => createTRPCClient());

  return (
    <trpc.Provider client={trpcClient} queryClient={queryClient}>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </trpc.Provider>
  );
}
