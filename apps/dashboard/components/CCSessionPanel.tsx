"use client";

import { useQuery } from "@tanstack/react-query";
import SessionDashboard from "@/components/SessionDashboard";
import type { SessionStatus } from "@/lib/session-manager";
import { useTRPC } from "@/lib/trpc/react";

export default function CCSessionPanel() {
  const trpc = useTRPC();
  const { data, isLoading, error } = useQuery(
    trpc.ccSession.status.queryOptions(),
  );
  const initialStatus = (data as SessionStatus | undefined) ?? null;

  if (isLoading) {
    return (
      <div className="space-y-4 animate-fade-in-up">
        {Array.from({ length: 4 }).map((_, i) => (
          <div
            key={i}
            className="h-24 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400"
          />
        ))}
      </div>
    );
  }

  if (error) {
    return (
      <div
        className="flex items-start gap-3 p-4 rounded-md"
        style={{
          background: "rgba(229, 72, 77, 0.08)",
          borderLeft: "3px solid var(--ds-red-700)",
        }}
      >
        <p className="text-copy-14 text-red-700">{error.message}</p>
      </div>
    );
  }

  return (
    <div className="animate-fade-in-up">
      <SessionDashboard initialStatus={initialStatus} />
    </div>
  );
}
