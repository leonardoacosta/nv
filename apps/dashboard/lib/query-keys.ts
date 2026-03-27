/**
 * Query key factory for the Nova dashboard.
 *
 * Key convention: ["api", path, params?]
 *
 * The "api" prefix namespace ensures no collisions with future tRPC keys,
 * which use [["router", "procedure"], { input }] shape.
 *
 * === tRPC Migration Notes ===
 *
 * When migrating a query to tRPC, replace:
 *   queryKey: queryKeys.api("/api/obligations")
 * with:
 *   queryKey: trpc.obligation.getAll.queryKey()
 *
 * For invalidation, replace:
 *   queryClient.invalidateQueries({ queryKey: queryKeys.api("/api/obligations") })
 * with:
 *   queryClient.invalidateQueries({ queryKey: trpc.obligation.getAll.queryKey() })
 *
 * The ["api"] prefix allows bulk invalidation of all REST queries:
 *   queryClient.invalidateQueries({ queryKey: ["api"] })
 */

export const queryKeys = {
  /** Root key for all REST API queries. Use for bulk invalidation. */
  all: ["api"] as const,

  /**
   * Build a query key for a specific API path.
   *
   * @param path - API path (e.g. "/api/obligations")
   * @param params - Optional URL search params for cache separation
   * @returns Tuple like ["api", "/api/obligations"] or ["api", "/api/sessions", { page: "1" }]
   */
  api: (path: string, params?: Record<string, string>) =>
    params
      ? (["api", path, params] as const)
      : (["api", path] as const),
} as const;
