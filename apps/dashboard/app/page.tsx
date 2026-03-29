/**
 * Home page — React Server Component wrapper.
 *
 * Prefetches critical dashboard queries on the server so the client renders
 * from cache on first paint without showing a loading skeleton.
 *
 * Prefetched queries:
 * - activity feed (most visible panel)
 * - obligations list (action items panel)
 * - messages list (action items panel)
 * - briefing latest (Nova status panel)
 * - fleet status (Nova status panel)
 * - automations (Nova status panel — watcher/interval)
 */

import { HydrationBoundary, dehydrate } from "@tanstack/react-query";
import { createServerTRPC } from "@/lib/trpc/server-trpc";
import HomeClient from "./HomeClient";

export default async function DashboardPage() {
  const { trpc, queryClient } = await createServerTRPC();

  await Promise.allSettled([
    queryClient.prefetchQuery(trpc.system.activityFeed.queryOptions(undefined)),
    queryClient.prefetchQuery(trpc.obligation.list.queryOptions({})),
    queryClient.prefetchQuery(
      trpc.message.list.queryOptions({ limit: 50 } as Record<string, unknown>),
    ),
    queryClient.prefetchQuery(trpc.briefing.latest.queryOptions(undefined)),
    queryClient.prefetchQuery(trpc.system.fleetStatus.queryOptions(undefined)),
    queryClient.prefetchQuery(trpc.automation.getAll.queryOptions(undefined)),
  ]);

  return (
    <HydrationBoundary state={dehydrate(queryClient)}>
      <HomeClient />
    </HydrationBoundary>
  );
}
