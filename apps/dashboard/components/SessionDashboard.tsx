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
import type { SessionStatus, SessionState } from "@/lib/session-manager";

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
      classes: "bg-emerald-500/20 text-emerald-400 border-emerald-500/30",
      dotClass: "bg-emerald-400",
    },
    idle: {
      label: "Idle",
      classes: "bg-amber-500/20 text-amber-400 border-amber-500/30",
      dotClass: "bg-amber-400",
    },
    starting: {
      label: "Starting",
      classes: "bg-amber-500/20 text-amber-400 border-amber-500/30",
      dotClass: "bg-amber-400 animate-pulse",
    },
    stopping: {
      label: "Stopping",
      classes: "bg-amber-500/20 text-amber-400 border-amber-500/30",
      dotClass: "bg-amber-400 animate-pulse",
    },
    stopped: {
      label: "Stopped",
      classes: "bg-cosmic-muted/20 text-cosmic-muted border-cosmic-muted/30",
      dotClass: "bg-cosmic-muted",
    },
    error: {
      label: "Error",
      classes: "bg-cosmic-rose/20 text-cosmic-rose border-cosmic-rose/30",
      dotClass: "bg-cosmic-rose",
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
  const [lines, setLines] = useState<string[]>([]);
  const [logsError, setLogsError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const fetchLogs = async () => {
      try {
        const res = await fetch("/api/session/logs");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data = (await res.json()) as { lines: string[] };
        setLines(data.lines);
        setLogsError(null);
      } catch (err) {
        setLogsError(err instanceof Error ? err.message : "Failed to load logs");
      }
    };

    void fetchLogs();
    const interval = setInterval(() => void fetchLogs(), 5000);
    return () => clearInterval(interval);
  }, []);

  // Auto-scroll to bottom when new lines arrive
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [lines]);

  if (logsError) {
    return (
      <div className="flex items-center gap-2 p-3 text-xs text-cosmic-rose font-mono bg-cosmic-rose/10 rounded-lg border border-cosmic-rose/30">
        <AlertCircle size={12} />
        {logsError}
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="h-64 overflow-y-auto rounded-lg bg-cosmic-dark border border-cosmic-border font-mono text-xs text-cosmic-text p-3 space-y-0.5"
    >
      {lines.length === 0 ? (
        <span className="text-cosmic-muted">No log output yet…</span>
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
  const [status, setStatus] = useState<SessionStatus | null>(initialStatus);
  const [actionPending, setActionPending] = useState<"start" | "stop" | "restart" | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  // Auto-refresh status every 5s
  useEffect(() => {
    const fetchStatus = async () => {
      try {
        const res = await fetch("/api/session/status");
        if (!res.ok) return;
        const data = (await res.json()) as SessionStatus;
        setStatus(data);
      } catch {
        // Silently ignore polling errors
      }
    };

    const interval = setInterval(() => void fetchStatus(), 5000);
    return () => clearInterval(interval);
  }, []);

  const sendControl = async (action: "start" | "stop" | "restart") => {
    setActionPending(action);
    setActionError(null);
    try {
      const res = await fetch("/api/session/control", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ action }),
      });
      const data = (await res.json()) as { status?: SessionStatus; error?: string };
      if (!res.ok) {
        setActionError(data.error ?? `Action failed: HTTP ${res.status}`);
      } else if (data.status) {
        setStatus(data.status);
      }
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

  return (
    <div className="space-y-6">
      {/* Status card */}
      <div className="p-6 rounded-cosmic bg-cosmic-surface border border-cosmic-border space-y-5">
        {/* Header row */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-cosmic-purple/20 border border-cosmic-purple/30">
              <Terminal size={18} className="text-cosmic-purple" />
            </div>
            <div>
              <p className="text-sm font-medium text-cosmic-bright">CC Session</p>
              <p className="text-xs text-cosmic-muted font-mono">nova-cc-session</p>
            </div>
          </div>
          <StateBadge state={state} />
        </div>

        {/* Stats row */}
        <div className="grid grid-cols-3 gap-4">
          <div className="flex items-center gap-2.5 p-3 rounded-lg bg-cosmic-dark border border-cosmic-border">
            <Clock size={14} className="text-cosmic-muted shrink-0" />
            <div>
              <p className="text-xs text-cosmic-muted uppercase tracking-wide">Uptime</p>
              <p className="text-sm font-mono font-medium text-cosmic-bright" suppressHydrationWarning>
                {formatUptime(status?.uptime_secs ?? null)}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2.5 p-3 rounded-lg bg-cosmic-dark border border-cosmic-border">
            <MessageSquare size={14} className="text-cosmic-muted shrink-0" />
            <div>
              <p className="text-xs text-cosmic-muted uppercase tracking-wide">Messages</p>
              <p className="text-sm font-mono font-medium text-cosmic-bright">
                {status?.message_count ?? 0}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2.5 p-3 rounded-lg bg-cosmic-dark border border-cosmic-border">
            <RefreshCw size={14} className="text-cosmic-muted shrink-0" />
            <div>
              <p className="text-xs text-cosmic-muted uppercase tracking-wide">Last Activity</p>
              <p className="text-sm font-mono font-medium text-cosmic-bright" suppressHydrationWarning>
                {formatRelativeTime(status?.last_message_at ?? null)}
              </p>
            </div>
          </div>
        </div>

        {/* Restart count info */}
        {(status?.restart_count ?? 0) > 0 && (
          <div className="flex items-center gap-2 text-xs text-amber-400 font-mono">
            <RotateCcw size={12} />
            Auto-restarted {status?.restart_count} time{status?.restart_count !== 1 ? "s" : ""}
          </div>
        )}

        {/* Controls */}
        <div className="flex items-center gap-3 pt-1">
          <button
            type="button"
            onClick={() => void sendControl("start")}
            disabled={!canStart || isTransitioning}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 hover:bg-emerald-500/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Play size={14} />
            Start
          </button>

          <button
            type="button"
            onClick={() => void sendControl("stop")}
            disabled={!canStop || isTransitioning}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium bg-cosmic-rose/20 text-cosmic-rose border border-cosmic-rose/30 hover:bg-cosmic-rose/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Square size={14} />
            Stop
          </button>

          <button
            type="button"
            onClick={() => void sendControl("restart")}
            disabled={!canRestart || isTransitioning}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium bg-cosmic-purple/20 text-cosmic-purple border border-cosmic-purple/30 hover:bg-cosmic-purple/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <RotateCcw size={14} className={actionPending === "restart" ? "animate-spin" : ""} />
            Restart
          </button>

          {isTransitioning && actionPending && (
            <span className="text-xs text-cosmic-muted font-mono animate-pulse">
              {actionPending}ing…
            </span>
          )}
        </div>
      </div>

      {/* Action error */}
      {actionError && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose text-sm">
          <AlertCircle size={15} />
          {actionError}
        </div>
      )}

      {/* Session error panel */}
      {state === "error" && status?.error_message && (
        <div className="p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 space-y-2">
          <div className="flex items-center gap-2 text-sm font-medium text-cosmic-rose">
            <AlertCircle size={15} />
            Session Error
          </div>
          <p className="text-xs font-mono text-cosmic-rose/80 leading-relaxed">
            {status.error_message}
          </p>
        </div>
      )}

      {/* Log viewer */}
      <div className="space-y-3">
        <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide flex items-center gap-2">
          <Terminal size={14} className="text-cosmic-muted" />
          Container Logs
        </h2>
        <LogViewer />
      </div>
    </div>
  );
}
