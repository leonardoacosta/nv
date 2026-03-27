"use client";

import {
  useEffect,
  useState,
  useCallback,
  useRef,
  useDeferredValue,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Search,
  X,
  MessageSquare,
  Terminal,
  ArrowUpRight,
  ArrowDownLeft,
  Clock,
  Zap,
  Gauge,
  ChevronDown,
  ChevronUp,
  ArrowUp,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import { channelAccentColor } from "@/lib/channel-colors";
import type { StoredMessage, MessagesGetResponse } from "@/types/api";
import { apiFetch } from "@/lib/api-client";
import { useApiQuery } from "@/lib/hooks/use-api-query";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PAGE_SIZE = 50;
const LOAD_MORE_THRESHOLD = 5; // rows from bottom before fetching more

const CHANNEL_ICONS: Record<string, React.ReactNode> = {
  telegram: <MessageSquare size={12} className="text-[#229ED9]" />,
  discord: <MessageSquare size={12} className="text-[#5865F2]" />,
  slack: <MessageSquare size={12} className="text-[#E01E5A]" />,
  cli: <Terminal size={12} className="text-ds-gray-1000" />,
  api: <Zap size={12} className="text-red-700" />,
};

type DateRange = "today" | "7d" | "all";
type SortMode = "newest" | "oldest" | "channel" | "direction";
type DirectionFilter = "all" | "inbound" | "outbound";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function channelIcon(channel: string): React.ReactNode {
  const key = channel.toLowerCase();
  return CHANNEL_ICONS[key] ?? <MessageSquare size={12} className="text-ds-gray-900" />;
}

function formatTs(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatRelativeTs(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  return d.toLocaleDateString([], { month: "short", day: "numeric" });
}

function truncate(text: string, max = 100): string {
  return text.length <= max ? text : `${text.slice(0, max)}…`;
}

function isInRange(iso: string, range: DateRange): boolean {
  if (range === "all") return true;
  const ts = new Date(iso).getTime();
  const now = Date.now();
  if (range === "today") {
    const startOfDay = new Date();
    startOfDay.setHours(0, 0, 0, 0);
    return ts >= startOfDay.getTime();
  }
  return ts >= now - 7 * 24 * 60 * 60 * 1000;
}

// ---------------------------------------------------------------------------
// Contact resolver hook
// ---------------------------------------------------------------------------

function useContactResolver(messages: StoredMessage[]) {
  const cacheRef = useRef<Map<string, string>>(new Map());
  const [, forceUpdate] = useState(0);

  useEffect(() => {
    const unique = Array.from(new Set(messages.map((m) => m.sender).filter(Boolean)));
    const uncached = unique.filter((s) => !cacheRef.current.has(s));
    if (uncached.length === 0) return;

    void apiFetch("/api/contacts/resolve", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ senders: uncached }),
    })
      .then((res) => (res.ok ? res.json() : null))
      .then((data: Record<string, string> | null) => {
        if (!data) return;
        // Cache all resolved names, mark unresolved ones with empty to avoid refetch
        for (const s of uncached) {
          cacheRef.current.set(s, data[s] ?? s);
        }
        forceUpdate((n) => n + 1);
      })
      .catch(() => {
        // On failure mark as unresolved so we don't loop
        for (const s of uncached) {
          cacheRef.current.set(s, s);
        }
      });
  }, [messages]);

  const resolve = useCallback(
    (sender: string): string => cacheRef.current.get(sender) ?? sender,
    [],
  );

  return resolve;
}

// ---------------------------------------------------------------------------
// Time grouping helpers
// ---------------------------------------------------------------------------

interface GroupHeader {
  kind: "group-header";
  label: string;
  key: string;
}

interface MessageVirtualItem {
  kind: "message";
  msg: StoredMessage;
}

type VirtualRow = GroupHeader | MessageVirtualItem;

function hourKey(iso: string): string {
  const d = new Date(iso);
  d.setMinutes(0, 0, 0);
  return d.toISOString();
}

function formatGroupLabel(isoHourKey: string): string {
  const d = new Date(isoHourKey);
  const now = new Date();
  const isToday =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();

  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const isYesterday =
    d.getFullYear() === yesterday.getFullYear() &&
    d.getMonth() === yesterday.getMonth() &&
    d.getDate() === yesterday.getDate();

  const datePrefix = isToday
    ? "Today"
    : isYesterday
      ? "Yesterday"
      : d.toLocaleDateString([], { month: "short", day: "numeric" });

  const hourStart = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const end = new Date(d);
  end.setHours(end.getHours() + 1);
  const hourEnd = end.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

  return `${datePrefix}, ${hourStart} \u2013 ${hourEnd}`;
}

function buildVirtualRows(messages: StoredMessage[], sort: SortMode): VirtualRow[] {
  const rows: VirtualRow[] = [];

  if (sort === "channel") {
    const grouped = new Map<string, StoredMessage[]>();
    for (const msg of messages) {
      const key = msg.channel.toLowerCase();
      if (!grouped.has(key)) grouped.set(key, []);
      grouped.get(key)!.push(msg);
    }
    const channels = Array.from(grouped.keys()).sort();
    for (const ch of channels) {
      rows.push({ kind: "group-header", label: `Channel: ${ch}`, key: `channel-${ch}` });
      for (const msg of grouped.get(ch)!) {
        rows.push({ kind: "message", msg });
      }
    }
    return rows;
  }

  if (sort === "direction") {
    const inbound = messages.filter((m) => m.direction === "inbound");
    const outbound = messages.filter((m) => m.direction === "outbound");
    if (inbound.length > 0) {
      rows.push({ kind: "group-header", label: "Inbound", key: "dir-inbound" });
      for (const msg of inbound) rows.push({ kind: "message", msg });
    }
    if (outbound.length > 0) {
      rows.push({ kind: "group-header", label: "Outbound", key: "dir-outbound" });
      for (const msg of outbound) rows.push({ kind: "message", msg });
    }
    return rows;
  }

  // Newest / Oldest — hour grouping
  let currentKey = "";
  for (const msg of messages) {
    const key = hourKey(msg.timestamp);
    if (key !== currentKey) {
      currentKey = key;
      rows.push({ kind: "group-header", label: formatGroupLabel(key), key: `hour-${key}` });
    }
    rows.push({ kind: "message", msg });
  }
  return rows;
}

// ---------------------------------------------------------------------------
// Type badge
// ---------------------------------------------------------------------------

function TypeBadge({ type }: { type: StoredMessage["type"] }) {
  if (type === "conversation") return null;
  if (type === "tool-call") {
    return (
      <span className="shrink-0 px-1 py-0.5 rounded text-[10px] font-mono bg-amber-700/15 text-amber-700">
        tool
      </span>
    );
  }
  return (
    <span className="shrink-0 px-1 py-0.5 rounded text-[10px] font-mono bg-ds-gray-alpha-200 text-ds-gray-900">
      sys
    </span>
  );
}

// ---------------------------------------------------------------------------
// MessageRowDense
// ---------------------------------------------------------------------------

interface MessageRowDenseProps {
  msg: StoredMessage;
  expanded: boolean;
  active: boolean;
  onToggle: () => void;
  resolvedName: string;
  measureRef: (el: HTMLElement | null) => void;
}

function MessageRowDense({
  msg,
  expanded,
  active,
  onToggle,
  resolvedName,
  measureRef,
}: MessageRowDenseProps) {
  const isInbound = msg.direction === "inbound";
  const channelKey = msg.channel.toLowerCase();
  const cIcon = channelIcon(channelKey);
  const accent = channelAccentColor(channelKey);

  return (
    <div
      ref={measureRef}
      data-message-id={msg.id}
      style={{ borderLeft: `3px solid ${accent}` }}
      className="border-b border-ds-gray-400 last:border-0"
    >
      {/* Dense row — 28-32px target height */}
      <button
        type="button"
        onClick={onToggle}
        className={[
          "w-full text-left flex items-center gap-2 py-1.5 px-3 hover:bg-ds-gray-200 transition-colors",
          active ? "bg-ds-gray-alpha-100" : "",
        ].join(" ")}
      >
        {/* Direction icon */}
        <div className="shrink-0">
          {isInbound ? (
            <ArrowDownLeft size={12} className="text-green-700" />
          ) : (
            <ArrowUpRight size={12} className="text-amber-700" />
          )}
        </div>

        {/* Channel icon */}
        <div className="shrink-0">{cIcon}</div>

        {/* Resolved sender name */}
        <span className="shrink-0 text-copy-13 text-ds-gray-1000 font-medium w-[72px] truncate hidden md:inline">
          {resolvedName || "\u2014"}
        </span>

        {/* Message preview */}
        <span className="flex-1 min-w-0 text-copy-13 text-ds-gray-900 truncate">
          {truncate(msg.content)}
        </span>

        {/* Type badge */}
        <TypeBadge type={msg.type} />

        {/* Latency badge */}
        {msg.response_time_ms !== null && msg.response_time_ms !== undefined && (
          <span className="shrink-0 flex items-center gap-0.5 px-1 py-0.5 rounded text-[10px] font-mono bg-ds-gray-100 border border-ds-gray-400 text-ds-gray-900 hidden lg:flex">
            <Gauge size={8} />
            {msg.response_time_ms}ms
          </span>
        )}

        {/* Timestamp */}
        <span
          className="shrink-0 text-copy-13 text-ds-gray-900 font-mono hidden sm:inline"
          suppressHydrationWarning
        >
          {formatRelativeTs(msg.timestamp)}
        </span>
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="px-4 pb-4 pt-1 space-y-4 bg-ds-gray-100/30">
          <section className="space-y-1.5">
            <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest font-semibold">
              Message content
            </p>
            <pre className="text-copy-13 text-ds-gray-1000 whitespace-pre-wrap break-words font-mono bg-ds-bg-100 rounded-lg p-3 border border-ds-gray-400 max-h-64 overflow-y-auto">
              {msg.content}
            </pre>
          </section>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">Direction</p>
              <p className="text-copy-13 font-medium text-ds-gray-1000 capitalize">{msg.direction}</p>
            </div>
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">Channel</p>
              <p className="text-copy-13 font-medium" style={{ color: accent }}>{msg.channel}</p>
            </div>
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">Sender</p>
              <p className="text-copy-13 font-medium text-ds-gray-1000">{msg.sender || "\u2014"}</p>
            </div>
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">Timestamp</p>
              <p className="text-copy-13 font-mono text-ds-gray-1000" suppressHydrationWarning>
                {formatTs(msg.timestamp)}
              </p>
            </div>
            {msg.response_time_ms !== null && msg.response_time_ms !== undefined && (
              <div className="space-y-0.5">
                <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">Latency</p>
                <p className="text-copy-13 font-mono text-ds-gray-1000">{msg.response_time_ms}ms</p>
              </div>
            )}
            {msg.tokens_in !== null && msg.tokens_in !== undefined && (
              <div className="space-y-0.5">
                <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">Tokens in</p>
                <p className="text-copy-13 font-mono text-ds-gray-1000">{msg.tokens_in.toLocaleString()}</p>
              </div>
            )}
            {msg.tokens_out !== null && msg.tokens_out !== undefined && (
              <div className="space-y-0.5">
                <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">Tokens out</p>
                <p className="text-copy-13 font-mono text-ds-gray-1000">{msg.tokens_out.toLocaleString()}</p>
              </div>
            )}
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">Type</p>
              <p className="text-copy-13 font-medium text-ds-gray-1000 capitalize">{msg.type}</p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Group header row
// ---------------------------------------------------------------------------

function GroupHeaderRow({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 px-4 py-2 bg-ds-gray-100/60 sticky top-0 z-10">
      <Clock size={11} className="text-ds-gray-900 shrink-0" />
      <span className="text-[11px] font-mono font-medium text-ds-gray-900 tracking-wide">
        {label}
      </span>
      <div className="flex-1 h-px bg-ds-gray-400" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Faceted filter bar
// ---------------------------------------------------------------------------

interface FilterBarProps {
  search: string;
  onSearchChange: (v: string) => void;
  channels: string[];
  channelFilter: string;
  onChannelFilter: (v: string) => void;
  direction: DirectionFilter;
  onDirection: (v: DirectionFilter) => void;
  dateRange: DateRange;
  onDateRange: (v: DateRange) => void;
  sort: SortMode;
  onSort: (v: SortMode) => void;
  hasActiveFilters: boolean;
  onClearAll: () => void;
}

function FacetedFilterBar({
  search,
  onSearchChange,
  channels,
  channelFilter,
  onChannelFilter,
  direction,
  onDirection,
  dateRange,
  onDateRange,
  sort,
  onSort,
  hasActiveFilters,
  onClearAll,
}: FilterBarProps) {
  return (
    <div className="flex items-center gap-2 flex-wrap">
      {/* Search */}
      <div className="relative min-w-[160px] max-w-xs flex-1">
        <Search
          size={13}
          className="absolute left-2.5 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
        />
        <input
          type="search"
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Search messages…"
          className="w-full pl-8 pr-7 py-1.5 surface-inset text-copy-13 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors h-[36px]"
        />
        {search && (
          <button
            type="button"
            onClick={() => onSearchChange("")}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
            aria-label="Clear search"
          >
            <X size={12} />
          </button>
        )}
      </div>

      {/* Channel dropdown */}
      <select
        value={channelFilter}
        onChange={(e) => onChannelFilter(e.target.value)}
        className="h-[36px] px-2 py-1.5 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
      >
        <option value="all">All channels</option>
        {channels.map((ch) => (
          <option key={ch} value={ch}>
            {ch}
          </option>
        ))}
      </select>

      {/* Direction dropdown */}
      <select
        value={direction}
        onChange={(e) => onDirection(e.target.value as DirectionFilter)}
        className="h-[36px] px-2 py-1.5 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
      >
        <option value="all">All directions</option>
        <option value="inbound">Inbound</option>
        <option value="outbound">Outbound</option>
      </select>

      {/* Date range toggle */}
      <div className="flex items-center gap-0.5 p-1 h-[36px] rounded-lg bg-ds-gray-100 border border-ds-gray-400">
        {(["today", "7d", "all"] as DateRange[]).map((r) => (
          <button
            key={r}
            type="button"
            onClick={() => onDateRange(r)}
            className={[
              "px-2 py-0.5 rounded text-[11px] font-medium transition-colors",
              dateRange === r
                ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                : "text-ds-gray-900 hover:text-ds-gray-1000",
            ].join(" ")}
          >
            {r === "today" ? "Today" : r === "7d" ? "7d" : "All"}
          </button>
        ))}
      </div>

      {/* Sort dropdown */}
      <select
        value={sort}
        onChange={(e) => onSort(e.target.value as SortMode)}
        className="h-[36px] px-2 py-1.5 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
      >
        <option value="newest">Newest first</option>
        <option value="oldest">Oldest first</option>
        <option value="channel">By channel</option>
        <option value="direction">By direction</option>
      </select>

      {/* Clear all */}
      {hasActiveFilters && (
        <button
          type="button"
          onClick={onClearAll}
          className="h-[36px] flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors"
        >
          <X size={11} />
          Clear all
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Messages Page
// ---------------------------------------------------------------------------

export default function MessagesPage() {
  // 1. State
  const [allMessages, setAllMessages] = useState<StoredMessage[]>([]);
  const [total, setTotal] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(true);
  const [searchInput, setSearchInput] = useState("");
  const deferredSearch = useDeferredValue(searchInput);
  const [channelFilter, setChannelFilter] = useState("all");
  const [dateRange, setDateRange] = useState<DateRange>("all");
  const [direction, setDirection] = useState<DirectionFilter>("all");
  const [sort, setSort] = useState<SortMode>("newest");
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  const [scrolledPast20, setScrolledPast20] = useState(false);

  // Refs
  const parentRef = useRef<HTMLDivElement>(null);
  const loadingMoreRef = useRef(false);
  const searchDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 2. Query params for initial load
  const initialParams: Record<string, string> = {
    limit: String(PAGE_SIZE),
    offset: "0",
    sort: sort === "oldest" ? "asc" : "desc",
  };
  if (deferredSearch) initialParams.search = deferredSearch;
  if (channelFilter !== "all") initialParams.channel = channelFilter;

  const initialQuery = useApiQuery<MessagesGetResponse>("/api/messages", {
    params: initialParams,
  });

  const loading = initialQuery.isLoading;
  const error = initialQuery.error;

  // Sync query data into local state for infinite scroll append
  useEffect(() => {
    if (initialQuery.data) {
      const fetched = initialQuery.data.messages ?? [];
      setAllMessages(fetched);
      setOffset(fetched.length);
      setTotal(initialQuery.data.total ?? fetched.length);
      setHasMore(fetched.length === PAGE_SIZE);
    }
  }, [initialQuery.data]);

  // 3. Contact resolver
  const resolveContact = useContactResolver(allMessages);

  // 4. Load more (append) — uses raw apiFetch since it appends to local state
  const fetchMoreMessages = useCallback(
    async () => {
      setLoadingMore(true);
      try {
        const params = new URLSearchParams({
          limit: String(PAGE_SIZE),
          offset: String(offset),
          sort: sort === "oldest" ? "asc" : "desc",
        });
        if (deferredSearch) params.set("search", deferredSearch);
        if (channelFilter !== "all") params.set("channel", channelFilter);

        const res = await apiFetch(`/api/messages?${params.toString()}`);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data = (await res.json()) as MessagesGetResponse;
        const fetched = data.messages ?? [];

        setAllMessages((prev) => [...prev, ...fetched]);
        setOffset((prev) => prev + fetched.length);
        setTotal(data.total ?? fetched.length);
        setHasMore(fetched.length === PAGE_SIZE);
      } catch {
        // Non-critical load-more failure
      } finally {
        setLoadingMore(false);
        loadingMoreRef.current = false;
      }
    },
    [offset, sort, deferredSearch, channelFilter],
  );

  // 5. Search debounce
  const handleSearchChange = (value: string) => {
    setSearchInput(value);
    if (searchDebounceRef.current) clearTimeout(searchDebounceRef.current);
    searchDebounceRef.current = setTimeout(() => {
      // useDeferredValue handles re-render
    }, 300);
  };

  // 6. Derived — apply client-side filters (date range, direction)
  const filtered = allMessages.filter((m) => {
    if (dateRange !== "all" && !isInRange(m.timestamp, dateRange)) return false;
    if (direction === "inbound" && m.direction !== "inbound") return false;
    if (direction === "outbound" && m.direction !== "outbound") return false;
    return true;
  });

  // Sort oldest puts them in ascending order (API already returns in order, just need to reverse display)
  const sorted =
    sort === "oldest"
      ? [...filtered].reverse()
      : sort === "channel"
        ? [...filtered].sort((a, b) => a.channel.localeCompare(b.channel))
        : sort === "direction"
          ? [...filtered].sort((a, b) => {
              if (a.direction === b.direction) return 0;
              return a.direction === "inbound" ? -1 : 1;
            })
          : filtered;

  const virtualRows = buildVirtualRows(sorted, sort);
  const messageIndexMap = new Map<number, number>(); // msgId -> virtualRow index
  virtualRows.forEach((row, idx) => {
    if (row.kind === "message") {
      messageIndexMap.set(row.msg.id, idx);
    }
  });

  // Only message rows count for active index purposes
  const messageVirtualRows = virtualRows.filter((r): r is MessageVirtualItem => r.kind === "message");

  const channels = Array.from(new Set(allMessages.map((m) => m.channel.toLowerCase()))).sort();

  const hasActiveFilters =
    deferredSearch !== "" ||
    channelFilter !== "all" ||
    dateRange !== "all" ||
    direction !== "all" ||
    sort !== "newest";

  // 7. Virtualizer
  const rowVirtualizer = useVirtualizer({
    count: virtualRows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (index) => {
      const row = virtualRows[index];
      if (!row) return 30;
      if (row.kind === "group-header") return 34;
      if (row.kind === "message" && expandedId === row.msg.id) return 280;
      return 30;
    },
    overscan: 10,
    measureElement: (el) => el.getBoundingClientRect().height,
  });

  // 8. Infinite scroll — load more when near bottom
  useEffect(() => {
    const virtualItems = rowVirtualizer.getVirtualItems();
    if (virtualItems.length === 0) return;
    const lastItem = virtualItems[virtualItems.length - 1];
    if (!lastItem) return;
    const threshold = virtualRows.length - LOAD_MORE_THRESHOLD;
    if (
      lastItem.index >= threshold &&
      hasMore &&
      !loading &&
      !loadingMore &&
      !loadingMoreRef.current
    ) {
      loadingMoreRef.current = true;
      void fetchMoreMessages();
    }
  }, [
    rowVirtualizer.getVirtualItems(),
    virtualRows.length,
    hasMore,
    loading,
    loadingMore,
    fetchMoreMessages,
  ]);

  // 9. Scroll to top button visibility
  useEffect(() => {
    const el = parentRef.current;
    if (!el) return;
    const handleScroll = () => {
      const items = rowVirtualizer.getVirtualItems();
      const firstVisible = items[0];
      setScrolledPast20(firstVisible ? firstVisible.index > 20 : false);
    };
    el.addEventListener("scroll", handleScroll, { passive: true });
    return () => el.removeEventListener("scroll", handleScroll);
  }, [rowVirtualizer]);

  // 10. Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (messageVirtualRows.length === 0) return;
    const current = activeIndex ?? -1;

    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      const next = Math.min(current + 1, messageVirtualRows.length - 1);
      setActiveIndex(next);
      const row = messageVirtualRows[next];
      if (row) {
        const vIdx = messageIndexMap.get(row.msg.id);
        if (vIdx !== undefined) rowVirtualizer.scrollToIndex(vIdx, { align: "auto" });
      }
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      const prev = Math.max(current - 1, 0);
      setActiveIndex(prev);
      const row = messageVirtualRows[prev];
      if (row) {
        const vIdx = messageIndexMap.get(row.msg.id);
        if (vIdx !== undefined) rowVirtualizer.scrollToIndex(vIdx, { align: "auto" });
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (activeIndex !== null) {
        const row = messageVirtualRows[activeIndex];
        if (row) {
          setExpandedId((prev) => (prev === row.msg.id ? null : row.msg.id));
        }
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      setExpandedId(null);
    }
  };

  // 11. Clear all filters
  const handleClearAll = () => {
    setSearchInput("");
    setChannelFilter("all");
    setDateRange("all");
    setDirection("all");
    setSort("newest");
    setExpandedId(null);
    setActiveIndex(null);
  };

  return (
    <PageShell
      title="Messages"
      subtitle={
        !loading && total > 0
          ? `Showing ${filtered.length} of ${total} messages`
          : "Channel message history"
      }
    >
      <div className="space-y-3">
        {error && (
          <ErrorBanner
            message="Failed to load messages"
            detail={error.message}
            onRetry={() => void initialQuery.refetch()}
          />
        )}

        {/* Faceted filter bar */}
        <FacetedFilterBar
          search={searchInput}
          onSearchChange={handleSearchChange}
          channels={channels}
          channelFilter={channelFilter}
          onChannelFilter={(v) => {
            setChannelFilter(v);
            setActiveIndex(null);
          }}
          direction={direction}
          onDirection={(v) => {
            setDirection(v);
            setActiveIndex(null);
          }}
          dateRange={dateRange}
          onDateRange={(v) => {
            setDateRange(v);
            setActiveIndex(null);
          }}
          sort={sort}
          onSort={(v) => {
            setSort(v);
            setActiveIndex(null);
          }}
          hasActiveFilters={hasActiveFilters}
          onClearAll={handleClearAll}
        />

        {/* Messages virtual list */}
        <div className="surface-card overflow-hidden relative">
          {loading ? (
            <div className="divide-y divide-ds-gray-400">
              {Array.from({ length: 10 }).map((_, i) => (
                <div key={i} className="flex items-center gap-2 px-3 py-1.5">
                  <div className="w-3 h-3 rounded-full animate-pulse bg-ds-gray-400" />
                  <div className="w-8 h-3 animate-pulse rounded bg-ds-gray-400" />
                  <div
                    className="flex-1 h-3 animate-pulse rounded bg-ds-gray-400"
                    style={{ opacity: 1 - i * 0.08 }}
                  />
                  <div className="w-14 h-3 animate-pulse rounded bg-ds-gray-400" />
                </div>
              ))}
            </div>
          ) : virtualRows.length === 0 ? (
            <div className="flex flex-col items-center gap-3 py-12">
              <Search size={28} className="text-ds-gray-600" />
              <p className="text-copy-13 text-ds-gray-900 text-center">
                {hasActiveFilters
                  ? "No messages match your filters."
                  : "No messages found."}
              </p>
              {hasActiveFilters && (
                <button
                  type="button"
                  onClick={handleClearAll}
                  className="px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors"
                >
                  Clear filters
                </button>
              )}
            </div>
          ) : (
            <div
              ref={parentRef}
              tabIndex={0}
              onKeyDown={handleKeyDown}
              className="overflow-auto focus:outline-hidden"
              style={{ height: "calc(100vh - 220px)", minHeight: "400px" }}
            >
              <div
                style={{
                  height: `${rowVirtualizer.getTotalSize()}px`,
                  width: "100%",
                  position: "relative",
                }}
              >
                {rowVirtualizer.getVirtualItems().map((virtualItem) => {
                  const row = virtualRows[virtualItem.index];
                  if (!row) return null;

                  if (row.kind === "group-header") {
                    return (
                      <div
                        key={row.key}
                        data-index={virtualItem.index}
                        ref={rowVirtualizer.measureElement}
                        style={{
                          position: "absolute",
                          top: 0,
                          left: 0,
                          width: "100%",
                          transform: `translateY(${virtualItem.start}px)`,
                        }}
                      >
                        <GroupHeaderRow label={row.label} />
                      </div>
                    );
                  }

                  // message row
                  const msgRow = row;
                  const msgActiveIdx = messageVirtualRows.findIndex((r) => r.msg.id === msgRow.msg.id);
                  const isActive = activeIndex === msgActiveIdx;
                  const resolvedName = resolveContact(msgRow.msg.sender);

                  return (
                    <div
                      key={msgRow.msg.id}
                      data-index={virtualItem.index}
                      ref={rowVirtualizer.measureElement}
                      style={{
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: "100%",
                        transform: `translateY(${virtualItem.start}px)`,
                      }}
                    >
                      <MessageRowDense
                        msg={msgRow.msg}
                        expanded={expandedId === msgRow.msg.id}
                        active={isActive}
                        onToggle={() => {
                          setExpandedId((prev) =>
                            prev === msgRow.msg.id ? null : msgRow.msg.id,
                          );
                          setActiveIndex(msgActiveIdx);
                          rowVirtualizer.measure();
                        }}
                        resolvedName={resolvedName}
                        measureRef={(el) => {
                          if (el) {
                            const domEl = el as unknown as HTMLElement & { dataset: DOMStringMap };
                            rowVirtualizer.measureElement(domEl);
                          }
                        }}
                      />
                    </div>
                  );
                })}
              </div>

              {/* Load more indicator */}
              {loadingMore && (
                <div className="flex items-center justify-center py-3">
                  <div className="w-4 h-4 border border-ds-gray-600 border-t-ds-gray-900 rounded-full animate-spin" />
                  <span className="ml-2 text-copy-13 text-ds-gray-900">Loading more…</span>
                </div>
              )}
            </div>
          )}

          {/* Scroll to top floating button */}
          {scrolledPast20 && !loading && (
            <button
              type="button"
              onClick={() => rowVirtualizer.scrollToIndex(0, { align: "start" })}
              className="absolute bottom-4 right-4 w-8 h-8 flex items-center justify-center rounded-full surface-card border border-ds-gray-500 text-ds-gray-900 hover:text-ds-gray-1000 hover:border-ds-gray-600 transition-colors shadow-lg"
              aria-label="Scroll to top"
            >
              <ArrowUp size={14} />
            </button>
          )}
        </div>

        {/* Result summary */}
        {!loading && virtualRows.length > 0 && (
          <p className="text-copy-13 text-ds-gray-900 font-mono">
            {filtered.length} messages
            {total > filtered.length ? ` (${total} total)` : ""}
            {deferredSearch ? ` matching "${deferredSearch}"` : ""}
          </p>
        )}
      </div>
    </PageShell>
  );
}
