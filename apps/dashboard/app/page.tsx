"use client";

import { useEffect, useState, useCallback, useRef } from "react";
import {
  CheckSquare,
  MessageSquare,
  BookOpen,
  RefreshCw,
  Timer,
  AlertTriangle,
  Info,
  Send,
  ArrowRight,
  FileText,
  Activity,
  Loader2,
} from "lucide-react";
import Link from "next/link";
import PageShell from "@/components/layout/PageShell";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import {
  useDaemonEvents,
  useDaemonStatus,
} from "@/components/providers/DaemonEventContext";
import type {
  ActivityFeedEvent,
  ActivityFeedGetResponse,
  ObligationsGetResponse,
  ServerHealthGetResponse,
  MessagesGetResponse,
  StoredMessage,
  BriefingGetResponse,
} from "@/types/api";
import { apiFetch } from "@/lib/api-client";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ApiObligation {
  id: string;
  detected_action: string;
  owner?: string;
  status?: string;
}

interface WsActivityEvent {
  id: string;
  type: string;
  label: string;
  ts: number;
}

interface MessageGroup {
  sender: string;
  channel: string;
  timestamp: string;
  preview: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatSecondsAgo(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  return `${Math.floor(minutes / 60)}h ago`;
}

function formatFeedTimestamp(iso: string): string {
  return new Date(iso).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

const FEED_ICON: Record<string, typeof MessageSquare> = {
  MessageSquare,
  CheckSquare,
  BookOpen,
};

function FeedIcon({ hint }: { hint: string }) {
  const Icon = FEED_ICON[hint] ?? Activity;
  return <Icon size={13} className="shrink-0 text-ds-gray-700" aria-hidden="true" />;
}

// ---------------------------------------------------------------------------
// PriorityBanner
// ---------------------------------------------------------------------------

function PriorityBanner({
  pendingCount,
  briefingAvailable,
  briefingTime,
}: {
  pendingCount: number;
  briefingAvailable: boolean;
  briefingTime: string | null;
}) {
  if (pendingCount > 0) {
    return (
      <Link
        href="/obligations"
        className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm"
        style={{ background: "rgba(245, 158, 11, 0.10)", border: "1px solid rgba(245, 158, 11, 0.25)" }}
      >
        <AlertTriangle size={14} className="text-amber-500 shrink-0" />
        <span className="text-amber-200">
          {pendingCount} obligation{pendingCount !== 1 ? "s" : ""} need{pendingCount === 1 ? "s" : ""} attention
        </span>
        <ArrowRight size={12} className="ml-auto text-amber-500/60" />
      </Link>
    );
  }

  if (briefingAvailable) {
    return (
      <Link
        href="/briefing"
        className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm"
        style={{ background: "rgba(59, 130, 246, 0.10)", border: "1px solid rgba(59, 130, 246, 0.25)" }}
      >
        <Info size={14} className="text-blue-400 shrink-0" />
        <span className="text-blue-300">
          Briefing available{briefingTime ? ` — last generated ${briefingTime}` : ""}
        </span>
        <ArrowRight size={12} className="ml-auto text-blue-400/60" />
      </Link>
    );
  }

  return null;
}

// ---------------------------------------------------------------------------
// ActivityFeed
// ---------------------------------------------------------------------------

function ActivityFeedSection({
  events,
  wsEvents,
  loading,
}: {
  events: ActivityFeedEvent[];
  wsEvents: WsActivityEvent[];
  loading: boolean;
}) {
  if (loading) {
    return (
      <div className="space-y-1">
        {Array.from({ length: 8 }).map((_, i) => (
          <div
            key={i}
            className="h-8 animate-pulse rounded bg-ds-gray-100"
          />
        ))}
      </div>
    );
  }

  // Merge WS events at the top, then DB events
  const allEvents: Array<{ id: string; time: string; icon: string; summary: string }> = [];

  for (const ws of wsEvents) {
    allEvents.push({
      id: `ws-${ws.id}`,
      time: new Date(ws.ts).toISOString(),
      icon: "Activity",
      summary: ws.label,
    });
  }

  for (const ev of events) {
    allEvents.push({
      id: ev.id,
      time: ev.timestamp,
      icon: ev.icon_hint,
      summary: ev.summary,
    });
  }

  if (allEvents.length === 0) {
    return (
      <p className="text-copy-13 text-ds-gray-900 py-3">No recent events</p>
    );
  }

  return (
    <div className="divide-y divide-ds-gray-400">
      {allEvents.map((ev) => (
        <div
          key={ev.id}
          className="flex items-center gap-3 py-1.5 hover:bg-ds-gray-100/50 transition-colors"
        >
          <span
            className="shrink-0 text-xs text-ds-gray-900 font-mono w-12 text-right tabular-nums"
            suppressHydrationWarning
          >
            {formatFeedTimestamp(ev.time)}
          </span>
          <FeedIcon hint={ev.icon} />
          <span className="flex-1 min-w-0 text-sm text-ds-gray-1000 truncate">
            {ev.summary}
          </span>
        </div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// QuickActions
// ---------------------------------------------------------------------------

function QuickActions({
  briefingPreview,
  healthStatus,
  healthLoading,
}: {
  briefingPreview: string | null;
  healthStatus: string | null;
  healthLoading: boolean;
}) {
  const [obligationInput, setObligationInput] = useState("");
  const [creating, setCreating] = useState(false);
  const [createResult, setCreateResult] = useState<"success" | "error" | null>(null);
  const [createError, setCreateError] = useState<string | null>(null);

  const handleCreateObligation = async () => {
    if (!obligationInput.trim()) return;
    setCreating(true);
    setCreateResult(null);
    setCreateError(null);
    try {
      const res = await apiFetch("/api/obligations", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          detected_action: obligationInput.trim(),
          owner: "nova",
          status: "open",
          priority: 2,
          source_channel: "dashboard",
        }),
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({ error: "Request failed" }));
        throw new Error((data as { error?: string }).error ?? "Request failed");
      }
      setObligationInput("");
      setCreateResult("success");
      setTimeout(() => setCreateResult(null), 2000);
    } catch (err) {
      setCreateResult("error");
      setCreateError(err instanceof Error ? err.message : "Failed");
    } finally {
      setCreating(false);
    }
  };

  const healthDot = healthStatus === "healthy" || healthStatus === "ok"
    ? "bg-emerald-500"
    : healthStatus === "degraded"
      ? "bg-amber-500"
      : healthStatus === "critical" || healthStatus === "down"
        ? "bg-red-500"
        : "bg-ds-gray-600";

  return (
    <div className="divide-y divide-ds-gray-400">
      {/* Message Nova */}
      <Link
        href="/chat"
        className="flex items-center gap-3 py-3 hover:bg-ds-gray-100/50 transition-colors group"
      >
        <MessageSquare size={16} className="text-ds-gray-700 shrink-0" />
        <div className="flex-1 min-w-0">
          <p className="text-sm text-ds-gray-1000 font-medium">Message Nova</p>
          <p className="text-xs text-ds-gray-900">Open chat interface</p>
        </div>
        <ArrowRight size={14} className="text-ds-gray-700 opacity-0 group-hover:opacity-100 transition-opacity" />
      </Link>

      {/* Create Obligation */}
      <div className="py-3 space-y-2">
        <div className="flex items-center gap-3">
          <CheckSquare size={16} className="text-ds-gray-700 shrink-0" />
          <div className="flex-1 min-w-0">
            <p className="text-sm text-ds-gray-1000 font-medium">Create Obligation</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={obligationInput}
            onChange={(e) => setObligationInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleCreateObligation();
            }}
            placeholder="What needs to be done..."
            className="flex-1 min-w-0 px-2.5 py-1.5 text-sm rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-none focus:border-ds-gray-600 transition-colors"
            disabled={creating}
          />
          <button
            type="button"
            onClick={() => void handleCreateObligation()}
            disabled={creating || !obligationInput.trim()}
            className="flex items-center justify-center px-2.5 py-1.5 rounded-lg text-xs font-medium border border-ds-gray-400 text-ds-gray-900 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-40"
          >
            {creating ? <Loader2 size={14} className="animate-spin" /> : <Send size={14} />}
          </button>
        </div>
        {createResult === "success" && (
          <p className="text-xs text-emerald-400">Created</p>
        )}
        {createResult === "error" && (
          <p className="text-xs text-red-400">{createError}</p>
        )}
      </div>

      {/* View Briefing */}
      <Link
        href="/briefing"
        className="flex items-center gap-3 py-3 hover:bg-ds-gray-100/50 transition-colors group"
      >
        <FileText size={16} className="text-ds-gray-700 shrink-0" />
        <div className="flex-1 min-w-0">
          <p className="text-sm text-ds-gray-1000 font-medium">View Briefing</p>
          <p className="text-xs text-ds-gray-900 truncate">
            {briefingPreview ?? "No briefing available"}
          </p>
        </div>
        <ArrowRight size={14} className="text-ds-gray-700 opacity-0 group-hover:opacity-100 transition-opacity" />
      </Link>

      {/* Fleet Health */}
      <div className="flex items-center gap-3 py-3">
        <Activity size={16} className="text-ds-gray-700 shrink-0" />
        <div className="flex-1 min-w-0">
          <p className="text-sm text-ds-gray-1000 font-medium">Fleet Health</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {healthLoading ? (
            <span className="text-xs text-ds-gray-700">loading...</span>
          ) : (
            <>
              <span className={`inline-block w-2 h-2 rounded-full ${healthDot}`} />
              <span className="text-xs font-mono text-ds-gray-900">
                DB: {healthStatus ?? "unknown"}
              </span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// RecentConversations
// ---------------------------------------------------------------------------

function RecentConversations({
  recentMessages,
  loading,
}: {
  recentMessages: StoredMessage[];
  loading: boolean;
}) {
  if (loading) {
    return (
      <div className="space-y-1">
        {Array.from({ length: 3 }).map((_, i) => (
          <div
            key={i}
            className="h-10 animate-pulse rounded bg-ds-gray-100"
          />
        ))}
      </div>
    );
  }

  // Group consecutive messages by sender
  const groups: MessageGroup[] = [];
  let currentGroup: { sender: string; channel: string; messages: StoredMessage[] } | null = null;

  for (const msg of recentMessages) {
    if (currentGroup && currentGroup.sender === msg.sender) {
      currentGroup.messages.push(msg);
    } else {
      if (currentGroup) {
        const first = currentGroup.messages[0]!;
        const preview = first.content.length > 120 ? `${first.content.slice(0, 120)}...` : first.content;
        groups.push({
          sender: currentGroup.sender,
          channel: currentGroup.channel,
          timestamp: first.timestamp,
          preview,
        });
      }
      currentGroup = { sender: msg.sender, channel: msg.channel, messages: [msg] };
    }
  }
  if (currentGroup) {
    const first = currentGroup.messages[0]!;
    const preview = first.content.length > 120 ? `${first.content.slice(0, 120)}...` : first.content;
    groups.push({
      sender: currentGroup.sender,
      channel: currentGroup.channel,
      timestamp: first.timestamp,
      preview,
    });
  }

  const topGroups = groups.slice(0, 5);

  if (topGroups.length === 0) {
    return (
      <p className="text-copy-13 text-ds-gray-900 py-3">No recent conversations</p>
    );
  }

  return (
    <div className="divide-y divide-ds-gray-400">
      {topGroups.map((group, i) => (
        <Link
          key={i}
          href="/messages"
          className="flex items-center gap-3 py-2 hover:bg-ds-gray-100/50 transition-colors"
        >
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-ds-gray-1000">{group.sender}</span>
              <span className="text-xs font-mono text-ds-gray-700 px-1.5 py-0.5 rounded bg-ds-gray-100">
                {group.channel}
              </span>
              <span
                className="text-xs text-ds-gray-700 font-mono ml-auto shrink-0"
                suppressHydrationWarning
              >
                {formatFeedTimestamp(group.timestamp)}
              </span>
            </div>
            <p className="text-xs text-ds-gray-900 truncate mt-0.5">{group.preview}</p>
          </div>
        </Link>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Dashboard Page
// ---------------------------------------------------------------------------

export default function DashboardPage() {
  // State
  const [feedEvents, setFeedEvents] = useState<ActivityFeedEvent[]>([]);
  const [wsEvents, setWsEvents] = useState<WsActivityEvent[]>([]);
  const [obligations, setObligations] = useState<ApiObligation[]>([]);
  const [recentMessages, setRecentMessages] = useState<StoredMessage[]>([]);
  const [healthStatus, setHealthStatus] = useState<string | null>(null);
  const [briefingPreview, setBriefingPreview] = useState<string | null>(null);
  const [briefingAvailable, setBriefingAvailable] = useState(false);
  const [briefingTime, setBriefingTime] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [healthLoading, setHealthLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [lastFetchedAt, setLastFetchedAt] = useState<number>(Date.now);
  const [, setTick] = useState(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const wsIdRef = useRef(0);

  // Daemon connection status for offline overlay
  const daemonStatus = useDaemonStatus();
  const isDisconnected = daemonStatus !== "connected";

  // WebSocket subscription — prepend events
  useDaemonEvents((ev) => {
    const label =
      typeof ev.payload === "object" &&
      ev.payload !== null &&
      "label" in ev.payload
        ? String((ev.payload as { label?: unknown }).label)
        : ev.type;
    setWsEvents((prev) =>
      [
        {
          id: String(++wsIdRef.current),
          type: ev.type,
          label,
          ts: ev.ts,
        },
        ...prev,
      ].slice(0, 10),
    );
  });

  // Data fetch
  const fetchData = useCallback(async () => {
    setError(null);
    try {
      const timeout = () => AbortSignal.timeout(8000);
      const [feedRes, oblRes, msgRes, healthRes, briefRes] = await Promise.allSettled([
        apiFetch("/api/activity-feed", { signal: timeout() }),
        apiFetch("/api/obligations", { signal: timeout() }),
        apiFetch("/api/messages?limit=10", { signal: timeout() }),
        apiFetch("/api/server-health", { signal: timeout() }),
        apiFetch("/api/briefing", { signal: timeout() }),
      ]);

      // Activity feed
      if (feedRes.status === "fulfilled" && feedRes.value.ok) {
        try {
          const data = (await feedRes.value.json()) as ActivityFeedGetResponse;
          setFeedEvents(data.events);
        } catch {
          // parse failure
        }
      }

      // Obligations
      let oblList: ApiObligation[] = [];
      if (oblRes.status === "fulfilled" && oblRes.value.ok) {
        try {
          const data = (await oblRes.value.json()) as ObligationsGetResponse;
          oblList = data.obligations as ApiObligation[];
        } catch {
          // parse failure
        }
      }
      setObligations(oblList);

      // Recent messages
      if (msgRes.status === "fulfilled" && msgRes.value.ok) {
        try {
          const data = (await msgRes.value.json()) as MessagesGetResponse;
          setRecentMessages(data.messages);
        } catch {
          // parse failure
        }
      }

      // Health
      if (healthRes.status === "fulfilled" && healthRes.value.ok) {
        try {
          const data = (await healthRes.value.json()) as ServerHealthGetResponse;
          setHealthStatus(data.status);
        } catch {
          // parse failure
        }
      }
      setHealthLoading(false);

      // Briefing
      if (briefRes.status === "fulfilled" && briefRes.value.ok) {
        try {
          const data = (await briefRes.value.json()) as BriefingGetResponse;
          if (data.entry) {
            setBriefingAvailable(true);
            const preview = data.entry.content.length > 100
              ? `${data.entry.content.slice(0, 100)}...`
              : data.entry.content;
            setBriefingPreview(preview);
            setBriefingTime(
              new Date(data.entry.generated_at).toLocaleTimeString([], {
                hour: "2-digit",
                minute: "2-digit",
              }),
            );
          } else {
            setBriefingAvailable(false);
            setBriefingPreview(null);
            setBriefingTime(null);
          }
        } catch {
          // parse failure
        }
      }

      setLastFetchedAt(Date.now());
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load data");
    } finally {
      setLoading(false);
    }
  }, []);

  // Effects — initial load + auto-refresh
  useEffect(() => {
    void fetchData();
  }, [fetchData]);

  useEffect(() => {
    if (autoRefresh) {
      intervalRef.current = setInterval(() => void fetchData(), 10_000);
    } else {
      if (intervalRef.current) clearInterval(intervalRef.current);
    }
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [autoRefresh, fetchData]);

  // 1-second tick for "Updated Xs ago"
  useEffect(() => {
    const tickInterval = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(tickInterval);
  }, []);

  // Derived
  const pendingObligations = obligations.filter(
    (o) => !o.status || o.status === "open" || o.status === "in_progress",
  );
  const updatedAgo = formatSecondsAgo(Date.now() - lastFetchedAt);
  const lastFetchedIso = new Date(lastFetchedAt).toISOString();
  const isRefreshing = loading;

  // Header action — refresh toggle + last-updated timestamp
  const headerAction = (
    <div className="flex items-center gap-3">
      <span
        className="text-xs text-ds-gray-900 tabular-nums"
        title={lastFetchedIso}
        suppressHydrationWarning
      >
        Updated {updatedAgo}
      </span>
      <button
        type="button"
        onClick={() => setAutoRefresh((v) => !v)}
        disabled={isDisconnected}
        className={[
          "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border transition-colors",
          isDisconnected
            ? "opacity-50 cursor-not-allowed text-ds-gray-900 border-ds-gray-400"
            : autoRefresh
              ? "bg-ds-gray-alpha-200 text-ds-gray-1000 border-ds-gray-1000/40 hover:bg-ds-gray-700/30"
              : "text-ds-gray-900 border-ds-gray-400 hover:border-ds-gray-500 hover:text-ds-gray-1000",
        ].join(" ")}
        aria-label={autoRefresh ? "Disable auto-refresh" : "Enable auto-refresh"}
      >
        <Timer size={12} aria-hidden="true" />
        Auto
      </button>
      <button
        type="button"
        onClick={() => void fetchData()}
        disabled={isRefreshing || isDisconnected}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
      >
        <RefreshCw size={12} className={isRefreshing ? "animate-spin" : ""} />
        Refresh
      </button>
    </div>
  );

  return (
    <PageShell
      title="Command Center"
      action={headerAction}
    >
      <div className={`space-y-4 animate-fade-in-up transition-opacity ${isDisconnected ? "opacity-50" : ""}`}>
        {error && (
          <ErrorBanner
            message="Failed to load dashboard data"
            detail={error}
            onRetry={() => void fetchData()}
          />
        )}

        {/* Priority Banner */}
        <PriorityBanner
          pendingCount={pendingObligations.length}
          briefingAvailable={briefingAvailable}
          briefingTime={briefingTime}
        />

        {/* Two-column layout: 60% feed + 40% quick actions */}
        <div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
          {/* Activity Feed (left 3/5) */}
          <div className="lg:col-span-3 space-y-2">
            <SectionHeader
              label="Activity Feed"
              count={feedEvents.length + wsEvents.length}
            />
            <ActivityFeedSection
              events={feedEvents}
              wsEvents={wsEvents}
              loading={loading}
            />
          </div>

          {/* Quick Actions (right 2/5) */}
          <div className="lg:col-span-2 space-y-2">
            <SectionHeader label="Quick Actions" />
            <QuickActions
              briefingPreview={briefingPreview}
              healthStatus={healthStatus}
              healthLoading={healthLoading}
            />
          </div>
        </div>

        {/* Recent Conversations — full width */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <SectionHeader
              label="Recent Conversations"
              count={recentMessages.length}
            />
            <Link
              href="/messages"
              className="flex items-center gap-1 text-xs text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
            >
              All messages
              <ArrowRight size={12} />
            </Link>
          </div>
          <RecentConversations recentMessages={recentMessages} loading={loading} />
        </div>
      </div>
    </PageShell>
  );
}
