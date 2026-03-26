"use client";

import {
  useEffect,
  useState,
  useCallback,
  useRef,
  useDeferredValue,
} from "react";
import {
  Search,
  X,
  ChevronLeft,
  ChevronRight,
  ChevronDown,
  ChevronUp,
  MessageSquare,
  Terminal,
  ArrowUpRight,
  ArrowDownLeft,
  Clock,
  Zap,
  Gauge,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import { channelAccentColor } from "@/lib/channel-colors";
import type { StoredMessage, MessagesGetResponse } from "@/types/api";
import { apiFetch } from "@/lib/api-client";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PAGE_SIZE = 50;

const CHANNEL_ICONS: Record<string, React.ReactNode> = {
  telegram: <MessageSquare size={13} className="text-[#229ED9]" />,
  discord: <MessageSquare size={13} className="text-[#5865F2]" />,
  slack: <MessageSquare size={13} className="text-[#E01E5A]" />,
  cli: <Terminal size={13} className="text-ds-gray-1000" />,
  api: <Zap size={13} className="text-red-700" />,
};

const CHANNEL_COLOR: Record<string, string> = {
  telegram: "text-[#229ED9]",
  discord: "text-[#5865F2]",
  slack: "text-[#E01E5A]",
  cli: "text-ds-gray-1000",
  api: "text-red-700",
};

type DateRange = "today" | "7d" | "all";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function channelIcon(channel: string): React.ReactNode {
  const key = channel.toLowerCase();
  return CHANNEL_ICONS[key] ?? <MessageSquare size={13} className="text-ds-gray-900" />;
}

function channelColor(channel: string): string {
  return CHANNEL_COLOR[channel.toLowerCase()] ?? "text-ds-gray-900";
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

function truncate(text: string, max = 120): string {
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
  // 7d
  return ts >= now - 7 * 24 * 60 * 60 * 1000;
}

// ---------------------------------------------------------------------------
// Time Grouping
// ---------------------------------------------------------------------------

interface MessageGroup {
  label: string;
  messages: StoredMessage[];
}

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

function groupMessagesByHour(messages: StoredMessage[]): MessageGroup[] {
  const groups: MessageGroup[] = [];
  let currentKey = "";
  let currentGroup: MessageGroup | null = null;

  for (const msg of messages) {
    const key = hourKey(msg.timestamp);
    if (key !== currentKey) {
      currentKey = key;
      currentGroup = { label: formatGroupLabel(key), messages: [] };
      groups.push(currentGroup);
    }
    currentGroup!.messages.push(msg);
  }

  return groups;
}

// ---------------------------------------------------------------------------
// Message Row (collapsed + expanded inline)
// ---------------------------------------------------------------------------

interface MessageRowProps {
  msg: StoredMessage;
  expanded: boolean;
  onToggle: () => void;
}

function MessageRow({ msg, expanded, onToggle }: MessageRowProps) {
  const [contentExpanded, setContentExpanded] = useState(false);
  const isInbound = msg.direction === "inbound";
  const channelKey = msg.channel.toLowerCase();
  const cColor = channelColor(channelKey);
  const cIcon = channelIcon(channelKey);
  const accent = channelAccentColor(channelKey);

  // Consider "long" if content exceeds ~180 chars (roughly 3 lines)
  const isLongContent = msg.content.length > 180;

  return (
    <li
      className="border-b border-ds-gray-400 last:border-0"
      style={{ borderLeft: `3px solid ${accent}` }}
    >
      {/* Collapsed row */}
      <button
        type="button"
        onClick={onToggle}
        className="w-full text-left flex items-center gap-3 px-4 py-3 hover:bg-ds-gray-200 transition-colors group"
      >
        {/* Direction icon */}
        <div className="shrink-0 text-ds-gray-900">
          {isInbound ? (
            <ArrowDownLeft size={13} className="text-emerald-400" />
          ) : (
            <ArrowUpRight size={13} className="text-amber-400" />
          )}
        </div>

        {/* Channel icon */}
        <div className={`shrink-0 ${cColor}`}>{cIcon}</div>

        {/* Channel badge with accent tint */}
        <span
          className="shrink-0 text-[11px] font-mono font-medium rounded px-1.5 py-0.5 hidden sm:inline"
          style={{ color: accent, backgroundColor: `${accent}15` }}
        >
          {msg.channel}
        </span>

        {/* Sender */}
        <span className="shrink-0 text-xs text-ds-gray-1000 font-medium min-w-[80px] truncate hidden md:inline">
          {msg.sender || "\u2014"}
        </span>

        {/* Preview */}
        <span className="flex-1 min-w-0 text-sm text-ds-gray-900 truncate">
          {truncate(msg.content)}
        </span>

        {/* Latency badge */}
        {msg.response_time_ms !== null && msg.response_time_ms !== undefined && (
          <span className="shrink-0 flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-mono bg-ds-gray-100 border border-ds-gray-400 text-ds-gray-900 hidden lg:flex">
            <Gauge size={9} />
            {msg.response_time_ms}ms
          </span>
        )}

        {/* Timestamp */}
        <span
          className="shrink-0 text-xs text-ds-gray-900 font-mono hidden sm:inline"
          suppressHydrationWarning
        >
          {formatTs(msg.timestamp)}
        </span>
      </button>

      {/* Inline content expand/collapse (independent of metadata expand) */}
      {isLongContent && !expanded && (
        <div className="px-4 pb-2">
          <div
            className="overflow-hidden transition-[max-height] duration-200 ease-out"
            style={{ maxHeight: contentExpanded ? "2000px" : "0px" }}
          >
            <pre className="text-xs text-ds-gray-1000 whitespace-pre-wrap break-words font-mono py-1">
              {msg.content}
            </pre>
          </div>
          <button
            type="button"
            onClick={() => setContentExpanded((prev) => !prev)}
            className="flex items-center gap-1 text-[11px] text-ds-gray-900 hover:text-ds-gray-1000 transition-colors mt-1"
          >
            {contentExpanded ? (
              <>
                <ChevronUp size={11} />
                Show less
              </>
            ) : (
              <>
                <ChevronDown size={11} />
                Show more
              </>
            )}
          </button>
        </div>
      )}

      {/* Expanded inline content (full metadata panel) */}
      <div className={`height-reveal ${expanded ? "open" : ""}`}>
        <div className="px-4 pb-4 pt-1 space-y-4 bg-ds-gray-100/30">
          {/* Full content */}
          <section className="space-y-1.5">
            <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest font-semibold">
              Message content
            </p>
            <pre className="text-xs text-ds-gray-1000 whitespace-pre-wrap break-words font-mono bg-ds-bg-100 rounded-lg p-3 border border-ds-gray-400 max-h-64 overflow-y-auto">
              {msg.content}
            </pre>
          </section>

          {/* Metadata grid */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">
                Direction
              </p>
              <p className="text-xs font-medium text-ds-gray-1000 capitalize">
                {msg.direction}
              </p>
            </div>
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">
                Channel
              </p>
              <p className={`text-xs font-medium ${channelColor(msg.channel.toLowerCase())}`}>
                {msg.channel}
              </p>
            </div>
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">
                Sender
              </p>
              <p className="text-xs font-medium text-ds-gray-1000">
                {msg.sender || "\u2014"}
              </p>
            </div>
            <div className="space-y-0.5">
              <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">
                Timestamp
              </p>
              <p className="text-xs font-mono text-ds-gray-1000" suppressHydrationWarning>
                {formatTs(msg.timestamp)}
              </p>
            </div>
            {msg.response_time_ms !== null && msg.response_time_ms !== undefined && (
              <div className="space-y-0.5">
                <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">
                  Latency
                </p>
                <p className="text-xs font-mono text-ds-gray-1000">
                  {msg.response_time_ms}ms
                </p>
              </div>
            )}
            {msg.tokens_in !== null && msg.tokens_in !== undefined && (
              <div className="space-y-0.5">
                <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">
                  Tokens in
                </p>
                <p className="text-xs font-mono text-ds-gray-1000">
                  {msg.tokens_in.toLocaleString()}
                </p>
              </div>
            )}
            {msg.tokens_out !== null && msg.tokens_out !== undefined && (
              <div className="space-y-0.5">
                <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest">
                  Tokens out
                </p>
                <p className="text-xs font-mono text-ds-gray-1000">
                  {msg.tokens_out.toLocaleString()}
                </p>
              </div>
            )}
          </div>
        </div>
      </div>
    </li>
  );
}

// ---------------------------------------------------------------------------
// Pagination Controls
// ---------------------------------------------------------------------------

interface PaginationProps {
  page: number;
  pageCount: number;
  onPrev: () => void;
  onNext: () => void;
  disabled: boolean;
}

function PaginationControls({
  page,
  pageCount,
  onPrev,
  onNext,
  disabled,
}: PaginationProps) {
  return (
    <div className="flex items-center justify-between px-4 py-3 border-t border-ds-gray-400">
      <button
        type="button"
        onClick={onPrev}
        disabled={page <= 0 || disabled}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <ChevronLeft size={13} />
        Prev
      </button>

      <span className="text-xs font-mono text-ds-gray-900">
        Page {page + 1}
        {pageCount > 0 ? ` / ${pageCount}` : ""}
      </span>

      <button
        type="button"
        onClick={onNext}
        disabled={pageCount > 0 && page >= pageCount - 1 || disabled}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
      >
        Next
        <ChevronRight size={13} />
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Messages Page
// ---------------------------------------------------------------------------

export default function MessagesPage() {
  // 1. State
  const [messages, setMessages] = useState<StoredMessage[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  const [searchInput, setSearchInput] = useState("");
  const deferredSearch = useDeferredValue(searchInput);
  const [channelFilter, setChannelFilter] = useState("all");
  const [dateRange, setDateRange] = useState<DateRange>("all");
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const searchDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 2. Fetch messages
  const fetchMessages = useCallback(
    async (pg: number, search: string, channel: string) => {
      setLoading(true);
      setError(null);
      try {
        const params = new URLSearchParams({
          limit: String(PAGE_SIZE),
          offset: String(pg * PAGE_SIZE),
        });
        if (search) params.set("search", search);
        if (channel !== "all") params.set("channel", channel);

        const res = await apiFetch(`/api/messages?${params.toString()}`);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data = (await res.json()) as MessagesGetResponse;
        setMessages(data.messages ?? []);
        // Estimate total from whether we got a full page
        setTotal((prev) => {
          const rowsReturned = (data.messages ?? []).length;
          if (rowsReturned < PAGE_SIZE) return pg * PAGE_SIZE + rowsReturned;
          return Math.max(prev, (pg + 1) * PAGE_SIZE + 1);
        });
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load messages");
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  // 3. Effects
  useEffect(() => {
    void fetchMessages(page, deferredSearch, channelFilter);
  }, [fetchMessages, page, deferredSearch, channelFilter]);

  // Reset to page 0 when search/filter changes
  useEffect(() => {
    setPage(0);
  }, [deferredSearch, channelFilter]);

  // 4. Search debounce handler (300ms)
  const handleSearchChange = (value: string) => {
    setSearchInput(value);
    if (searchDebounceRef.current) clearTimeout(searchDebounceRef.current);
    searchDebounceRef.current = setTimeout(() => {
      // deferredSearch triggers via useDeferredValue — nothing extra
    }, 300);
  };

  // 5. Derived
  const filtered =
    dateRange === "all"
      ? messages
      : messages.filter((m) => isInRange(m.timestamp, dateRange));

  const pageCount = Math.ceil(total / PAGE_SIZE);

  const channels = Array.from(
    new Set(messages.map((m) => m.channel.toLowerCase())),
  ).sort();

  return (
    <PageShell
      title="Messages"
      subtitle="Channel message history — newest first"
    >
      <div className="space-y-4">
        {error && (
          <ErrorBanner
            message="Failed to load messages"
            detail={error}
            onRetry={() => void fetchMessages(page, deferredSearch, channelFilter)}
          />
        )}

        {/* Controls bar */}
        <div className="flex flex-col sm:flex-row gap-3 flex-wrap">
          {/* Search */}
          <div className="relative flex-1 min-w-0 max-w-sm">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
            />
            <input
              type="search"
              value={searchInput}
              onChange={(e) => handleSearchChange(e.target.value)}
              placeholder="Full-text search…"
              className="w-full pl-9 pr-8 py-2 surface-inset text-label-14 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
            />
            {searchInput && (
              <button
                type="button"
                onClick={() => setSearchInput("")}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
                aria-label="Clear search"
              >
                <X size={13} />
              </button>
            )}
          </div>

          {/* Filter chips row */}
          <div className="flex items-center gap-2 flex-wrap">
            {/* Channel filter pills */}
            <div className="flex items-center gap-1.5 flex-wrap">
              <button
                type="button"
                onClick={() => setChannelFilter("all")}
                className={[
                  "px-3 py-1.5 rounded-full text-xs font-medium border transition-colors",
                  channelFilter === "all"
                    ? "bg-ds-gray-alpha-200 text-ds-gray-1000 border-ds-gray-500"
                    : "text-ds-gray-900 hover:text-ds-gray-1000 border-ds-gray-400 hover:border-ds-gray-500",
                ].join(" ")}
              >
                All channels
              </button>
              {channels.map((ch) => {
                const pillAccent = channelAccentColor(ch);
                const isActive = channelFilter === ch;
                return (
                  <button
                    key={ch}
                    type="button"
                    onClick={() => setChannelFilter(ch)}
                    className={[
                      "flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium border transition-colors capitalize",
                      isActive
                        ? "text-ds-gray-1000"
                        : "text-ds-gray-900 hover:text-ds-gray-1000 border-ds-gray-400 hover:border-ds-gray-500",
                    ].join(" ")}
                    style={
                      isActive
                        ? {
                            borderColor: pillAccent,
                            backgroundColor: `${pillAccent}15`,
                            borderLeftWidth: "3px",
                          }
                        : undefined
                    }
                  >
                    {channelIcon(ch)}
                    {ch}
                  </button>
                );
              })}
            </div>

            {/* Date range chips */}
            <div className="flex items-center gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400">
              {(["today", "7d", "all"] as DateRange[]).map((r) => (
                <button
                  key={r}
                  type="button"
                  onClick={() => setDateRange(r)}
                  className={[
                    "flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-medium transition-colors",
                    dateRange === r
                      ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                      : "text-ds-gray-900 hover:text-ds-gray-1000",
                  ].join(" ")}
                >
                  <Clock size={10} />
                  {r === "today" ? "Today" : r === "7d" ? "7 days" : "All time"}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Result summary */}
        {!loading && (
          <div className="flex items-center justify-between">
            <SectionHeader
              label="Messages"
              count={filtered.length}
            />
            {deferredSearch && (
              <span className="text-xs text-ds-gray-900">
                Searching: &ldquo;{deferredSearch}&rdquo;
              </span>
            )}
          </div>
        )}

        {/* Messages list */}
        <div key={`${channelFilter}-${dateRange}`} className="animate-crossfade-in surface-card overflow-hidden">
          {loading ? (
            <ul className="divide-y divide-ds-gray-400">
              {Array.from({ length: 8 }).map((_, i) => (
                <li
                  key={i}
                  className="flex items-center gap-3 px-4 py-3"
                >
                  <div className="w-3 h-3 rounded-full animate-pulse bg-ds-gray-400" />
                  <div className="w-10 h-3 animate-pulse rounded bg-ds-gray-400" />
                  <div className="flex-1 h-3 animate-pulse rounded bg-ds-gray-400" style={{ opacity: 1 - i * 0.08 }} />
                  <div className="w-16 h-3 animate-pulse rounded bg-ds-gray-400" />
                </li>
              ))}
            </ul>
          ) : filtered.length === 0 ? (
            <div className="py-4">
              <EmptyState
                title="No messages found"
                description={
                  deferredSearch || channelFilter !== "all" || dateRange !== "all"
                    ? "Try adjusting your search or filters."
                    : "Messages will appear here as Nova processes activity."
                }
                icon={<MessageSquare size={24} aria-hidden="true" />}
              />
            </div>
          ) : (
            <>
              <ul>
                {groupMessagesByHour(filtered).map((group) => (
                  <li key={group.label}>
                    {/* Time group divider */}
                    <div className="flex items-center gap-3 px-4 py-2 bg-ds-gray-100/60">
                      <Clock size={11} className="text-ds-gray-900 shrink-0" />
                      <span
                        className="text-[11px] font-mono font-medium text-ds-gray-900 tracking-wide"
                        suppressHydrationWarning
                      >
                        {group.label}
                      </span>
                      <div className="flex-1 h-px bg-ds-gray-400" />
                    </div>
                    <ul className="divide-y divide-ds-gray-400">
                      {group.messages.map((msg) => (
                        <MessageRow
                          key={msg.id}
                          msg={msg}
                          expanded={expandedId === msg.id}
                          onToggle={() =>
                            setExpandedId((prev) =>
                              prev === msg.id ? null : msg.id,
                            )
                          }
                        />
                      ))}
                    </ul>
                  </li>
                ))}
              </ul>
              <PaginationControls
                page={page}
                pageCount={pageCount}
                disabled={loading}
                onPrev={() => {
                  setPage((p) => Math.max(0, p - 1));
                  setExpandedId(null);
                }}
                onNext={() => {
                  setPage((p) => p + 1);
                  setExpandedId(null);
                }}
              />
            </>
          )}
        </div>
      </div>
    </PageShell>
  );
}
