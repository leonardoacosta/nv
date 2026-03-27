"use client";

import { useState, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
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
  Activity,
  Loader2,
  Mail,
  Server,
  MonitorPlay,
  Clock,
  ExternalLink,
  Terminal,
} from "lucide-react";
import Link from "next/link";
import PageShell from "@/components/layout/PageShell";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import QuerySkeleton from "@/components/layout/QuerySkeleton";
import StatStrip from "@/components/StatStrip";
import {
  useDaemonEvents,
  useDaemonStatus,
} from "@/components/providers/DaemonEventContext";
import type {
  ActivityFeedEvent,
  ActivityFeedGetResponse,
  CcSessionSummary,
  CcSessionsGetResponse,
  ObligationsGetResponse,
  MessagesGetResponse,
  StoredMessage,
  BriefingGetResponse,
  FleetHealthResponse,
  SessionsGetResponse,
} from "@/types/api";
import { useQuery, useMutation } from "@tanstack/react-query";
import { trpc } from "@/lib/trpc/react";

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

type FeedCategory = "all" | "message" | "session" | "obligation" | "system";

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

function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const s = Math.floor(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

/** Maps a feed event to a severity tier for visual weight. Uses the API severity field. */
function getEventSeverity(event: ActivityFeedEvent): "error" | "warning" | "routine" {
  if (event.severity === "error") return "error";
  if (event.severity === "warning") return "warning";
  return "routine";
}

/** Icon + color config per severity tier. */
function getSeverityConfig(severity: "error" | "warning" | "routine") {
  if (severity === "error") {
    return { iconColor: "text-red-700", rowBg: "bg-red-700/5", leftBorder: "border-l-2 border-red-700" };
  }
  if (severity === "warning") {
    return { iconColor: "text-amber-700", rowBg: "bg-amber-700/5", leftBorder: "border-l-2 border-amber-700" };
  }
  return { iconColor: "text-ds-gray-700", rowBg: "", leftBorder: "" };
}

/** Icon component per event type. */
function FeedEventIcon({ type, severity }: { type: string; severity: "error" | "warning" | "routine" }) {
  const { iconColor } = getSeverityConfig(severity);
  const cls = `shrink-0 ${iconColor}`;
  if (type === "message") return <MessageSquare size={13} className={cls} aria-hidden="true" />;
  if (type === "obligation") return <CheckSquare size={13} className={cls} aria-hidden="true" />;
  if (type === "diary") return <BookOpen size={13} className={cls} aria-hidden="true" />;
  if (type === "session") return <MonitorPlay size={13} className={cls} aria-hidden="true" />;
  return <Activity size={13} className={cls} aria-hidden="true" />;
}

/** Destination link for "View" in expanded detail. */
function getViewLink(type: string): string {
  if (type === "message") return "/messages";
  if (type === "obligation") return "/obligations";
  if (type === "diary") return "/diary";
  if (type === "session") return "/sessions";
  return "/";
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
        className="flex items-center gap-2 px-3 py-2 rounded-lg text-copy-13 bg-amber-700/10 border border-amber-700/25"
      >
        <AlertTriangle size={14} className="text-amber-700 shrink-0" />
        <span className="text-amber-700">
          {pendingCount} obligation{pendingCount !== 1 ? "s" : ""} need{pendingCount === 1 ? "s" : ""} attention
        </span>
        <ArrowRight size={12} className="ml-auto text-amber-700/60" />
      </Link>
    );
  }

  if (briefingAvailable) {
    return (
      <Link
        href="/briefing"
        className="flex items-center gap-2 px-3 py-2 rounded-lg text-copy-13 bg-blue-700/10 border border-blue-700/25"
      >
        <Info size={14} className="text-blue-700 shrink-0" />
        <span className="text-blue-700">
          Briefing available{briefingTime ? ` — last generated ${briefingTime}` : ""}
        </span>
        <ArrowRight size={12} className="ml-auto text-blue-700/60" />
      </Link>
    );
  }

  return null;
}

// ---------------------------------------------------------------------------
// ObligationBar — full-width quick-add input
// ---------------------------------------------------------------------------

function ObligationBar() {
  const [input, setInput] = useState("");
  const [result, setResult] = useState<"success" | "error" | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const { mutate: createObligation, isPending: creating } = useMutation(
    trpc.obligation.create.mutationOptions({
      onSuccess: () => {
        setInput("");
        setResult("success");
        setTimeout(() => setResult(null), 2000);
        void queryClient.invalidateQueries({ queryKey: trpc.obligation.list.queryKey() });
        void queryClient.invalidateQueries({ queryKey: trpc.system.activityFeed.queryKey() });
      },
      onError: (err) => {
        setResult("error");
        setErrorMsg(err.message);
      },
    }),
  );

  const handleCreate = () => {
    if (!input.trim()) return;
    setResult(null);
    setErrorMsg(null);
    createObligation({
      detected_action: input.trim(),
      owner: "nova",
      status: "open",
      priority: 2,
      source_channel: "dashboard",
    });
  };

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2 border border-ds-gray-400 rounded-lg px-3 py-1.5">
        <CheckSquare size={14} className="text-ds-gray-700 shrink-0" />
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleCreate();
          }}
          placeholder="Add obligation..."
          className="flex-1 min-w-0 bg-transparent text-copy-13 text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-hidden"
          disabled={creating}
        />
        <button
          type="button"
          onClick={handleCreate}
          disabled={creating || !input.trim()}
          className="flex items-center justify-center px-2 py-0.5 rounded text-label-12 border border-ds-gray-400 text-ds-gray-900 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-40"
        >
          {creating ? <Loader2 size={13} className="animate-spin" /> : <Send size={13} />}
        </button>
      </div>
      {result === "success" && (
        <p className="text-copy-13 text-green-700 px-1">Created</p>
      )}
      {result === "error" && (
        <p className="text-copy-13 text-red-700 px-1">{errorMsg}</p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// CategoryPills — filter tabs above feed
// ---------------------------------------------------------------------------

const PILL_LABELS: { key: FeedCategory; label: string }[] = [
  { key: "all", label: "All" },
  { key: "message", label: "Messages" },
  { key: "session", label: "Sessions" },
  { key: "obligation", label: "Obligations" },
  { key: "system", label: "System" },
];

function getCategoryCount(
  events: ActivityFeedEvent[],
  wsEvents: WsActivityEvent[],
  category: FeedCategory,
): number {
  if (category === "all") return events.length + wsEvents.length;
  if (category === "system") return events.filter((e) => e.type === "diary").length;
  return events.filter((e) => e.type === category).length;
}

function CategoryPills({
  active,
  onChange,
  events,
  wsEvents,
}: {
  active: FeedCategory;
  onChange: (c: FeedCategory) => void;
  events: ActivityFeedEvent[];
  wsEvents: WsActivityEvent[];
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {PILL_LABELS.map(({ key, label }) => {
        const count = getCategoryCount(events, wsEvents, key);
        const isActive = active === key;
        return (
          <button
            key={key}
            type="button"
            onClick={() => onChange(key)}
            className={[
              "text-label-12 px-2.5 py-1 rounded-full border transition-colors",
              isActive
                ? "bg-ds-gray-alpha-200 border-ds-gray-1000/40 text-ds-gray-1000"
                : "text-ds-gray-700 border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500",
            ].join(" ")}
          >
            {label} ({count})
          </button>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// ActivityFeedSection — density-pass rework
// ---------------------------------------------------------------------------

interface MergedFeedEvent {
  id: string;
  type: string;
  time: string;
  summary: string;
  severity: "error" | "warning" | "routine";
  isWs?: boolean;
}

function ActivityFeedSection({
  events,
  wsEvents,
  loading,
  category,
}: {
  events: ActivityFeedEvent[];
  wsEvents: WsActivityEvent[];
  loading: boolean;
  category: FeedCategory;
}) {
  const [expandedId, setExpandedId] = useState<string | null>(null);

  if (loading) {
    return (
      <div className="flex flex-col gap-px">
        {Array.from({ length: 8 }).map((_, i) => (
          <div key={i} className="h-7 animate-pulse rounded bg-ds-gray-100" />
        ))}
      </div>
    );
  }

  // Build merged list
  const merged: MergedFeedEvent[] = [];

  for (const ws of wsEvents) {
    merged.push({
      id: `ws-${ws.id}`,
      type: ws.type,
      time: new Date(ws.ts).toISOString(),
      summary: ws.label,
      severity: "routine",
      isWs: true,
    });
  }

  for (const ev of events) {
    merged.push({
      id: ev.id,
      type: ev.type,
      time: ev.timestamp,
      summary: ev.summary,
      severity: getEventSeverity(ev),
    });
  }

  // Apply category filter
  const filtered = merged.filter((ev) => {
    if (category === "all") return true;
    if (category === "system") return ev.type === "diary";
    return ev.type === category;
  });

  if (filtered.length === 0) {
    return (
      <p className="text-copy-13 text-ds-gray-900 py-3">No events in this category</p>
    );
  }

  return (
    <div className="divide-y divide-ds-gray-400">
      {filtered.map((ev) => {
        const { rowBg, leftBorder } = getSeverityConfig(ev.severity);
        const isExpanded = expandedId === ev.id;

        return (
          <div key={ev.id} className={rowBg}>
            {/* Main row — single horizontal line */}
            <button
              type="button"
              onClick={() => setExpandedId(isExpanded ? null : ev.id)}
              className={[
                "w-full flex items-center gap-2 py-1 px-1 hover:bg-ds-gray-alpha-100 transition-colors text-left",
                leftBorder,
              ].join(" ")}
            >
              <span
                className="shrink-0 text-copy-13 text-ds-gray-900 font-mono w-12 text-right tabular-nums"
                suppressHydrationWarning
              >
                {formatFeedTimestamp(ev.time)}
              </span>
              <FeedEventIcon type={ev.type} severity={ev.severity} />
              <span className="flex-1 min-w-0 text-copy-13 text-ds-gray-1000 truncate">
                {ev.summary}
              </span>
              <span
                className="shrink-0 text-copy-13 text-ds-gray-700 tabular-nums"
                suppressHydrationWarning
              >
                {formatRelativeTime(ev.time)}
              </span>
            </button>

            {/* Expandable detail panel */}
            {isExpanded && (
              <div className="px-4 py-2 bg-ds-gray-alpha-100 border-t border-ds-gray-400 flex flex-col gap-1.5">
                <p className="text-copy-13 text-ds-gray-1000 leading-snug">{ev.summary}</p>
                <div className="flex items-center gap-3 flex-wrap">
                  <span className="text-label-12 px-1.5 py-0.5 rounded bg-ds-gray-200 text-ds-gray-900 border border-ds-gray-400">
                    {ev.type}
                  </span>
                  <span
                    className="text-copy-13 text-ds-gray-700 font-mono tabular-nums"
                    suppressHydrationWarning
                  >
                    {new Date(ev.time).toLocaleString()}
                  </span>
                  <Link
                    href={getViewLink(ev.type)}
                    className="flex items-center gap-1 text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors ml-auto"
                  >
                    View
                    <ExternalLink size={11} />
                  </Link>
                </div>
              </div>
            )}
          </div>
        );
      })}
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
      <div className="flex flex-col gap-1">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="h-10 animate-pulse rounded bg-ds-gray-100" />
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
              <span className="text-label-14 text-ds-gray-1000">{group.sender}</span>
              <span className="text-copy-13 font-mono text-ds-gray-700 px-1.5 py-0.5 rounded bg-ds-gray-100">
                {group.channel}
              </span>
              <span
                className="text-copy-13 text-ds-gray-700 font-mono ml-auto shrink-0"
                suppressHydrationWarning
              >
                {formatFeedTimestamp(group.timestamp)}
              </span>
            </div>
            <p className="text-copy-13 text-ds-gray-900 truncate mt-0.5">{group.preview}</p>
          </div>
        </Link>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// CcSessionsWidget — compact card for the home page
// ---------------------------------------------------------------------------

function CcSessionsWidget() {
  const { data, isLoading } = useQuery(trpc.session.ccSessions.queryOptions());
  const sessions = data?.sessions ?? [];
  const running = sessions.filter((s) => s.state === "running").length;

  if (isLoading) {
    return (
      <div className="h-16 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400" />
    );
  }

  if (sessions.length === 0) return null;

  return (
    <Link
      href="/sessions"
      className="flex items-center gap-3 px-4 py-3 rounded-xl surface-card hover:border-ds-gray-1000/40 transition-colors"
    >
      <div className="flex items-center gap-2">
        <Terminal size={14} className="text-ds-gray-1000" />
        <span className="text-label-14 font-medium text-ds-gray-1000">
          CC Sessions
        </span>
      </div>
      <div className="flex items-center gap-2 ml-auto">
        {running > 0 && (
          <span className="flex items-center gap-1.5 text-copy-13 text-green-700">
            <span className="inline-block size-2 rounded-full bg-green-700 animate-pulse" />
            {running} running
          </span>
        )}
        <span className="text-copy-13 text-ds-gray-900">
          {sessions.length} total
        </span>
        <ArrowRight size={12} className="text-ds-gray-700" />
      </div>
    </Link>
  );
}

// ---------------------------------------------------------------------------
// Main Dashboard Page
// ---------------------------------------------------------------------------

export default function DashboardPage() {
  // --- 1. Local state ---
  const [wsEvents, setWsEvents] = useState<WsActivityEvent[]>([]);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [, setTick] = useState(0);
  const [feedCategory, setFeedCategory] = useState<FeedCategory>("all");
  const wsIdRef = useRef(0);
  const queryClient = useQueryClient();

  // --- 2. Context/routing ---
  const daemonStatus = useDaemonStatus();
  const isDisconnected = daemonStatus !== "connected";

  // --- 3. WebSocket subscription — prepend events ---
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

  // --- 4. Queries (tRPC with independent refetch intervals) ---
  const refetchOpts = { refetchInterval: autoRefresh ? 10_000 : false as const };
  const feedQuery = useQuery(
    trpc.system.activityFeed.queryOptions(undefined, refetchOpts),
  );
  const oblQuery = useQuery(
    trpc.obligation.list.queryOptions({}, refetchOpts),
  );
  const msgQuery = useQuery(
    trpc.message.list.queryOptions({ limit: 50 } as Record<string, unknown>, refetchOpts),
  );
  const briefQuery = useQuery(
    trpc.briefing.latest.queryOptions(undefined, refetchOpts),
  );
  const fleetQuery = useQuery(
    trpc.system.fleetStatus.queryOptions(undefined, refetchOpts),
  );
  const sessionsQuery = useQuery(
    trpc.session.list.queryOptions({}, refetchOpts),
  );

  // --- 5. Derived values from queries ---
  const feedData = feedQuery.data as ActivityFeedGetResponse | undefined;
  const feedEvents = feedData?.events ?? [];
  const oblData = oblQuery.data as ObligationsGetResponse | undefined;
  const obligations = (oblData?.obligations ?? []) as ApiObligation[];
  const msgData = msgQuery.data as MessagesGetResponse | undefined;
  const recentMessages = (msgData?.messages ?? []).slice(0, 10) as StoredMessage[];
  const allMessages = (msgData?.messages ?? []) as StoredMessage[];

  const briefData = briefQuery.data as BriefingGetResponse | undefined;
  const briefingEntry = briefData?.entry;
  const briefingAvailable = !!briefingEntry;
  const briefingTime = briefingEntry
    ? new Date(briefingEntry.generated_at).toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
      })
    : null;

  const fleetRaw = fleetQuery.data as FleetHealthResponse | undefined;
  const fleetData = fleetRaw?.fleet;
  const fleetHealthy = fleetData?.healthy_count ?? null;
  const fleetTotal = fleetData?.total_count ?? null;
  const fleetStatus = fleetData?.status ?? null;

  const sessData = sessionsQuery.data as SessionsGetResponse | undefined;
  const sessionsList = sessData?.sessions ?? [];
  const activeSessions = sessionsList.filter(
    (s) => s.status === "running" || s.status === "active",
  ).length;

  const loading = feedQuery.isLoading && oblQuery.isLoading && msgQuery.isLoading;
  const error = feedQuery.error ?? oblQuery.error ?? null;

  const pendingObligations = obligations.filter(
    (o) => !o.status || o.status === "open" || o.status === "in_progress",
  );
  const pendingNova = pendingObligations.filter((o) => !o.owner || o.owner === "nova").length;
  const pendingLeo = pendingObligations.filter((o) => o.owner === "leo").length;

  // Unread messages: sender !== "nova" in last 4h
  const fourHoursAgo = Date.now() - 4 * 60 * 60 * 1000;
  const unreadMessages = allMessages.filter(
    (m) => m.sender !== "nova" && new Date(m.timestamp).getTime() >= fourHoursAgo,
  );
  const unreadByChannel: Record<string, number> = {};
  for (const m of unreadMessages) {
    unreadByChannel[m.channel] = (unreadByChannel[m.channel] ?? 0) + 1;
  }
  const channelBreakdown = Object.entries(unreadByChannel)
    .map(([ch, n]) => `${ch.slice(0, 2).toUpperCase()}: ${n}`)
    .join(" / ");

  // Fleet health dot
  const fleetDot =
    fleetStatus === "healthy"
      ? "bg-green-700"
      : fleetStatus === "degraded"
        ? "bg-amber-700"
        : fleetStatus === "unhealthy"
          ? "bg-red-700"
          : "bg-ds-gray-600";

  // Next briefing label
  const nextBriefingLabel = briefingAvailable ? "Available" : "No schedule";

  const lastFetchedAt = feedQuery.dataUpdatedAt || Date.now();
  const updatedAgo = formatSecondsAgo(Date.now() - lastFetchedAt);
  const lastFetchedIso = new Date(lastFetchedAt).toISOString();
  const isRefreshing = feedQuery.isFetching;

  // --- 6. Refresh all ---
  const handleRefreshAll = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.all });
  };

  // --- 7. Stat strip cells ---
  const statCells = [
    {
      icon: <Mail size={14} />,
      label: "Unread Messages",
      value: String(unreadMessages.length),
      sublabel: channelBreakdown || undefined,
    },
    {
      icon: <CheckSquare size={14} />,
      label: "Pending Obligations",
      value: String(pendingObligations.length),
      sublabel: `Nova: ${pendingNova} / Leo: ${pendingLeo}`,
    },
    {
      icon: (
        <span className="flex items-center gap-1">
          <Server size={14} />
          <span className={`inline-block size-2 rounded-full ${fleetDot}`} />
        </span>
      ),
      label: "Fleet Health",
      value:
        fleetHealthy !== null && fleetTotal !== null
          ? `${fleetHealthy}/${fleetTotal} up`
          : "\u2014",
    },
    {
      icon: <MonitorPlay size={14} />,
      label: "Active Sessions",
      value: String(activeSessions),
    },
    {
      icon: <Clock size={14} />,
      label: "Next Briefing",
      value: nextBriefingLabel,
      sublabel: briefingAvailable && briefingTime ? `generated ${briefingTime}` : undefined,
    },
  ];

  // --- 8. Header action ---
  const headerAction = (
    <div className="flex items-center gap-3">
      <span
        className="text-copy-13 text-ds-gray-900 tabular-nums"
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
          "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 border transition-colors",
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
        onClick={handleRefreshAll}
        disabled={isRefreshing || isDisconnected}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
      >
        <RefreshCw size={12} className={isRefreshing ? "animate-spin" : ""} />
        Refresh
      </button>
    </div>
  );

  // --- 9. Render ---
  return (
    <PageShell title="Command Center" action={headerAction}>
      <div className={`flex flex-col gap-4 animate-fade-in-up transition-opacity ${isDisconnected ? "opacity-50" : ""}`}>
        {error && (
          <ErrorBanner
            message="Failed to load dashboard data"
            detail={error.message}
            onRetry={handleRefreshAll}
          />
        )}

        {/* Priority Banner */}
        <PriorityBanner
          pendingCount={pendingObligations.length}
          briefingAvailable={briefingAvailable}
          briefingTime={briefingTime}
        />

        {/* Stat Strip — full width */}
        <StatStrip cells={statCells} />

        {/* CC Sessions Widget */}
        <CcSessionsWidget />

        {/* Activity Feed — full width */}
        <div className="flex flex-col gap-2">
          <SectionHeader
            label="Activity Feed"
            count={feedEvents.length + wsEvents.length}
          />
          {/* Filter pills */}
          <CategoryPills
            active={feedCategory}
            onChange={setFeedCategory}
            events={feedEvents}
            wsEvents={wsEvents}
          />
          {/* Quick-add obligation bar */}
          <ObligationBar />
          {/* Feed rows */}
          <ActivityFeedSection
            events={feedEvents}
            wsEvents={wsEvents}
            loading={loading}
            category={feedCategory}
          />
        </div>

        {/* Recent Conversations — full width */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <SectionHeader
              label="Recent Conversations"
              count={recentMessages.length}
            />
            <Link
              href="/messages"
              className="flex items-center gap-1 text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
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
