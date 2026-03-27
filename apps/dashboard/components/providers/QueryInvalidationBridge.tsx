"use client";

import { useQueryClient } from "@tanstack/react-query";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";
import { queryKeys } from "@/lib/query-keys";

/**
 * QueryInvalidationBridge — listens to WebSocket events from the daemon
 * and invalidates relevant TanStack Query caches.
 *
 * This replaces the previous pattern of manually calling fetchData()
 * on WebSocket events. Instead, query invalidation triggers automatic
 * refetches for any active queries with matching keys.
 *
 * Renders nothing — pure side-effect component.
 */

/** Map event type prefixes to query paths that should be invalidated. */
const EVENT_TO_QUERIES: Record<string, string[]> = {
  obligation: ["/api/obligations", "/api/activity-feed"],
  message: ["/api/messages", "/api/activity-feed"],
  session: ["/api/sessions", "/api/activity-feed"],
  approval: ["/api/obligations", "/api/activity-feed"],
  briefing: ["/api/briefing"],
  fleet: ["/api/fleet-status"],
  diary: ["/api/activity-feed"],
};

export default function QueryInvalidationBridge() {
  const queryClient = useQueryClient();

  useDaemonEvents((event) => {
    // Find matching query paths for this event type
    const paths = EVENT_TO_QUERIES[event.type];

    if (paths) {
      for (const path of paths) {
        queryClient.invalidateQueries({ queryKey: queryKeys.api(path) });
      }
    } else {
      // For unknown event types, invalidate the activity feed as a catch-all
      queryClient.invalidateQueries({ queryKey: queryKeys.api("/api/activity-feed") });
    }
  });

  return null;
}
