"use client";

import { useEffect, useState } from "react";
import SessionDashboard from "@/components/SessionDashboard";
import type { SessionStatus } from "@/lib/session-manager";

export default function SessionPage() {
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
        setError(err instanceof Error ? err.message : "Failed to load session status");
      } finally {
        setLoading(false);
      }
    };

    void fetchStatus();
  }, []);

  if (loading) {
    return (
      <div className="p-8 max-w-4xl">
        <div className="mb-8">
          <div className="h-7 w-48 animate-pulse rounded-lg bg-cosmic-surface" />
          <div className="mt-2 h-4 w-64 animate-pulse rounded bg-cosmic-surface" />
        </div>
        <div className="space-y-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="h-24 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border" />
          ))}
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-8 max-w-4xl">
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose text-sm">
          {error}
        </div>
      </div>
    );
  }

  return (
    <div className="p-8 max-w-4xl">
      <div className="mb-8">
        <h1 className="text-2xl font-semibold text-cosmic-bright">CC Session</h1>
        <p className="mt-1 text-sm text-cosmic-muted">
          Manage the Claude Code container session
        </p>
      </div>
      <SessionDashboard initialStatus={initialStatus} />
    </div>
  );
}
