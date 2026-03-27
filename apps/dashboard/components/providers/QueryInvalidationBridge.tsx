"use client";

import { useQueryClient } from "@tanstack/react-query";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";
import { trpc } from "@/lib/trpc/react";

/**
 * QueryInvalidationBridge -- listens to WebSocket events from the daemon
 * and invalidates relevant tRPC query caches.
 *
 * Uses tRPC queryKey() helpers for cache invalidation so that both tRPC
 * queries and any remaining legacy REST queries are refreshed.
 *
 * Renders nothing -- pure side-effect component.
 */

/** Map event type prefixes to tRPC router-level query key prefixes. */
function invalidateForEvent(
  queryClient: ReturnType<typeof useQueryClient>,
  eventType: string,
) {
  switch (eventType) {
    case "obligation":
      void queryClient.invalidateQueries({
        queryKey: trpc.obligation.list.queryKey(),
      });
      void queryClient.invalidateQueries({
        queryKey: trpc.obligation.stats.queryKey(),
      });
      void queryClient.invalidateQueries({
        queryKey: trpc.system.activityFeed.queryKey(),
      });
      break;
    case "message":
      void queryClient.invalidateQueries({
        queryKey: trpc.message.list.queryKey(),
      });
      void queryClient.invalidateQueries({
        queryKey: trpc.system.activityFeed.queryKey(),
      });
      break;
    case "session":
      void queryClient.invalidateQueries({
        queryKey: trpc.session.list.queryKey(),
      });
      void queryClient.invalidateQueries({
        queryKey: trpc.system.activityFeed.queryKey(),
      });
      break;
    case "approval":
      void queryClient.invalidateQueries({
        queryKey: trpc.obligation.list.queryKey(),
      });
      void queryClient.invalidateQueries({
        queryKey: trpc.system.activityFeed.queryKey(),
      });
      break;
    case "briefing":
      void queryClient.invalidateQueries({
        queryKey: trpc.briefing.latest.queryKey(),
      });
      void queryClient.invalidateQueries({
        queryKey: trpc.briefing.history.queryKey(),
      });
      break;
    case "fleet":
      void queryClient.invalidateQueries({
        queryKey: trpc.system.fleetStatus.queryKey(),
      });
      break;
    case "diary":
      void queryClient.invalidateQueries({
        queryKey: trpc.system.activityFeed.queryKey(),
      });
      void queryClient.invalidateQueries({
        queryKey: trpc.diary.list.queryKey(),
      });
      break;
    default:
      // Unknown events: invalidate activity feed as catch-all
      void queryClient.invalidateQueries({
        queryKey: trpc.system.activityFeed.queryKey(),
      });
      break;
  }
}

export default function QueryInvalidationBridge() {
  const queryClient = useQueryClient();

  useDaemonEvents((event) => {
    invalidateForEvent(queryClient, event.type);
  });

  return null;
}
