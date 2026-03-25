"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { Terminal, RotateCcw, MessageSquare, ArrowRight, AlertCircle } from "lucide-react";
import type { SessionStatus, SessionState } from "@/lib/session-manager";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
      classes: "bg-emerald-500/20 text-emerald-400",
      dotClass: "bg-emerald-400",
    },
    idle: {
      label: "Idle",
      classes: "bg-amber-500/20 text-amber-400",
      dotClass: "bg-amber-400",
    },
    starting: {
      label: "Starting",
      classes: "bg-amber-500/20 text-amber-400",
      dotClass: "bg-amber-400 animate-pulse",
    },
    stopping: {
      label: "Stopping",
      classes: "bg-amber-500/20 text-amber-400",
      dotClass: "bg-amber-400 animate-pulse",
    },
    stopped: {
      label: "Stopped",
      classes: "bg-cosmic-muted/20 text-cosmic-muted",
      dotClass: "bg-cosmic-muted",
    },
    error: {
      label: "Error",
      classes: "bg-cosmic-rose/20 text-cosmic-rose",
      dotClass: "bg-cosmic-rose",
    },
  };

  const { label, classes, dotClass } = config[state] ?? config.stopped;

  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-medium font-mono ${classes}`}
    >
      <span className={`w-1.5 h-1.5 rounded-full ${dotClass}`} />
      {label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// SessionWidget
// ---------------------------------------------------------------------------

export default function SessionWidget() {
  const [status, setStatus] = useState<SessionStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [restarting, setRestarting] = useState(false);

  const fetchStatus = async () => {
    try {
      const res = await fetch("/api/session/status");
      if (!res.ok) return;
      const data = (await res.json()) as SessionStatus;
      setStatus(data);
    } catch {
      // Silently ignore
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchStatus();
    const interval = setInterval(() => void fetchStatus(), 5000);
    return () => clearInterval(interval);
  }, []);

  const handleRestart = async () => {
    setRestarting(true);
    try {
      await fetch("/api/session/control", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ action: "restart" }),
      });
      // Refresh status after restart request
      await fetchStatus();
    } catch {
      // Silently ignore
    } finally {
      setRestarting(false);
    }
  };

  if (loading) {
    return (
      <div className="p-5 rounded-cosmic bg-cosmic-surface border border-cosmic-border animate-pulse">
        <div className="h-4 w-32 rounded bg-cosmic-border mb-3" />
        <div className="h-4 w-48 rounded bg-cosmic-border" />
      </div>
    );
  }

  const state = status?.state ?? "stopped";
  const canRestart = state === "active" || state === "idle" || state === "error";

  return (
    <div className="p-5 rounded-cosmic bg-cosmic-surface border border-cosmic-border hover:border-cosmic-purple/50 transition-colors">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2.5">
          <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-cosmic-purple/20 border border-cosmic-purple/30">
            <Terminal size={16} className="text-cosmic-purple" />
          </div>
          <div>
            <p className="text-sm font-medium text-cosmic-bright">CC Session</p>
            <p className="text-xs text-cosmic-muted font-mono">nova-cc-session</p>
          </div>
        </div>
        <StateBadge state={state} />
      </div>

      {/* Error message */}
      {state === "error" && status?.error_message && (
        <div className="flex items-start gap-2 mb-4 p-2.5 rounded-lg bg-cosmic-rose/10 border border-cosmic-rose/20 text-xs text-cosmic-rose font-mono">
          <AlertCircle size={11} className="mt-0.5 shrink-0" />
          <span className="break-all">{status.error_message}</span>
        </div>
      )}

      {/* Stats */}
      <div className="flex items-center gap-4 text-xs text-cosmic-muted font-mono mb-4">
        <div className="flex items-center gap-1.5">
          <MessageSquare size={11} />
          <span>{status?.message_count ?? 0} msgs</span>
        </div>
        <div className="text-cosmic-border">·</div>
        <div suppressHydrationWarning>
          {formatRelativeTime(status?.last_message_at ?? null)}
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center justify-between">
        <button
          type="button"
          onClick={() => void handleRestart()}
          disabled={!canRestart || restarting}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-cosmic-purple/20 text-cosmic-purple border border-cosmic-purple/30 hover:bg-cosmic-purple/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          <RotateCcw size={11} className={restarting ? "animate-spin" : ""} />
          Restart
        </button>

        <Link
          href="/session"
          className="flex items-center gap-1 text-xs text-cosmic-muted hover:text-cosmic-text transition-colors"
        >
          Manage
          <ArrowRight size={12} />
        </Link>
      </div>
    </div>
  );
}
