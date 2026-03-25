"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import {
  ArrowLeft,
  MessageSquare,
  Terminal,
  Clock,
  Activity,
  RefreshCw,
  Layers,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface SessionDetail {
  id: string;
  service: string;
  status: "active" | "idle" | "completed";
  messages: number;
  tools_executed: number;
  started_at: string;
  ended_at?: string;
  user?: string;
  project?: string;
  cost_usd?: number;
  model?: string;
  input_tokens?: number;
  output_tokens?: number;
  recent_messages?: Array<{
    id: string;
    role: "user" | "assistant";
    content: string;
    ts: string;
  }>;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SERVICE_COLORS: Record<string, string> = {
  Telegram: "bg-[#229ED9]/20 text-[#229ED9]",
  Discord: "bg-[#5865F2]/20 text-[#5865F2]",
  Slack: "bg-[#4A154B]/20 text-[#E01E5A]",
  CLI: "bg-cosmic-purple/20 text-cosmic-purple",
  API: "bg-cosmic-rose/20 text-cosmic-rose",
  Web: "bg-emerald-500/20 text-emerald-400",
};

const STATUS_CONFIG: Record<
  SessionDetail["status"],
  { label: string; dot: string; text: string }
> = {
  active: { label: "Active", dot: "bg-emerald-400 animate-pulse", text: "text-emerald-400" },
  idle: { label: "Idle", dot: "bg-amber-400", text: "text-amber-400" },
  completed: { label: "Completed", dot: "bg-cosmic-muted", text: "text-cosmic-muted" },
};

function elapsed(startIso: string, endIso?: string): string {
  const start = new Date(startIso).getTime();
  const end = endIso ? new Date(endIso).getTime() : Date.now();
  const diffMs = end - start;
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 60) return `${diffMin}m`;
  const h = Math.floor(diffMin / 60);
  return `${h}h ${diffMin % 60}m`;
}

// ---------------------------------------------------------------------------
// StatTile
// ---------------------------------------------------------------------------

function StatTile({
  icon: Icon,
  label,
  value,
  accent,
}: {
  icon: React.ElementType;
  label: string;
  value: string | number;
  accent?: string;
}) {
  return (
    <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-surface border border-cosmic-border">
      <div
        className={`flex items-center justify-center w-9 h-9 rounded-lg shrink-0 ${accent ?? "bg-cosmic-purple/20"}`}
      >
        <Icon
          size={18}
          className={accent ? "text-cosmic-rose" : "text-cosmic-purple"}
        />
      </div>
      <div className="min-w-0">
        <p className="text-xs text-cosmic-muted uppercase tracking-wide truncate">
          {label}
        </p>
        <p className="text-lg font-semibold font-mono text-cosmic-bright">
          {value}
        </p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// MessageRow
// ---------------------------------------------------------------------------

function MessageRow({
  msg,
}: {
  msg: NonNullable<SessionDetail["recent_messages"]>[number];
}) {
  const isUser = msg.role === "user";
  return (
    <div
      className={`flex gap-3 py-3 px-4 ${
        isUser ? "bg-cosmic-surface/50" : ""
      }`}
    >
      <div
        className={`flex items-center justify-center w-6 h-6 rounded-full shrink-0 text-xs font-bold font-mono mt-0.5 ${
          isUser
            ? "bg-cosmic-rose/20 text-cosmic-rose"
            : "bg-cosmic-purple/20 text-cosmic-purple"
        }`}
      >
        {isUser ? "U" : "N"}
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm text-cosmic-text leading-relaxed whitespace-pre-wrap break-words">
          {msg.content}
        </p>
        <p
          className="text-xs text-cosmic-muted mt-1 font-mono"
          suppressHydrationWarning
        >
          {new Date(msg.ts).toLocaleTimeString()}
        </p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// SessionDetailPage
// ---------------------------------------------------------------------------

export default function SessionDetailPage() {
  const params = useParams<{ id: string }>();
  const router = useRouter();
  const sessionId = params.id;

  // 1. State
  const [session, setSession] = useState<SessionDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // 2. Fetch
  const fetchSession = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(`/api/sessions/${sessionId}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as SessionDetail;
      setSession(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load session");
    } finally {
      setLoading(false);
    }
  };

  // 3. Initial load
  useEffect(() => {
    void fetchSession();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  // 4. Real-time updates for active sessions
  useDaemonEvents(
    (ev) => {
      if (
        session?.status === "active" &&
        (ev.type === "session.message" || ev.type === "session.update")
      ) {
        void fetchSession();
      }
    },
    "session",
  );

  // 5. Action slot
  const action = (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={() => router.back()}
        className="flex items-center gap-2 px-3 py-2 min-h-11 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors"
      >
        <ArrowLeft size={14} />
        <span className="hidden sm:inline">Back</span>
      </button>
      <button
        type="button"
        onClick={() => void fetchSession()}
        disabled={loading}
        className="flex items-center gap-2 px-3 py-2 min-h-11 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
      >
        <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
      </button>
    </div>
  );

  // 6. Early returns
  if (loading) {
    return (
      <PageShell title="Session" action={action}>
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <div
                key={i}
                className="h-20 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
              />
            ))}
          </div>
          <div className="h-64 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border" />
        </div>
      </PageShell>
    );
  }

  if (error) {
    return (
      <PageShell title="Session" action={action}>
        <ErrorBanner message={error} onRetry={() => void fetchSession()} />
      </PageShell>
    );
  }

  if (!session) {
    return (
      <PageShell title="Session" action={action}>
        <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted">
          <Layers size={32} />
          <p className="text-sm">Session not found</p>
        </div>
      </PageShell>
    );
  }

  const statusCfg = STATUS_CONFIG[session.status];
  const serviceColor =
    SERVICE_COLORS[session.service] ?? "bg-cosmic-muted/20 text-cosmic-muted";

  return (
    <PageShell
      title={`Session`}
      subtitle={session.id}
      action={action}
    >
      {/* Status row */}
      <div className="flex items-center gap-3 mb-6 flex-wrap">
        <span
          className={`inline-flex items-center px-2.5 py-1 rounded-lg text-xs font-medium font-mono ${serviceColor}`}
        >
          {session.service}
        </span>
        <span className="flex items-center gap-1.5 text-sm">
          <span
            className={`inline-block w-2 h-2 rounded-full shrink-0 ${statusCfg.dot}`}
          />
          <span className={`text-sm font-medium ${statusCfg.text}`}>
            {statusCfg.label}
          </span>
        </span>
        {session.user && (
          <span className="text-sm text-cosmic-muted">@{session.user}</span>
        )}
        {session.project && (
          <span className="text-xs font-mono px-2 py-0.5 rounded bg-cosmic-surface border border-cosmic-border text-cosmic-muted">
            {session.project}
          </span>
        )}
      </div>

      {/* Stat tiles — 2-col on mobile, 4-col on desktop */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-6">
        <StatTile
          icon={MessageSquare}
          label="Messages"
          value={session.messages}
        />
        <StatTile
          icon={Terminal}
          label="Tools"
          value={session.tools_executed}
        />
        <StatTile
          icon={Clock}
          label="Duration"
          value={elapsed(session.started_at, session.ended_at)}
          accent="bg-cosmic-rose/20"
        />
        <StatTile
          icon={Activity}
          label="Model"
          value={session.model ?? "—"}
          accent="bg-cosmic-rose/20"
        />
      </div>

      {/* Token / cost details */}
      {(session.input_tokens ?? session.output_tokens ?? session.cost_usd) && (
        <div className="grid grid-cols-3 gap-3 mb-6">
          {session.input_tokens !== undefined && (
            <StatTile
              icon={MessageSquare}
              label="Input tokens"
              value={session.input_tokens.toLocaleString()}
            />
          )}
          {session.output_tokens !== undefined && (
            <StatTile
              icon={MessageSquare}
              label="Output tokens"
              value={session.output_tokens.toLocaleString()}
            />
          )}
          {session.cost_usd !== undefined && (
            <StatTile
              icon={Activity}
              label="Cost"
              value={`$${session.cost_usd.toFixed(4)}`}
              accent="bg-cosmic-rose/20"
            />
          )}
        </div>
      )}

      {/* Recent messages */}
      {session.recent_messages && session.recent_messages.length > 0 && (
        <div className="rounded-cosmic border border-cosmic-border overflow-hidden">
          <div className="flex items-center gap-2 px-4 py-3 border-b border-cosmic-border bg-cosmic-surface shrink-0">
            <MessageSquare size={14} className="text-cosmic-muted" />
            <span className="text-xs font-semibold text-cosmic-muted uppercase tracking-widest">
              Recent Messages
            </span>
            <span className="ml-auto text-xs font-mono text-cosmic-muted">
              {session.recent_messages.length}
            </span>
          </div>
          <div className="divide-y divide-cosmic-border/50">
            {session.recent_messages.map((msg) => (
              <MessageRow key={msg.id} msg={msg} />
            ))}
          </div>
        </div>
      )}
    </PageShell>
  );
}
