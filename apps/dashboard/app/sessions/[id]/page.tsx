"use client";

import { useState } from "react";
import { useParams, useSearchParams } from "next/navigation";
import Link from "next/link";
import {
  ArrowLeft,
  Clock,
  Layers,
  MessageSquare,
  RefreshCw,
  Terminal,
  Zap,
} from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import SessionTimelineEvent from "@/components/SessionTimelineEvent";
import type { SessionEventItem } from "@/types/api";
import { useTRPC } from "@/lib/trpc/react";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function elapsed(startIso: string, endIso?: string | null): string {
  const start = new Date(startIso).getTime();
  const end = endIso ? new Date(endIso).getTime() : Date.now();
  const diffMs = end - start;
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 1) return "<1m";
  if (diffMin < 60) return `${diffMin}m`;
  const h = Math.floor(diffMin / 60);
  return `${h}h ${diffMin % 60}m`;
}

const STATUS_DOT: Record<string, string> = {
  running: "bg-green-700 animate-pulse",
  active: "bg-green-700 animate-pulse",
  completed: "bg-ds-gray-600",
  stopped: "bg-amber-700",
  idle: "bg-amber-700",
};

const STATUS_LABEL: Record<string, string> = {
  running: "Running",
  active: "Active",
  completed: "Completed",
  stopped: "Stopped",
  idle: "Idle",
};

// ---------------------------------------------------------------------------
// StatTile
// ---------------------------------------------------------------------------

function StatTile({
  icon: Icon,
  label,
  value,
}: {
  icon: React.ElementType;
  label: string;
  value: string | number;
}) {
  return (
    <div className="flex items-center gap-3 p-4 rounded-xl bg-ds-gray-100 border border-ds-gray-400">
      <div className="flex items-center justify-center size-9 rounded-lg shrink-0 bg-ds-gray-alpha-200">
        <Icon size={18} className="text-ds-gray-1000" />
      </div>
      <div className="min-w-0">
        <p className="text-label-12 text-ds-gray-900 uppercase tracking-wide truncate">
          {label}
        </p>
        <p className="text-heading-16 font-mono text-ds-gray-1000">{value}</p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// SessionDetailPage
// ---------------------------------------------------------------------------

export default function SessionDetailPage() {
  const trpc = useTRPC();
  const params = useParams<{ id: string }>();
  const searchParams = useSearchParams();
  const queryClient = useQueryClient();
  const sessionId = params.id;

  // 1. Queries
  const sessionQuery = useQuery(
    trpc.session.getById.queryOptions({ id: sessionId }),
  );
  const eventsQuery = useQuery(
    trpc.session.getEvents.queryOptions({ id: sessionId }),
  );

  const session = sessionQuery.data ?? null;
  const events = (eventsQuery.data?.events ?? []) as SessionEventItem[];
  const loading = sessionQuery.isLoading;
  const error = sessionQuery.error?.message ?? null;

  const fetchData = () => {
    void queryClient.invalidateQueries({ queryKey: trpc.session.getById.queryKey() });
    void queryClient.invalidateQueries({ queryKey: trpc.session.getEvents.queryKey() });
  };

  // 4. Build "Back to Sessions" link preserving filter state (task 3.9)
  const backParams = new URLSearchParams();
  for (const [key, value] of searchParams.entries()) {
    if (key !== "id") backParams.set(key, value);
  }
  // Preserve original filter params from the sessions list page
  const backUrl =
    backParams.toString() ? `/sessions?${backParams.toString()}` : "/sessions";

  // 5. Action slot
  const action = (
    <div className="flex items-center gap-2">
      <Link
        href={backUrl}
        className="flex items-center gap-2 px-3 py-2 min-h-11 rounded-lg text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors"
      >
        <ArrowLeft size={14} />
        <span className="hidden sm:inline">Back to Sessions</span>
      </Link>
      <button
        type="button"
        onClick={() => void fetchData()}
        disabled={loading}
        className="flex items-center gap-2 px-3 py-2 min-h-11 rounded-lg text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
      >
        <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
      </button>
    </div>
  );

  // 6. Loading skeleton
  if (loading) {
    return (
      <PageShell title="Session" action={action}>
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <div
                key={i}
                className="h-20 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
              />
            ))}
          </div>
          <div className="h-64 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400" />
        </div>
      </PageShell>
    );
  }

  // 7. Error state
  if (error) {
    return (
      <PageShell title="Session" action={action}>
        <ErrorBanner message={error} onRetry={() => void fetchData()} />
      </PageShell>
    );
  }

  // 8. Not found
  if (!session) {
    return (
      <PageShell title="Session" action={action}>
        <div className="flex flex-col items-center gap-3 py-16 text-ds-gray-900">
          <Layers size={32} />
          <p className="text-copy-13">Session not found</p>
        </div>
      </PageShell>
    );
  }

  const statusDot = STATUS_DOT[session.status] ?? "bg-ds-gray-600";
  const statusLabel = STATUS_LABEL[session.status] ?? session.status;

  return (
    <PageShell
      title="Session"
      subtitle={session.id}
      action={action}
    >
      {/* Status row */}
      <div className="flex items-center gap-3 mb-3 flex-wrap">
        <span className="flex items-center gap-1.5 text-copy-13">
          <span className={`inline-block size-2 rounded-full shrink-0 ${statusDot}`} />
          <span className="text-label-13 text-ds-gray-1000">{statusLabel}</span>
        </span>
        <span className="text-copy-13 font-mono px-2 py-0.5 rounded bg-ds-gray-100 border border-ds-gray-400 text-ds-gray-900">
          {session.project}
        </span>
        {session.trigger_type && (
          <span className="inline-flex items-center px-2 py-0.5 rounded-full text-label-12 font-medium capitalize bg-ds-gray-alpha-200 text-ds-gray-1000">
            {session.trigger_type}
          </span>
        )}
      </div>

      {/* Stat tiles */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
        <StatTile
          icon={MessageSquare}
          label="Messages"
          value={session.message_count}
        />
        <StatTile
          icon={Terminal}
          label="Tools"
          value={session.tool_count}
        />
        <StatTile
          icon={Clock}
          label="Duration"
          value={elapsed(session.started_at, session.ended_at)}
        />
        <StatTile
          icon={Zap}
          label="Service"
          value={session.service}
        />
      </div>

      {/* Timeline of events */}
      <div className="flex flex-col gap-0">
        <div className="flex items-center gap-2 mb-3">
          <h2 className="text-label-14 font-semibold text-ds-gray-1000">
            Timeline
          </h2>
          <span className="inline-flex items-center justify-center px-1.5 py-0.5 min-w-[1.25rem] rounded text-xs font-mono font-medium text-ds-gray-900 bg-ds-gray-alpha-200">
            {events.length}
          </span>
        </div>

        {events.length === 0 ? (
          /* Empty state (task 3.7) */
          <div className="flex flex-col items-center gap-3 py-12 text-ds-gray-900">
            <Layers size={28} className="text-ds-gray-600" />
            <p className="text-copy-13">
              No interactions recorded for this session
            </p>
          </div>
        ) : (
          <div className="pl-1">
            {events.map((event) => (
              <SessionTimelineEvent key={event.id} event={event} />
            ))}
          </div>
        )}
      </div>
    </PageShell>
  );
}
