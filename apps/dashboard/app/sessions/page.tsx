"use client";

import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import Link from "next/link";
import {
  ChevronRight,
  Clock,
  Layers,
  MessageSquare,
  RefreshCw,
  Search,
  Terminal,
  X,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import QuerySkeleton from "@/components/layout/QuerySkeleton";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { SessionTimelineItem } from "@/types/api";
import { useTRPC } from "@/lib/trpc/react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TriggerFilter = "all" | "manual" | "watcher" | "briefing";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function relativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diff = now - then;
  if (diff < 0) return "just now";
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return `${weeks}w ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months}mo ago`;
  return ">1y ago";
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

const TRIGGER_BADGE: Record<string, { bg: string; text: string }> = {
  manual: { bg: "bg-ds-gray-alpha-200", text: "text-ds-gray-1000" },
  watcher: { bg: "bg-amber-700/15", text: "text-amber-700" },
  briefing: { bg: "bg-blue-700/15", text: "text-blue-700" },
};

// ---------------------------------------------------------------------------
// SessionRow
// ---------------------------------------------------------------------------

function SessionRow({ session }: { session: SessionTimelineItem }) {
  const dot = STATUS_DOT[session.status] ?? "bg-ds-gray-600";
  const label = STATUS_LABEL[session.status] ?? session.status;
  const triggerBadge = session.trigger_type
    ? TRIGGER_BADGE[session.trigger_type]
    : null;

  return (
    <Link
      href={`/sessions/${session.id}`}
      className="flex items-center gap-3 px-4 py-3 border-b border-ds-gray-400 hover:bg-ds-gray-100/40 transition-colors group"
    >
      {/* Status dot */}
      <span className={`inline-block size-2 rounded-full shrink-0 ${dot}`} />

      {/* Project + command */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-copy-14 font-medium text-ds-gray-1000 truncate">
            {session.project}
          </span>
          {/* Status */}
          <span className="text-copy-13 text-ds-gray-900">{label}</span>
          {/* Trigger type badge */}
          {triggerBadge && session.trigger_type && (
            <span
              className={`inline-flex items-center px-2 py-0.5 rounded-full text-label-12 font-medium capitalize ${triggerBadge.bg} ${triggerBadge.text}`}
            >
              {session.trigger_type}
            </span>
          )}
        </div>
        <p className="text-copy-13 text-ds-gray-900 font-mono truncate mt-0.5">
          {session.command}
        </p>
      </div>

      {/* Stats */}
      <div className="hidden sm:flex items-center gap-4 shrink-0 text-copy-13 text-ds-gray-900">
        <span className="flex items-center gap-1 font-mono">
          <MessageSquare size={11} />
          {session.message_count}
        </span>
        <span className="flex items-center gap-1 font-mono">
          <Terminal size={11} />
          {session.tool_count}
        </span>
        <span className="flex items-center gap-1 font-mono">
          <Clock size={11} />
          {session.duration_display}
        </span>
      </div>

      {/* Relative time + chevron */}
      <div className="flex items-center gap-2 shrink-0">
        <span
          className="text-copy-13 text-ds-gray-700 font-mono"
          suppressHydrationWarning
        >
          {relativeTime(session.started_at)}
        </span>
        <ChevronRight
          size={14}
          className="text-ds-gray-700 group-hover:text-ds-gray-1000 transition-colors"
        />
      </div>
    </Link>
  );
}

// ---------------------------------------------------------------------------
// SessionsPage
// ---------------------------------------------------------------------------

function SessionsPage() {
  const trpc = useTRPC();
  // 1. Context/Routing
  const searchParams = useSearchParams();
  const router = useRouter();

  // Initialize filters from URL search params
  const initialProject = searchParams.get("project") ?? "all";
  const initialTrigger = (searchParams.get("trigger_type") ?? "all") as TriggerFilter;
  const initialDateFrom = searchParams.get("date_from") ?? "";
  const initialDateTo = searchParams.get("date_to") ?? "";
  const initialSearch = searchParams.get("q") ?? "";
  const initialPage = Number(searchParams.get("page")) || 1;
  const initialCommand = searchParams.get("command") ?? "";

  // 2. Local State
  const [page, setPage] = useState(initialPage);
  const [projectFilter, setProjectFilter] = useState(initialProject);
  const [triggerFilter, setTriggerFilter] = useState<TriggerFilter>(initialTrigger);
  const [dateFrom, setDateFrom] = useState(initialDateFrom);
  const [dateTo, setDateTo] = useState(initialDateTo);
  const [searchInput, setSearchInput] = useState(initialSearch);
  const [debouncedSearch, setDebouncedSearch] = useState(initialSearch);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [commandFilter, setCommandFilter] = useState(initialCommand);

  // Build query input for tRPC
  const queryInput: Record<string, unknown> = {
    page,
    limit: 25,
  };
  if (projectFilter !== "all") queryInput.project = projectFilter;
  if (triggerFilter !== "all") queryInput.trigger_type = triggerFilter;
  if (dateFrom) queryInput.date_from = dateFrom;
  if (dateTo) queryInput.date_to = dateTo;

  // 3. Query -- sessions list
  const { data, isLoading, error, refetch } = useQuery(
    trpc.session.list.queryOptions(queryInput as { page?: number; limit?: number; project?: string; trigger_type?: string; date_from?: string; date_to?: string }),
  );

  const sessions = (data?.sessions ?? []) as SessionTimelineItem[];
  const total = data?.total ?? 0;

  // 4. Distinct projects query (unfiltered, for dropdown)
  const { data: allSessionsData } = useQuery(
    trpc.session.list.queryOptions({ page: 1, limit: 100 }),
  );
  const distinctProjects = Array.from(
    new Set([
      ...((allSessionsData?.sessions ?? []) as SessionTimelineItem[]).map((s) => s.project),
      ...sessions.map((s) => s.project),
    ]),
  ).sort();

  // 5. Debounced search
  const handleSearchChange = (value: string) => {
    setSearchInput(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setDebouncedSearch(value);
    }, 300);
  };

  // 6. Update URL search params
  useEffect(() => {
    const params = new URLSearchParams();
    if (projectFilter !== "all") params.set("project", projectFilter);
    if (triggerFilter !== "all") params.set("trigger_type", triggerFilter);
    if (dateFrom) params.set("date_from", dateFrom);
    if (dateTo) params.set("date_to", dateTo);
    if (debouncedSearch) params.set("q", debouncedSearch);
    if (commandFilter) params.set("command", commandFilter);
    if (page > 1) params.set("page", String(page));

    const paramStr = params.toString();
    const newUrl = paramStr ? `?${paramStr}` : "/sessions";
    router.replace(newUrl, { scroll: false });
  }, [projectFilter, triggerFilter, dateFrom, dateTo, debouncedSearch, commandFilter, page, router]);

  // 7. Derived — client-side text search + command filtering
  const filtered = sessions.filter((s) => {
    // Command filter
    if (commandFilter && s.command !== commandFilter) return false;
    // Text search
    if (debouncedSearch) {
      const q = debouncedSearch.toLowerCase();
      return (
        s.project.toLowerCase().includes(q) ||
        s.command.toLowerCase().includes(q) ||
        s.id.toLowerCase().includes(q) ||
        (s.trigger_type?.toLowerCase().includes(q) ?? false)
      );
    }
    return true;
  });

  // 8. Handlers
  const handleClearFilters = () => {
    setProjectFilter("all");
    setTriggerFilter("all");
    setDateFrom("");
    setDateTo("");
    setSearchInput("");
    setDebouncedSearch("");
    setCommandFilter("");
    setPage(1);
  };

  const hasFilters =
    projectFilter !== "all" ||
    triggerFilter !== "all" ||
    dateFrom !== "" ||
    dateTo !== "" ||
    debouncedSearch !== "" ||
    commandFilter !== "";

  const totalPages = Math.ceil(total / 25);

  // 9. Header action
  const headerAction = (
    <button
      type="button"
      onClick={() => void refetch()}
      disabled={isLoading}
      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
    >
      <RefreshCw size={12} className={isLoading ? "animate-spin" : ""} />
      Refresh
    </button>
  );

  // 10. Render
  return (
    <PageShell
      title="Sessions"
      subtitle={
        isLoading
          ? "Loading..."
          : `${total} session${total !== 1 ? "s" : ""} total`
      }
      action={headerAction}
    >
      <div className="flex flex-col gap-3">
        {/* Filter bar */}
        <div className="flex flex-col gap-3 sm:flex-row sm:flex-wrap sm:items-end">
          {/* Search */}
          <div className="relative flex-1 min-w-[200px] max-w-sm">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
            />
            <input
              type="search"
              value={searchInput}
              onChange={(e) => handleSearchChange(e.target.value)}
              placeholder="Search by ID, project, command..."
              className="w-full pl-9 pr-8 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
            />
            {searchInput && (
              <button
                type="button"
                onClick={() => {
                  setSearchInput("");
                  setDebouncedSearch("");
                }}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
                aria-label="Clear search"
              >
                <X size={13} />
              </button>
            )}
          </div>

          {/* Project dropdown */}
          <select
            value={projectFilter}
            onChange={(e) => { setProjectFilter(e.target.value); setPage(1); }}
            className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
          >
            <option value="all">All projects</option>
            {distinctProjects.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>

          {/* Trigger type selector */}
          <div className="flex items-center gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400">
            {(["all", "manual", "watcher", "briefing"] as TriggerFilter[]).map(
              (t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => { setTriggerFilter(t); setPage(1); }}
                  className={[
                    "px-3 py-1 rounded-md text-label-13 transition-colors capitalize",
                    triggerFilter === t
                      ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                      : "text-ds-gray-900 hover:text-ds-gray-1000",
                  ].join(" ")}
                >
                  {t}
                </button>
              ),
            )}
          </div>

          {/* Date range */}
          <div className="flex items-center gap-2">
            <input
              type="date"
              value={dateFrom}
              onChange={(e) => { setDateFrom(e.target.value); setPage(1); }}
              className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
              aria-label="Date from"
            />
            <span className="text-copy-13 text-ds-gray-700">to</span>
            <input
              type="date"
              value={dateTo}
              onChange={(e) => { setDateTo(e.target.value); setPage(1); }}
              className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
              aria-label="Date to"
            />
          </div>
        </div>

        {/* Command filter chip */}
        {commandFilter && (
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-ds-gray-alpha-200 text-label-13 text-ds-gray-1000">
              <Terminal size={12} />
              command: {commandFilter}
              <button
                type="button"
                onClick={() => setCommandFilter("")}
                className="ml-0.5 text-ds-gray-700 hover:text-ds-gray-1000 transition-colors"
                aria-label="Clear command filter"
              >
                <X size={12} />
              </button>
            </span>
          </div>
        )}

        {/* Results count */}
        {!isLoading && (
          <div className="flex items-center justify-between">
            <p className="text-copy-13 text-ds-gray-900">
              {filtered.length} session{filtered.length !== 1 ? "s" : ""}
              {debouncedSearch ? ` matching "${debouncedSearch}"` : ""}
              {totalPages > 1 ? ` (page ${page} of ${totalPages})` : ""}
            </p>
            {hasFilters && (
              <button
                type="button"
                onClick={handleClearFilters}
                className="text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors underline"
              >
                Clear filters
              </button>
            )}
          </div>
        )}

        {error && (
          <ErrorBanner
            message="Failed to load sessions"
            detail={error.message}
            onRetry={() => void refetch()}
          />
        )}

        {/* Loading skeleton */}
        {isLoading && sessions.length === 0 ? (
          <QuerySkeleton rows={8} height="h-16" />
        ) : filtered.length === 0 ? (
          /* Empty state */
          <div className="flex flex-col items-center gap-3 py-16">
            {hasFilters ? (
              <>
                <Search size={28} className="text-ds-gray-600" />
                <p className="text-copy-13 text-ds-gray-900 text-center">
                  No sessions match your filters.
                </p>
                <button
                  type="button"
                  onClick={handleClearFilters}
                  className="px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors"
                >
                  Clear filters
                </button>
              </>
            ) : (
              <>
                <Layers size={28} className="text-ds-gray-600" />
                <div className="text-center flex flex-col gap-1">
                  <p className="text-copy-13 text-ds-gray-900">
                    No sessions recorded yet.
                  </p>
                  <p className="text-copy-13 text-ds-gray-700">
                    Sessions will appear automatically when agent commands run.
                  </p>
                </div>
              </>
            )}
          </div>
        ) : (
          /* Session list */
          <div className="rounded-xl border border-ds-gray-400 overflow-hidden">
            {filtered.map((session, idx) => (
              <div
                key={session.id}
                className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
              >
                <SessionRow session={session} />
              </div>
            ))}
          </div>
        )}

        {/* Pagination */}
        {totalPages > 1 && !isLoading && (
          <div className="flex items-center justify-center gap-2 py-2">
            <button
              type="button"
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              disabled={page <= 1}
              className="px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
            >
              Previous
            </button>
            <span className="text-copy-13 text-ds-gray-900 font-mono">
              {page} / {totalPages}
            </span>
            <button
              type="button"
              onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
              disabled={page >= totalPages}
              className="px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
            >
              Next
            </button>
          </div>
        )}
      </div>
    </PageShell>
  );
}

// ---------------------------------------------------------------------------
// Export with Suspense wrapper (useSearchParams requires it)
// ---------------------------------------------------------------------------

export default function SessionsPageWrapper() {
  return (
    <Suspense>
      <SessionsPage />
    </Suspense>
  );
}
