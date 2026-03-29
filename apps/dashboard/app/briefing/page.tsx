/**
 * Briefing page — React Server Component wrapper.
 *
 * Prefetches the latest briefing and history on the server so the client
 * renders with data immediately on first paint (no loading skeleton flash).
 *
 * Prefetched queries:
 * - briefing.latest: the most recent briefing entry
 * - briefing.history: the last 10 briefings (history rail)
 */

import { HydrationBoundary, dehydrate } from "@tanstack/react-query";
import { createServerTRPC } from "@/lib/trpc/server-trpc";
import BriefingClient from "./BriefingClient";

export default async function BriefingPage() {
  const { trpc, queryClient } = await createServerTRPC();

  await Promise.allSettled([
    queryClient.prefetchQuery(trpc.briefing.latest.queryOptions(undefined)),
    queryClient.prefetchQuery(trpc.briefing.history.queryOptions({ limit: 10 })),
  ]);

  return (
    <HydrationBoundary state={dehydrate(queryClient)}>
      <BriefingClient />
    </HydrationBoundary>
  );
}
