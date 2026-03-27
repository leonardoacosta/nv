/**
 * tRPC React integration with queryOptions/mutationOptions pattern.
 *
 * Uses @trpc/tanstack-react-query (tRPC v11) which provides the typed
 * options proxy via useTRPC().
 *
 * Usage:
 *   import { useTRPC } from "@/lib/trpc/react";
 *   import { useQuery, useMutation } from "@tanstack/react-query";
 *
 *   function MyComponent() {
 *     const trpc = useTRPC();
 *     const { data } = useQuery(trpc.obligation.list.queryOptions({ status: "open" }));
 *     const { mutate } = useMutation(trpc.obligation.create.mutationOptions({
 *       onSuccess: () => queryClient.invalidateQueries({ queryKey: trpc.obligation.list.queryKey() }),
 *     }));
 *   }
 */

"use client";

import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { createTRPCContext } from "@trpc/tanstack-react-query";
import { createTRPCClient, httpBatchLink, loggerLink } from "@trpc/client";
import superjson from "superjson";
import type { DashboardRouter } from "@/lib/trpc/router";

function getBaseUrl() {
  if (typeof window !== "undefined") return "";
  return `http://localhost:${process.env.PORT ?? 3000}`;
}

/**
 * Create the tRPC context with typed provider and useTRPC hook.
 *
 * useTRPC() returns a proxy with .queryOptions(), .mutationOptions(),
 * .queryKey() on each procedure -- the v11 options pattern.
 */
const { TRPCProvider, useTRPC } = createTRPCContext<DashboardRouter>();

export { useTRPC };

/**
 * Create tRPC client with httpBatchLink and session cookie auth.
 */
function makeTRPCClient() {
  return createTRPCClient<DashboardRouter>({
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
}

/**
 * TRPCReactProvider wrapping QueryClientProvider + TRPCProvider.
 *
 * Wrap your app with this to enable useTRPC() in all child components.
 */
export function TRPCReactProvider({ children }: { children: React.ReactNode }) {
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
  const [trpcClient] = useState(() => makeTRPCClient());

  return (
    <QueryClientProvider client={queryClient}>
      <TRPCProvider queryClient={queryClient} trpcClient={trpcClient}>
        {children}
        {process.env.NODE_ENV === "development" && (
          <ReactQueryDevtools initialIsOpen={false} />
        )}
      </TRPCProvider>
    </QueryClientProvider>
  );
}
