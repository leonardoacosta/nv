"use client";

import {
  useQuery,
  useMutation,
  type UseQueryOptions,
  type UseQueryResult,
  type UseMutationOptions,
  type UseMutationResult,
} from "@tanstack/react-query";

import { apiFetch } from "@/lib/api-client";
import { queryKeys } from "@/lib/query-keys";

/**
 * Wrapper around useQuery that fetches from the Nova API via apiFetch.
 *
 * Handles JSON parsing and error extraction. Uses the ["api", path, params?]
 * key convention for cache management.
 *
 * @param path - API path (e.g. "/api/obligations")
 * @param options - Query options
 * @returns Standard UseQueryResult<T>
 *
 * === tRPC Migration ===
 * Replace:
 *   useApiQuery<Obligations>("/api/obligations")
 * with:
 *   useQuery(trpc.obligation.getAll.queryOptions())
 */
export function useApiQuery<T>(
  path: string,
  options?: {
    params?: Record<string, string>;
    enabled?: boolean;
    refetchInterval?: number | false;
    select?: (data: T) => unknown;
    staleTime?: number;
  },
): UseQueryResult<T> {
  const url = options?.params
    ? `${path}?${new URLSearchParams(options.params).toString()}`
    : path;

  const queryKey = queryKeys.api(path, options?.params);

  return useQuery<T>({
    queryKey,
    queryFn: async (): Promise<T> => {
      const response = await apiFetch(url);
      if (!response.ok) {
        const body = await response.text();
        let message: string;
        try {
          const parsed = JSON.parse(body) as { error?: string; message?: string };
          message = parsed.error ?? parsed.message ?? `Request failed: ${response.status}`;
        } catch {
          message = body || `Request failed: ${response.status}`;
        }
        throw new Error(message);
      }
      return response.json() as Promise<T>;
    },
    enabled: options?.enabled,
    refetchInterval: options?.refetchInterval,
    select: options?.select as ((data: T) => T) | undefined,
    staleTime: options?.staleTime,
  });
}

/**
 * Wrapper around useMutation that posts/puts/patches/deletes via apiFetch.
 *
 * @param path - API path (e.g. "/api/obligations")
 * @param options - Mutation options including HTTP method
 * @returns Standard UseMutationResult
 *
 * === tRPC Migration ===
 * Replace:
 *   useApiMutation<Obligation, CreateObligationInput>("/api/obligations", { ... })
 * with:
 *   useMutation(trpc.obligation.create.mutationOptions({ ... }))
 */
export function useApiMutation<TData, TVariables>(
  path: string,
  options?: {
    method?: "POST" | "PUT" | "PATCH" | "DELETE";
    onSuccess?: UseMutationOptions<TData, Error, TVariables>["onSuccess"];
    onError?: UseMutationOptions<TData, Error, TVariables>["onError"];
    onSettled?: UseMutationOptions<TData, Error, TVariables>["onSettled"];
  },
): UseMutationResult<TData, Error, TVariables> {
  const method = options?.method ?? "POST";

  return useMutation<TData, Error, TVariables>({
    mutationFn: async (variables: TVariables): Promise<TData> => {
      const response = await apiFetch(path, {
        method,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(variables),
      });
      if (!response.ok) {
        const body = await response.text();
        let message: string;
        try {
          const parsed = JSON.parse(body) as { error?: string; message?: string };
          message = parsed.error ?? parsed.message ?? `Request failed: ${response.status}`;
        } catch {
          message = body || `Request failed: ${response.status}`;
        }
        throw new Error(message);
      }
      return response.json() as Promise<TData>;
    },
    onSuccess: options?.onSuccess,
    onError: options?.onError,
    onSettled: options?.onSettled,
  });
}
