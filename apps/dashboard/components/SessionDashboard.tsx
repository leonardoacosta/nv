"use client";

import { useEffect, useRef, useState } from "react";
import {
  Play,
  Square,
  RotateCcw,
  AlertCircle,
  Clock,
  MessageSquare,
  RefreshCw,
  Terminal,
} from "lucide-react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import type { SessionStatus, SessionState } from "@/lib/session-manager";
import { useTRPC } from "@/lib/trpc/react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface SessionDashboardProps {
  initialStatus: SessionStatus | null;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatUptime(secs: number | null): string {
  if (secs === null) return "—";
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function formatRelativeTime(iso: string | null): string {
  if (!iso) return "Never";
  const diffMs = Date.now() - new Date(iso).getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  if (diffSecs < 60) return `${diffSecs}s ago`;
  const diffMins = Math.floor(diffSecs / 60);
  if (diffMins < 60) return `${diffMins}m ago`;
  const diffHrs = Math.floor(diffMins / 60);
  return `${diffHrs}h ago`;
}

function StateBadge({ state }: { state: SessionState }) {
  const config: Record<SessionState, { label: string; classes: string; dotClass: string }> = {
    active: {
      label: "Active",
      classes: "bg-green-700/10 text-green-700 border-green-700/20",
      dotClass: "bg-green-700",
    },
    idle: {
      label: "Idle",
      classes: "bg-amber-700/10 text-amber-700 border-amber-700/20",
      dotClass: "bg-amber-700",
    },
    starting: {
      label: "Starting",
      classes: "bg-amber-700/10 text-amber-700 border-amber-700/20",
      dotClass: "bg-amber-700 animate-pulse",
    },
    stopping: {
      label: "Stopping",
      classes: "bg-amber-700/10 text-amber-700 border-amber-700/20",
      dotClass: "bg-amber-700 animate-pulse",
    },
    stopped: {
      label: "Stopped",
      classes: "bg-ds-gray-alpha-200 text-ds-gray-900 border-ds-gray-400",
      dotClass: "bg-ds-gray-600",
    },
    error: {
      label: "Error",
      classes: "bg-red-700/10 text-red-700 border-red-700/20",
      dotClass: "bg-red-700",
    },
  };

  const { label, classes, dotClass } = config[state] ?? config.stopped;

  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium font-mono border ${classes}`}
    >
      <span className={`w-1.5 h-1.5 rounded-full ${dotClass}`} />
      {label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// LogViewer sub-component
// ---------------------------------------------------------------------------

function LogViewer() {
  const trpc = useTRPC();
  const { data, error: logsQueryError } = useQuery(
    trpc.ccSession.logs.queryOptions(
      { lines: 50 },
      { refetchInterval: 5000 },
    ),
  );
  const lines = data?.lines ?? [];
  const logsError = logsQueryError?.message ?? null;
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new lines arrive
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [lines]);

  if (logsError) {
    return (
      <div className="flex items-center gap-2 p-3 text-xs text-red-700 font-mono bg-red-700/10 rounded-lg border border-red-700/30">
        <AlertCircle size={12} />
        {logsError}
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="h-64 overflow-y-auto surface-inset text-label-13-mono text-ds-gray-1000 p-3 space-y-0.5"
    >
      {lines.length === 0 ? (
        <span className="text-ds-gray-900">No log output yet…</span>
      ) : (
        lines.map((line, i) => (
          <div key={i} className="leading-5 whitespace-pre-wrap break-all">
            {line}
          </div>
        ))
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// SessionDashboard main component
// ---------------------------------------------------------------------------

export default function SessionDashboard({ initialStatus }: SessionDashboardProps) {
  const trpc = useTRPC();
  const queryClient = useQueryClient();
  const [actionPending, setActionPending] = useState<"start" | "stop" | "restart" | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  // Auto-refresh status every 5s via tRPC
  const statusQuery = useQuery(
    trpc.ccSession.status.queryOptions(undefined, { refetchInterval: 5000 }),
  );
  const status = (statusQuery.data as SessionStatus | undefined) ?? initialStatus;

  const controlMutation = useMutation(
    trpc.ccSession.control.mutationOptions({
      onSuccess: () => {
        void queryClient.invalidateQueries({ queryKey: trpc.ccSession.status.queryKey() });
      },
    }),
  );

  const sendControl = async (action: "start" | "stop" | "restart") => {
    setActionPending(action);
    setActionError(null);
    try {
      await controlMutation.mutateAsync({ action });
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Request failed");
    } finally {
      setActionPending(null);
    }
  };

  const state = status?.state ?? "stopped";
  const isTransitioning = state === "starting" || state === "stopping" || actionPending !== null;
  const canStart = state === "stopped" || state === "error";
  const canStop = state === "active" || state === "idle";
  const canRestart = state === "active" || state === "idle" || state === "error";

  const accentBar =
    state === "active"
      ? "bg-green-700"
      : state === "idle" || state === "starting" || state === "stopping"
        ? "bg-amber-700"
        : state === "error"
          ? "bg-red-700"
          : "bg-ds-gray-600";

  return (
    <div className="space-y-4">
      {/* Status card — surface-card with accent bar */}
      <div className="surface-card relative p-6 space-y-5 overflow-hidden">
        {/* Left accent bar */}
        <div
          className={`absolute left-0 top-0 bottom-0 w-1 ${accentBar} rounded-l-xl`}
          aria-hidden="true"
        />

        {/* Header row */}
        <div className="flex items-center justify-between pl-2">
          <div className="flex items-center gap-3">
            <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-ds-gray-alpha-200 border border-ds-gray-1000/30">
              <Terminal size={18} className="text-ds-gray-1000" />
            </div>
            <div>
              <p className="text-label-14 text-ds-gray-1000">CC Session</p>
              <p className="text-label-13-mono text-ds-gray-900">nova-cc-session</p>
            </div>
          </div>
          <StateBadge state={state} />
        </div>

        {/* Stats row — surface-inset tiles */}
        <div className="grid grid-cols-3 gap-3 pl-2">
          <div className="surface-inset flex items-center gap-2.5 p-3">
            <Clock size={14} className="text-ds-gray-700 shrink-0" />
            <div>
              <p className="text-label-12 text-ds-gray-900">Uptime</p>
              <p className="text-label-13-mono text-ds-gray-1000" suppressHydrationWarning>
                {formatUptime(status?.uptime_secs ?? null)}
              </p>
            </div>
          </div>

          <div className="surface-inset flex items-center gap-2.5 p-3">
            <MessageSquare size={14} className="text-ds-gray-700 shrink-0" />
            <div>
              <p className="text-label-12 text-ds-gray-900">Messages</p>
              <p className="text-label-13-mono text-ds-gray-1000">
                {status?.message_count ?? 0}
              </p>
            </div>
          </div>

          <div className="surface-inset flex items-center gap-2.5 p-3">
            <RefreshCw size={14} className="text-ds-gray-700 shrink-0" />
            <div>
              <p className="text-label-12 text-ds-gray-900">Last Activity</p>
              <p className="text-label-13-mono text-ds-gray-1000" suppressHydrationWarning>
                {formatRelativeTime(status?.last_message_at ?? null)}
              </p>
            </div>
          </div>
        </div>

        {/* Restart count info */}
        {(status?.restart_count ?? 0) > 0 && (
          <div className="flex items-center gap-2 text-label-13-mono text-amber-700 pl-2">
            <RotateCcw size={12} />
            Auto-restarted {status?.restart_count} time{status?.restart_count !== 1 ? "s" : ""}
          </div>
        )}

        {/* Controls */}
        <div className="flex items-center gap-3 pt-1 pl-2">
          <button
            type="button"
            onClick={() => void sendControl("start")}
            disabled={!canStart || isTransitioning}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-button-14 font-medium bg-green-700/10 text-green-700 border border-green-700/20 hover:bg-green-700/20 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Play size={14} />
            Start
          </button>

          <button
            type="button"
            onClick={() => void sendControl("stop")}
            disabled={!canStop || isTransitioning}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-button-14 font-medium bg-red-700/10 text-red-700 border border-red-700/20 hover:bg-red-700/20 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Square size={14} />
            Stop
          </button>

          <button
            type="button"
            onClick={() => void sendControl("restart")}
            disabled={!canRestart || isTransitioning}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-button-14 font-medium bg-ds-gray-alpha-200 text-ds-gray-1000 border border-ds-gray-1000/30 hover:bg-ds-gray-300/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <RotateCcw size={14} className={actionPending === "restart" ? "animate-spin" : ""} />
            Restart
          </button>

          {isTransitioning && actionPending && (
            <span className="text-label-13-mono text-ds-gray-900 animate-pulse">
              {actionPending}ing…
            </span>
          )}
        </div>
      </div>

      {/* Action error */}
      {actionError && (
        <div className="flex items-center gap-3 p-4 rounded-xl bg-red-700/10 border border-red-700/30 text-red-700 text-copy-14">
          <AlertCircle size={15} />
          {actionError}
        </div>
      )}

      {/* Session error panel */}
      {state === "error" && status?.error_message && (
        <div
          className="p-4 rounded-md space-y-2"
          style={{
            background: "rgba(229, 72, 77, 0.08)",
            borderLeft: "3px solid var(--ds-red-700)",
          }}
        >
          <div className="flex items-center gap-2 text-label-14 font-medium text-red-700">
            <AlertCircle size={15} />
            Session Error
          </div>
          <p className="text-label-13-mono text-red-700/80 leading-relaxed">
            {status.error_message}
          </p>
        </div>
      )}

      {/* Log viewer */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Terminal size={14} className="text-ds-gray-700" />
          <span className="text-label-12 text-ds-gray-700">Container Logs</span>
        </div>
        <LogViewer />
      </div>
    </div>
  );
}
