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
  Send,
  ArrowRight,
  Activity,
  Loader2,
  Server,
  MonitorPlay,
  ExternalLink,
  Plus,
  ChevronDown,
  ChevronRight,
  Circle,
  X,
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
  MessagesGetResponse,
  BriefingGetResponse,
  FleetHealthResponse,
  SessionsGetResponse,
  ActionItem,
} from "@/types/api";
import { useQuery, useMutation } from "@tanstack/react-query";
import { useTRPC } from "@/lib/trpc/react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ApiObligation {
  id: string;
  detected_action: string;
  owner?: string;
  status?: string;
  updated_at?: string;
}

interface WsActivityEvent {
  id: string;
  type: string;
  label: string;
  ts: number;
}

interface CategoryBadge {
  type: "message" | "obligation" | "session" | "system";
  newCount: number;
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

/** Maps a feed event to a severity tier for visual weight. */
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
// ActionItems — top priority panel
// ---------------------------------------------------------------------------

function ActionItems({
  obligations,
  messages,
  oblLoading,
  msgLoading,
}: {
  obligations: ApiObligation[];
  messages: { sender: string; timestamp: string; content: string }[];
  oblLoading: boolean;
  msgLoading: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const fourHoursAgo = Date.now() - 4 * 60 * 60 * 1000;

  // Build action items from data
  const items: ActionItem[] = [];

  // Pending obligations
  for (const ob of obligations) {
    if (ob.status === "open" || ob.status === "in_progress") {
      items.push({
        id: ob.id,
        severity: "warning",
        category: "obligation",
        summary: ob.detected_action,
        link: "/obligations",
      });
    }
  }

  // Unread messages (last 4h, not from nova)
  const unreadMessages = messages.filter(
    (m) => m.sender !== "nova" && new Date(m.timestamp).getTime() >= fourHoursAgo,
  );
  for (const msg of unreadMessages.slice(0, 5)) {
    items.push({
      id: msg.timestamp,
      severity: "warning",
      category: "message",
      summary: msg.content.length > 80 ? `${msg.content.slice(0, 80)}...` : msg.content,
      link: "/messages",
    });
  }

  const loading = oblLoading || msgLoading;
  const displayItems = expanded ? items.slice(0, 10) : items.slice(0, 5);
  const remaining = items.length - 5;

  if (loading) {
    return (
      <div className="flex flex-col gap-1">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="h-8 animate-pulse rounded bg-ds-gray-100" />
        ))}
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <p className="text-copy-13 text-ds-gray-700 py-2">
        All clear — no pending obligations or unread messages
      </p>
    );
  }

  return (
    <div className="flex flex-col divide-y divide-ds-gray-400">
      {displayItems.map((item) => (
        <Link
          key={item.id}
          href={item.link}
          className="flex items-center gap-2 py-2 hover:bg-ds-gray-alpha-100 transition-colors px-1 -mx-1 rounded"
        >
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${
              item.severity === "error" ? "bg-red-700" : "bg-amber-700"
            }`}
          />
          <span className={`text-label-12 shrink-0 ${
            item.category === "obligation" ? "text-amber-700" : "text-blue-700"
          }`}>
            {item.category}
          </span>
          <span className="text-copy-13 text-ds-gray-1000 flex-1 min-w-0 truncate">
            {item.summary}
          </span>
          <ArrowRight size={11} className="text-ds-gray-700 shrink-0" />
        </Link>
      ))}
      {!expanded && remaining > 0 && (
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 py-2 text-left transition-colors"
        >
          {remaining} more...
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// NovaStatus — compact horizontal status row
// ---------------------------------------------------------------------------

function NovaStatus({
  fleetData,
  automationData,
  briefingData,
  loading,
}: {
  fleetData: FleetHealthResponse | undefined;
  automationData: { watcher?: { enabled: boolean; interval_minutes: number }; reminders?: unknown[] } | undefined;
  briefingData: BriefingGetResponse | undefined;
  loading: boolean;
}) {
  if (loading) {
    return <div className="h-12 animate-pulse rounded-lg bg-ds-gray-100 border border-ds-gray-400" />;
  }

  const channels = fleetData?.channels ?? [];
  const watcher = automationData?.watcher;
  const briefingEntry = briefingData?.entry;
  const lastBriefingTime = briefingEntry
    ? formatRelativeTime(briefingEntry.generated_at)
    : "None";

  return (
    <div className="flex items-stretch border border-ds-gray-400 rounded-lg overflow-hidden">
      {/* Channels */}
      <div className="flex-1 px-3 py-2 flex flex-col gap-1">
        <span className="text-label-12 text-ds-gray-700">Channels</span>
        <div className="flex flex-wrap gap-2">
          {channels.length === 0 ? (
            <span className="text-copy-13 font-mono text-ds-gray-700">—</span>
          ) : (
            channels.map((ch) => (
              <span key={ch.name} className="flex items-center gap-1 text-copy-13">
                <span
                  className={`w-1.5 h-1.5 rounded-full shrink-0 ${
                    ch.status === "configured" ? "bg-green-700" : "bg-ds-gray-500"
                  }`}
                />
                <span className="font-mono capitalize text-ds-gray-900">{ch.name}</span>
              </span>
            ))
          )}
        </div>
      </div>

      {/* Divider */}
      <div className="border-r border-ds-gray-400" />

      {/* Watcher */}
      <div className="flex-1 px-3 py-2 flex flex-col gap-1">
        <span className="text-label-12 text-ds-gray-700">Watcher</span>
        <span className="text-copy-13 font-mono text-ds-gray-900">
          {watcher
            ? watcher.enabled
              ? `On / every ${watcher.interval_minutes}m`
              : "Disabled"
            : "—"}
        </span>
      </div>

      {/* Divider */}
      <div className="border-r border-ds-gray-400" />

      {/* Last briefing */}
      <div className="flex-1 px-3 py-2 flex flex-col gap-1">
        <span className="text-label-12 text-ds-gray-700">Last Briefing</span>
        <span
          className="text-copy-13 font-mono text-ds-gray-900"
          suppressHydrationWarning
        >
          {lastBriefingTime}
        </span>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// EventDetailPanel — expandable row detail
// ---------------------------------------------------------------------------

function EventDetailPanel({
  type,
  time,
  summary,
}: {
  type: string;
  time: string;
  summary: string;
}) {
  return (
    <div className="px-4 py-2 bg-ds-gray-alpha-100 border-t border-ds-gray-400 flex flex-col gap-1.5">
      <p className="text-copy-13 text-ds-gray-1000 leading-snug">{summary}</p>
      <div className="flex items-center gap-3 flex-wrap">
        <span className="text-label-12 px-1.5 py-0.5 rounded bg-ds-gray-200 text-ds-gray-900 border border-ds-gray-400">
          {type}
        </span>
        <span
          className="text-copy-13 text-ds-gray-700 font-mono tabular-nums"
          suppressHydrationWarning
        >
          {new Date(time).toLocaleString()}
        </span>
        <Link
          href={getViewLink(type)}
          className="flex items-center gap-1 text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors ml-auto"
        >
          View
          <ExternalLink size={11} />
        </Link>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// CategoryGroup — single group with header + collapsible events
// ---------------------------------------------------------------------------

interface MergedFeedEvent {
  id: string;
  type: string;
  time: string;
  summary: string;
  severity: "error" | "warning" | "routine";
  isWs?: boolean;
}

interface CategoryGroupProps {
  type: "message" | "obligation" | "session" | "system";
  events: MergedFeedEvent[];
  isExpanded: boolean;
  onToggle: () => void;
  newCount: number;
  onBadgeClick: () => void;
}

function CategoryGroup({
  type,
  events,
  isExpanded,
  onToggle,
  newCount,
  onBadgeClick,
}: CategoryGroupProps) {
  const [expandedEventId, setExpandedEventId] = useState<string | null>(null);

  if (events.length === 0) return null;

  const displayLabel =
    type === "message"
      ? "Messages"
      : type === "obligation"
        ? "Obligations"
        : type === "session"
          ? "Sessions"
          : "System";

  const icon = type === "message"
    ? <MessageSquare size={13} className="text-ds-gray-700 shrink-0" />
    : type === "obligation"
      ? <CheckSquare size={13} className="text-ds-gray-700 shrink-0" />
      : type === "session"
        ? <MonitorPlay size={13} className="text-ds-gray-700 shrink-0" />
        : <Activity size={13} className="text-ds-gray-700 shrink-0" />;

  // Compute a short summary text
  const summaryText = (() => {
    if (type === "message") {
      const inbound = events.filter((e) => !e.isWs && e.summary.toLowerCase().includes("inbound")).length;
      return `${events.length} message${events.length !== 1 ? "s" : ""}`;
    }
    return `${events.length} event${events.length !== 1 ? "s" : ""}`;
  })();

  const mostRecent = events[0];
  const visibleEvents = isExpanded ? events.slice(0, 50) : events.slice(0, 3);

  return (
    <div className="border-b border-ds-gray-400 last:border-b-0">
      {/* Group header */}
      <div className="flex items-center gap-2 py-1.5 px-1">
        <button
          type="button"
          onClick={onToggle}
          className="flex items-center gap-2 flex-1 min-w-0 hover:text-ds-gray-1000 transition-colors text-left"
        >
          {isExpanded ? (
            <ChevronDown size={13} className="text-ds-gray-700 shrink-0" />
          ) : (
            <ChevronRight size={13} className="text-ds-gray-700 shrink-0" />
          )}
          {icon}
          <span className="text-label-13 text-ds-gray-900 font-medium">{displayLabel}</span>
          <span className="text-label-12 font-mono text-ds-gray-700 px-1.5 py-0.5 rounded bg-ds-gray-alpha-100">
            {events.length}
          </span>
          <span className="text-copy-13 text-ds-gray-700 flex-1 min-w-0 truncate">
            {summaryText}
          </span>
        </button>
        <div className="flex items-center gap-2">
          {newCount > 0 && (
            <button
              type="button"
              onClick={onBadgeClick}
              className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-blue-700/15 text-blue-700 text-label-12 hover:bg-blue-700/25 transition-colors"
            >
              {newCount} new
            </button>
          )}
          {mostRecent && (
            <span
              className="text-copy-13 text-ds-gray-700 font-mono tabular-nums shrink-0"
              suppressHydrationWarning
            >
              {formatRelativeTime(mostRecent.time)}
            </span>
          )}
          <Link
            href={getViewLink(type)}
            className="text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 transition-colors"
          >
            <ExternalLink size={11} />
          </Link>
        </div>
      </div>

      {/* Events (compact rows) */}
      {visibleEvents.map((ev) => {
        const { rowBg, leftBorder } = getSeverityConfig(ev.severity);
        const isDetailExpanded = expandedEventId === ev.id;

        return (
          <div key={ev.id} className={rowBg}>
            <button
              type="button"
              onClick={() => setExpandedEventId(isDetailExpanded ? null : ev.id)}
              className={[
                "w-full flex items-center gap-2 py-1 pl-8 pr-1 hover:bg-ds-gray-alpha-100 transition-colors text-left",
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
            {isDetailExpanded && (
              <EventDetailPanel
                type={ev.type}
                time={ev.time}
                summary={ev.summary}
              />
            )}
          </div>
        );
      })}

      {isExpanded && events.length > 50 && (
        <p className="text-copy-13 text-ds-gray-700 pl-8 py-1">
          + {events.length - 50} more events
        </p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// GroupedActivitySummaries
// ---------------------------------------------------------------------------

function GroupedActivitySummaries({
  events,
  wsEvents,
  loading,
  badgeCounters,
  onBadgeReset,
}: {
  events: ActivityFeedEvent[];
  wsEvents: WsActivityEvent[];
  loading: boolean;
  badgeCounters: Record<string, number>;
  onBadgeReset: (type: string) => void;
}) {
  const [expandedGroup, setExpandedGroup] = useState<string | null>(null);

  if (loading) {
    return (
      <div className="flex flex-col gap-px">
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="h-10 animate-pulse rounded bg-ds-gray-100" />
        ))}
      </div>
    );
  }

  // Build merged event list
  const allMerged: MergedFeedEvent[] = [];

  for (const ws of wsEvents) {
    allMerged.push({
      id: `ws-${ws.id}`,
      type: ws.type,
      time: new Date(ws.ts).toISOString(),
      summary: ws.label,
      severity: "routine",
      isWs: true,
    });
  }

  for (const ev of events) {
    allMerged.push({
      id: ev.id,
      type: ev.type,
      time: ev.timestamp,
      summary: ev.summary,
      severity: getEventSeverity(ev),
    });
  }

  // Group by type — map diary to "system"
  const groups: Record<string, MergedFeedEvent[]> = {
    message: [],
    obligation: [],
    session: [],
    system: [],
  };

  for (const ev of allMerged) {
    const key = ev.type === "diary" ? "system" : ev.type;
    if (key in groups) {
      groups[key]!.push(ev);
    }
  }

  // Sort each group by time desc
  for (const key of Object.keys(groups)) {
    groups[key]!.sort((a, b) => new Date(b.time).getTime() - new Date(a.time).getTime());
  }

  const groupOrder: ("message" | "obligation" | "session" | "system")[] = [
    "message",
    "obligation",
    "session",
    "system",
  ];

  const handleToggle = (type: string) => {
    setExpandedGroup((prev) => (prev === type ? null : type));
  };

  const hasAnyEvents = groupOrder.some((t) => (groups[t]?.length ?? 0) > 0);

  if (!hasAnyEvents) {
    return (
      <p className="text-copy-13 text-ds-gray-700 py-3">No activity in the feed</p>
    );
  }

  return (
    <div className="divide-y divide-ds-gray-400 border border-ds-gray-400 rounded-lg overflow-hidden">
      {groupOrder.map((type) => (
        <CategoryGroup
          key={type}
          type={type}
          events={groups[type] ?? []}
          isExpanded={expandedGroup === type}
          onToggle={() => handleToggle(type)}
          newCount={badgeCounters[type] ?? 0}
          onBadgeClick={() => onBadgeReset(type)}
        />
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// QuickAdd — collapsible obligation input
// ---------------------------------------------------------------------------

function QuickAdd() {
  const trpc = useTRPC();
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState("");
  const [result, setResult] = useState<"success" | "error" | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const { mutate: createObligation, isPending: creating } = useMutation(
    trpc.obligation.create.mutationOptions({
      onSuccess: () => {
        setInput("");
        setResult("success");
        setTimeout(() => {
          setResult(null);
          setOpen(false);
        }, 1500);
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
      {!open ? (
        <button
          type="button"
          onClick={() => setOpen(true)}
          className="flex items-center gap-2 py-2 px-3 border border-ds-gray-400 rounded-lg text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors w-full text-left"
        >
          <Plus size={13} className="shrink-0" />
          Add obligation...
        </button>
      ) : (
        <div className="flex flex-col gap-1.5 border border-ds-gray-400 rounded-lg px-3 py-2">
          <div className="flex items-center gap-2">
            <CheckSquare size={14} className="text-ds-gray-700 shrink-0" />
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleCreate();
                if (e.key === "Escape") {
                  setOpen(false);
                  setInput("");
                }
              }}
              placeholder="Add obligation..."
              className="flex-1 min-w-0 bg-transparent text-copy-13 text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-hidden"
              disabled={creating}
              autoFocus
            />
            <button
              type="button"
              onClick={handleCreate}
              disabled={creating || !input.trim()}
              className="flex items-center justify-center px-2 py-0.5 rounded text-label-12 border border-ds-gray-400 text-ds-gray-900 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-40"
            >
              {creating ? <Loader2 size={13} className="animate-spin" /> : <Send size={13} />}
            </button>
            <button
              type="button"
              onClick={() => {
                setOpen(false);
                setInput("");
                setResult(null);
              }}
              className="flex items-center justify-center w-6 h-6 rounded text-ds-gray-700 hover:text-ds-gray-1000 transition-colors"
            >
              <X size={13} />
            </button>
          </div>
          {result === "success" && (
            <p className="text-copy-13 text-green-700 px-1">Created</p>
          )}
          {result === "error" && (
            <p className="text-copy-13 text-red-700 px-1">{errorMsg}</p>
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Dashboard Page
// ---------------------------------------------------------------------------

export default function DashboardPage() {
  const trpc = useTRPC();
  // --- 1. Local state ---
  const [wsEvents, setWsEvents] = useState<WsActivityEvent[]>([]);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [, setTick] = useState(0);
  const [badgeCounters, setBadgeCounters] = useState<Record<string, number>>({});
  const wsIdRef = useRef(0);
  const queryClient = useQueryClient();

  // --- 2. Context/routing ---
  const daemonStatus = useDaemonStatus();
  const isDisconnected = daemonStatus !== "connected";

  // --- 3. WebSocket subscription — increment badge counters instead of prepending ---
  useDaemonEvents((ev) => {
    const label =
      typeof ev.payload === "object" &&
      ev.payload !== null &&
      "label" in ev.payload
        ? String((ev.payload as { label?: unknown }).label)
        : ev.type;

    // Add to ws events for feed display
    setWsEvents((prev) =>
      [
        {
          id: String(++wsIdRef.current),
          type: ev.type,
          label,
          ts: ev.ts,
        },
        ...prev,
      ].slice(0, 50),
    );

    // Increment badge counter for the category
    const catKey = ev.type === "diary" ? "system" : ev.type;
    setBadgeCounters((prev) => ({
      ...prev,
      [catKey]: (prev[catKey] ?? 0) + 1,
    }));
  });

  // --- 4. Queries ---
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
  const automationQuery = useQuery(
    trpc.automation.getAll.queryOptions(undefined, refetchOpts),
  );

  // --- 5. Derived values ---
  const feedData = feedQuery.data as ActivityFeedGetResponse | undefined;
  const feedEvents = feedData?.events ?? [];
  const oblData = oblQuery.data as ObligationsGetResponse | undefined;
  const obligations = (oblData?.obligations ?? []) as ApiObligation[];
  const msgData = msgQuery.data as { messages: { sender: string; timestamp: string; content: string }[] } | undefined;
  const allMessages = msgData?.messages ?? [];
  const briefData = briefQuery.data as BriefingGetResponse | undefined;
  const fleetRaw = fleetQuery.data as FleetHealthResponse | undefined;
  const automationRaw = automationQuery.data as {
    watcher?: { enabled: boolean; interval_minutes: number };
    reminders?: unknown[];
  } | undefined;

  const loading = feedQuery.isLoading || oblQuery.isLoading;
  const error = feedQuery.error ?? oblQuery.error ?? null;

  const lastFetchedAt = feedQuery.dataUpdatedAt || Date.now();
  const updatedAgo = formatSecondsAgo(Date.now() - lastFetchedAt);
  const lastFetchedIso = new Date(lastFetchedAt).toISOString();
  const isRefreshing = feedQuery.isFetching;

  // --- 6. Refresh all ---
  const handleRefreshAll = () => {
    void queryClient.invalidateQueries();
  };

  // --- 7. Badge reset handler ---
  const handleBadgeReset = (type: string) => {
    setBadgeCounters((prev) => ({ ...prev, [type]: 0 }));
    void queryClient.invalidateQueries({ queryKey: trpc.system.activityFeed.queryKey() });
  };

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
      <div
        className={`flex flex-col animate-fade-in-up transition-opacity ${isDisconnected ? "opacity-50" : ""}`}
      >
        {error && (
          <div className="border-b border-ds-gray-400 pb-4 mb-4">
            <ErrorBanner
              message="Failed to load dashboard data"
              detail={error.message}
              onRetry={handleRefreshAll}
            />
          </div>
        )}

        {/* Action Items */}
        <div className="border-b border-ds-gray-400 pb-4 mb-4">
          <SectionHeader
            label="Action Items"
            count={
              obligations.filter((o) => o.status === "open" || o.status === "in_progress").length
            }
          />
          <div className="mt-2">
            <ActionItems
              obligations={obligations}
              messages={allMessages}
              oblLoading={oblQuery.isLoading}
              msgLoading={msgQuery.isLoading}
            />
          </div>
        </div>

        {/* Nova Status */}
        <div className="border-b border-ds-gray-400 pb-4 mb-4">
          <SectionHeader label="Nova Status" />
          <div className="mt-2">
            <NovaStatus
              fleetData={fleetRaw}
              automationData={automationRaw}
              briefingData={briefData}
              loading={fleetQuery.isLoading || automationQuery.isLoading}
            />
          </div>
        </div>

        {/* Activity Summaries */}
        <div className="border-b border-ds-gray-400 pb-4 mb-4">
          <SectionHeader
            label="Activity"
            count={feedEvents.length + wsEvents.length}
          />
          <div className="mt-2">
            <GroupedActivitySummaries
              events={feedEvents}
              wsEvents={wsEvents}
              loading={loading}
              badgeCounters={badgeCounters}
              onBadgeReset={handleBadgeReset}
            />
          </div>
        </div>

        {/* Quick Add */}
        <div className="pb-4">
          <QuickAdd />
        </div>
      </div>
    </PageShell>
  );
}
