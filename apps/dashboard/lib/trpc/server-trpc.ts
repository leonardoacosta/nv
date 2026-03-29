/**
 * Server-side tRPC options proxy for RSC prefetching.
 *
 * Creates a `createTRPCOptionsProxy` backed by the server caller (no HTTP round-trip).
 * Use in React Server Components to prefetch queries into a dehydrated QueryClient
 * so client components skip the loading state on first paint.
 *
 * Usage in a RSC page:
 *
 *   import { createServerTRPC } from "@/lib/trpc/server-trpc";
 *   import { HydrationBoundary, dehydrate } from "@tanstack/react-query";
 *
 *   export default async function Page() {
 *     const { trpc, queryClient } = await createServerTRPC();
 *
 *     await queryClient.prefetchQuery(trpc.obligation.list.queryOptions({}));
 *     await queryClient.prefetchQuery(trpc.briefing.latest.queryOptions(undefined));
 *
 *     return (
 *       <HydrationBoundary state={dehydrate(queryClient)}>
 *         <YourClientPage />
 *       </HydrationBoundary>
 *     );
 *   }
 */

import { createTRPCOptionsProxy } from "@trpc/tanstack-react-query";
import { createQueryClient } from "@/lib/query-client";
import { createServerCaller } from "@/lib/trpc/server";
import { dashboardRouter } from "@/lib/trpc/router";
import type { TRPCContext } from "@nova/api";
import { cookies } from "next/headers";

const AUTH_COOKIE_NAME = "dashboard_token";

/**
 * Creates a server-side tRPC options proxy backed by the router directly
 * (no HTTP round-trip) with auth from the incoming request cookies.
 *
 * Returns:
 * - `trpc`: typed proxy — call `.queryOptions()` on any procedure
 * - `queryClient`: fresh QueryClient to accumulate prefetched data
 */
export async function createServerTRPC() {
  const cookieStore = await cookies();
  const token = cookieStore.get(AUTH_COOKIE_NAME)?.value ?? null;
  const ctx: TRPCContext = { token };

  const queryClient = createQueryClient();

  const trpc = createTRPCOptionsProxy<typeof dashboardRouter>({
    router: dashboardRouter,
    ctx,
    queryClient,
  });

  return { trpc, queryClient };
}
