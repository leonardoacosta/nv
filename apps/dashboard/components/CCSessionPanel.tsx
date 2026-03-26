"use client";

import { useEffect, useState } from "react";
import SessionDashboard from "@/components/SessionDashboard";
import type { SessionStatus } from "@/lib/session-manager";

export default function CCSessionPanel() {
  const [initialStatus, setInitialStatus] = useState<SessionStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchStatus = async () => {
      try {
        const res = await fetch("/api/session/status");
        if (!res.ok) throw new Error(`Failed to fetch status: ${res.status}`);
        const data = (await res.json()) as SessionStatus;
        setInitialStatus(data);
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Failed to load session status",
        );
      } finally {
        setLoading(false);
      }
    };

    void fetchStatus();
  }, []);

  if (loading) {
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
        <p className="text-copy-14 text-red-700">{error}</p>
      </div>
    );
  }

  return (
    <div className="animate-fade-in-up">
      <SessionDashboard initialStatus={initialStatus} />
    </div>
  );
}
