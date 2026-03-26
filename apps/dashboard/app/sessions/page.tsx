"use client";

import {
  useEffect,
  useState,
  useCallback,
  useRef,
  useDeferredValue,
} from "react";
import { useSearchParams } from "next/navigation";
import {
  RefreshCw,
  Layers,
  Search,
  X,
  ChevronRight,
  Clock,
  GitBranch,
  MessageSquare,
  Terminal,
  Monitor,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import CCSessionPanel from "@/components/CCSessionPanel";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";
import type {
  SessionsGetResponse,
  NexusSessionRaw,
  CcSessionSummary,
  CcSessionsGetResponse,
} from "@/types/api";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SessionItem {
  id: string;
  slug: string;
  project: string;
  agent_name: string;
  status: "active" | "idle" | "completed";
  duration_display: string;
  started_at: string;
  branch?: string;
  spec?: string;
  progress?: number;
  phase_label?: string;
}

type StatusFilter = "all" | "active" | "idle" | "completed";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function mapSession(s: NexusSessionRaw): SessionItem {
  const mapStatus = (raw: string): SessionItem["status"] => {
    if (raw === "active") return "active";
    if (raw === "idle") return "idle";
    return "completed";
  };
  return {
    id: s.id,
    slug: s.id.slice(0, 8),
    project: s.project ?? s.agent_name,
    agent_name: s.agent_name,
    status: mapStatus(s.status),
    duration_display: s.duration_display,
    started_at: s.started_at ?? new Date().toISOString(),
    branch: s.branch ?? undefined,
    spec: s.spec ?? undefined,
    progress: s.progress?.progress_pct,
    phase_label: s.progress?.phase_label,
  };
}

function elapsed(startedAt: string): string {
  const diffMs = Date.now() - new Date(startedAt).getTime();
  const min = Math.floor(diffMs / 60000);
  if (min < 60) return `${min}m`;
  const hr = Math.floor(min / 60);
  return `${hr}h ${min % 60}m`;
}

// ---------------------------------------------------------------------------
// Status dot
// ---------------------------------------------------------------------------

const STATUS_DOT: Record<SessionItem["status"], string> = {
  active: "bg-green-700 animate-pulse",
  idle: "bg-amber-700",
  completed: "bg-ds-gray-600",
};

const STATUS_LABEL: Record<SessionItem["status"], string> = {
  active: "Active",
  idle: "Idle",
  completed: "Completed",
};

const STATUS_TEXT: Record<SessionItem["status"], string> = {
  active: "text-green-700",
  idle: "text-amber-700",
  completed: "text-ds-gray-900",
};

// ---------------------------------------------------------------------------
// Enhanced Session Card
// ---------------------------------------------------------------------------

interface SessionCardProps {
  session: SessionItem;
  onSelect: (s: SessionItem) => void;
  selected: boolean;
}

function EnhancedSessionCard({
  session,
  onSelect,
  selected,
}: SessionCardProps) {
  const dot = STATUS_DOT[session.status];
  const statusText = STATUS_TEXT[session.status];
  const label = STATUS_LABEL[session.status];
  const progress = session.progress ?? 0;

  return (
    <button
      type="button"
      onClick={() => onSelect(session)}
      className={[
        "w-full text-left p-4 space-y-3 surface-card",
        selected
          ? "!border-ds-gray-1000/60 !bg-ds-gray-alpha-100"
          : "",
      ].join(" ")}
    >
      {/* Header row */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 flex-wrap min-w-0">
          <span className={`inline-block w-2 h-2 rounded-full shrink-0 ${dot}`} />
          <span className="text-sm font-medium text-ds-gray-1000 truncate">
            {session.project}
          </span>
          <span className={`text-xs font-medium ${statusText}`}>{label}</span>
        </div>
        <div className="flex items-center gap-1 shrink-0 text-xs text-ds-gray-900 font-mono">
          <Clock size={11} />
          <span suppressHydrationWarning>{elapsed(session.started_at)}</span>
        </div>
      </div>

      {/* Slug + branch */}
      <div className="flex items-center gap-3 text-xs text-ds-gray-900 font-mono">
        <span className="text-ds-gray-1000/80">{session.slug}…</span>
        {session.branch && (
          <span className="flex items-center gap-1">
            <GitBranch size={11} />
            <span className="truncate max-w-[160px]">{session.branch}</span>
          </span>
        )}
        {session.spec && (
          <span className="px-1.5 py-0.5 rounded bg-ds-gray-alpha-200 text-ds-gray-1000 text-[10px] truncate max-w-[120px]">
            {session.spec}
          </span>
        )}
      </div>

      {/* Phase label */}
      {session.phase_label && (
        <p className="text-xs text-ds-gray-900 truncate pl-4">
          {session.phase_label}
        </p>
      )}

      {/* Progress bar — active sessions only */}
      {session.status === "active" && (
        <div className="space-y-1">
          <div className="h-1 rounded-full bg-ds-bg-100 overflow-hidden">
            <div
              className="h-full rounded-full bg-ds-gray-700 transition-all duration-500"
              style={{ width: `${Math.min(100, Math.max(0, progress))}%` }}
            />
          </div>
          <p className="text-[10px] text-ds-gray-900 font-mono text-right">
            {progress}%
          </p>
        </div>
      )}

      {/* Footer */}
      <div className="flex items-center justify-between pt-1 border-t border-ds-gray-400">
        <span className="text-xs text-ds-gray-900 font-mono">{session.agent_name}</span>
        <ChevronRight size={14} className="text-ds-gray-900" />
      </div>
    </button>
  );
}

// ---------------------------------------------------------------------------
// Session Detail Drawer
// ---------------------------------------------------------------------------

function SessionDetailDrawer({
  session,
  onClose,
}: {
  session: SessionItem | null;
  onClose: () => void;
}) {
  if (!session) return null;

  const dot = STATUS_DOT[session.status];
  const statusText = STATUS_TEXT[session.status];

  return (
    <div className="fixed inset-y-0 right-0 z-40 flex">
      {/* Backdrop */}
      <button
        type="button"
        className="fixed inset-0 bg-black/40"
        onClick={onClose}
        aria-label="Close drawer"
      />

      {/* Panel */}
      <aside className="relative ml-auto w-80 md:w-96 bg-ds-bg-100 border-l border-ds-gray-400 flex flex-col shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between gap-2 px-5 py-4 border-b border-ds-gray-400">
          <div className="flex items-center gap-2 min-w-0">
            <span className={`inline-block w-2 h-2 rounded-full shrink-0 ${dot}`} />
            <span className="text-sm font-semibold text-ds-gray-1000 truncate">
              {session.project}
            </span>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1.5 rounded-lg text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-100 transition-colors"
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">
          {/* Status */}
          <section className="space-y-2">
            <p className="text-xs text-ds-gray-900 uppercase tracking-widest font-semibold">
              Status
            </p>
            <span className={`text-sm font-medium ${statusText}`}>
              {STATUS_LABEL[session.status]}
            </span>
          </section>

          {/* Identifiers */}
          <section className="space-y-3">
            <p className="text-xs text-ds-gray-900 uppercase tracking-widest font-semibold">
              Identifiers
            </p>
            <div className="space-y-2">
              <div className="flex items-start justify-between gap-2">
                <span className="text-xs text-ds-gray-900">Session ID</span>
                <span className="text-xs font-mono text-ds-gray-1000 break-all text-right max-w-[200px]">
                  {session.id}
                </span>
              </div>
              <div className="flex items-start justify-between gap-2">
                <span className="text-xs text-ds-gray-900">Agent</span>
                <span className="text-xs font-mono text-ds-gray-1000">
                  {session.agent_name}
                </span>
              </div>
              {session.branch && (
                <div className="flex items-start justify-between gap-2">
                  <span className="text-xs text-ds-gray-900">Branch</span>
                  <span className="flex items-center gap-1 text-xs font-mono text-ds-gray-1000">
                    <GitBranch size={10} />
                    {session.branch}
                  </span>
                </div>
              )}
              {session.spec && (
                <div className="flex items-start justify-between gap-2">
                  <span className="text-xs text-ds-gray-900">Spec</span>
                  <span className="text-xs font-mono text-ds-gray-1000">
                    {session.spec}
                  </span>
                </div>
              )}
            </div>
          </section>

          {/* Timing */}
          <section className="space-y-3">
            <p className="text-xs text-ds-gray-900 uppercase tracking-widest font-semibold">
              Timing
            </p>
            <div className="space-y-2">
              <div className="flex items-start justify-between gap-2">
                <span className="text-xs text-ds-gray-900">Started</span>
                <span
                  className="text-xs font-mono text-ds-gray-1000"
                  suppressHydrationWarning
                >
                  {new Date(session.started_at).toLocaleString()}
                </span>
              </div>
              <div className="flex items-start justify-between gap-2">
                <span className="text-xs text-ds-gray-900">Duration</span>
                <span className="text-xs font-mono text-ds-gray-1000">
                  {session.duration_display}
                </span>
              </div>
              <div className="flex items-start justify-between gap-2">
                <span className="text-xs text-ds-gray-900">Elapsed</span>
                <span
                  className="text-xs font-mono text-ds-gray-1000"
                  suppressHydrationWarning
                >
                  {elapsed(session.started_at)}
                </span>
              </div>
            </div>
          </section>

          {/* Progress */}
          {session.status === "active" && (
            <section className="space-y-3">
              <p className="text-xs text-ds-gray-900 uppercase tracking-widest font-semibold">
                Progress
              </p>
              {session.phase_label && (
                <p className="text-xs text-ds-gray-1000">{session.phase_label}</p>
              )}
              <div className="space-y-1">
                <div className="h-2 rounded-full bg-ds-bg-100 overflow-hidden">
                  <div
                    className="h-full rounded-full bg-ds-gray-700 transition-all duration-500"
                    style={{
                      width: `${Math.min(100, Math.max(0, session.progress ?? 0))}%`,
                    }}
                  />
                </div>
                <p className="text-xs font-mono text-ds-gray-900 text-right">
                  {session.progress ?? 0}%
                </p>
              </div>
            </section>
          )}
        </div>
      </aside>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ProjectSessionsTable — CC subprocess sessions (CcSessionManager)
// ---------------------------------------------------------------------------

const CC_STATE_DOT: Record<string, string> = {
  running: "bg-green-700 animate-pulse",
  completed: "bg-ds-gray-600",
  stopped: "bg-amber-700",
};

function ProjectSessionsTable() {
  const [sessions, setSessions] = useState<CcSessionSummary[]>([]);
  const [configured, setConfigured] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchCcSessions = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/cc-sessions");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as CcSessionsGetResponse;
      setSessions(data.sessions ?? []);
      setConfigured(data.configured ?? false);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load CC sessions",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchCcSessions();
  }, [fetchCcSessions]);

  if (!configured && !loading) return null;

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-ds-gray-1000 flex items-center gap-2">
          <Terminal size={14} className="text-ds-gray-1000" />
          CC Sessions
        </h3>
        <button
          type="button"
          onClick={() => void fetchCcSessions()}
          disabled={loading}
          className="text-xs text-ds-gray-900 hover:text-ds-gray-1000 transition-colors disabled:opacity-50"
          aria-label="Refresh CC sessions"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      {error && (
        <p className="text-xs text-destructive">
          Failed to load CC sessions: {error}
        </p>
      )}

      {loading ? (
        <div className="space-y-1.5">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="h-10 animate-pulse rounded-md bg-ds-gray-100 border border-ds-gray-400"
            />
          ))}
        </div>
      ) : sessions.length === 0 ? (
        <p className="text-xs text-ds-gray-900 py-2">No CC sessions.</p>
      ) : (
        <div className="rounded-xl border border-ds-gray-400 overflow-hidden">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-ds-gray-400 bg-ds-gray-100/60">
                <th className="px-3 py-2 text-left font-medium text-ds-gray-900">
                  ID
                </th>
                <th className="px-3 py-2 text-left font-medium text-ds-gray-900">
                  Project
                </th>
                <th className="px-3 py-2 text-left font-medium text-ds-gray-900">
                  State
                </th>
                <th className="px-3 py-2 text-left font-medium text-ds-gray-900">
                  Duration
                </th>
                <th className="px-3 py-2 text-left font-medium text-ds-gray-900">
                  Restarts
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-ds-gray-400">
              {sessions.map((s) => (
                <tr
                  key={s.id}
                  className="hover:bg-ds-gray-100/40 transition-colors"
                >
                  <td className="px-3 py-2 font-mono text-ds-gray-1000">
                    {s.id.slice(0, 10)}
                  </td>
                  <td className="px-3 py-2 text-ds-gray-1000 font-medium">
                    {s.project}
                  </td>
                  <td className="px-3 py-2">
                    <span className="flex items-center gap-1.5">
                      <span
                        className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${CC_STATE_DOT[s.state] ?? "bg-ds-gray-600"}`}
                      />
                      <span className="text-ds-gray-900 capitalize">
                        {s.state}
                      </span>
                    </span>
                  </td>
                  <td className="px-3 py-2 text-ds-gray-900 font-mono">
                    {s.duration_display}
                  </td>
                  <td className="px-3 py-2 text-ds-gray-900 font-mono">
                    {s.restart_attempts}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// Sessions Page
// ---------------------------------------------------------------------------

export default function SessionsPage() {
  // 1. Context/Routing
  const searchParams = useSearchParams();
  const showCcPanel = searchParams.get("panel") === "cc";
  const [ccPanelOpen, setCcPanelOpen] = useState(showCcPanel);

  // 2. Local State
  const [sessions, setSessions] = useState<SessionItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [projectFilter, setProjectFilter] = useState("all");
  const [searchInput, setSearchInput] = useState("");
  const deferredSearch = useDeferredValue(searchInput);
  const searchDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 2. WebSocket — live session updates
  useDaemonEvents(
    useCallback((ev) => {
      const payload = ev.payload as
        | Partial<NexusSessionRaw>
        | null
        | undefined;
      if (!payload?.id) return;
      setSessions((prev) => {
        const idx = prev.findIndex((s) => s.id === payload.id);
        if (idx === -1) {
          const mapped = mapSession(payload as NexusSessionRaw);
          return [mapped, ...prev];
        }
        const updated = { ...prev[idx]! };
        if (payload.status) {
          updated.status = (() => {
            if (payload.status === "active") return "active";
            if (payload.status === "idle") return "idle";
            return "completed";
          })();
        }
        if (payload.progress) {
          updated.progress = payload.progress.progress_pct;
          updated.phase_label = payload.progress.phase_label;
        }
        const copy = [...prev];
        copy[idx] = updated;
        return copy;
      });
    }, []),
    "session",
  );

  // 3. Fetch sessions
  const fetchSessions = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/sessions");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as SessionsGetResponse;
      setSessions((data.sessions ?? []).map(mapSession));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load sessions");
    } finally {
      setLoading(false);
    }
  }, []);

  // 4. Effects
  useEffect(() => {
    void fetchSessions();
  }, [fetchSessions]);

  // 5. Search debounce (300ms)
  const handleSearchChange = (value: string) => {
    setSearchInput(value);
    if (searchDebounceRef.current) clearTimeout(searchDebounceRef.current);
    searchDebounceRef.current = setTimeout(() => {
      // Deferred value takes care of rendering — nothing extra needed
    }, 300);
  };

  // 6. Derived — filtered lists
  const projects = Array.from(new Set(sessions.map((s) => s.project))).sort();

  const filtered = sessions.filter((s) => {
    if (statusFilter !== "all" && s.status !== statusFilter) return false;
    if (projectFilter !== "all" && s.project !== projectFilter) return false;
    if (deferredSearch) {
      const q = deferredSearch.toLowerCase();
      if (
        !s.id.toLowerCase().includes(q) &&
        !s.project.toLowerCase().includes(q) &&
        !s.agent_name.toLowerCase().includes(q) &&
        !(s.branch?.toLowerCase().includes(q) ?? false) &&
        !(s.spec?.toLowerCase().includes(q) ?? false)
      ) {
        return false;
      }
    }
    return true;
  });

  const active = filtered.filter((s) => s.status === "active");
  const idle = filtered.filter((s) => s.status === "idle");
  const completed = filtered.filter((s) => s.status === "completed");

  const selectedSession = sessions.find((s) => s.id === selectedId) ?? null;

  // 7. Header action
  const headerAction = (
    <button
      type="button"
      onClick={() => void fetchSessions()}
      disabled={loading}
      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
    >
      <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
      Refresh
    </button>
  );

  return (
    <>
      <PageShell
        title="Sessions"
        subtitle="Active, idle, and completed agent sessions"
        action={headerAction}
      >
        <div className="space-y-5">
          {/* CC Session panel toggle + collapsible panel */}
          <div className="space-y-3">
            <button
              type="button"
              onClick={() => setCcPanelOpen((prev) => !prev)}
              className={[
                "flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium border transition-colors",
                ccPanelOpen
                  ? "bg-ds-gray-alpha-200 text-ds-gray-1000 border-ds-gray-1000/30"
                  : "text-ds-gray-900 border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500",
              ].join(" ")}
            >
              <Monitor size={14} />
              CC Session
            </button>
            {ccPanelOpen && (
              <div className="surface-card p-5">
                <CCSessionPanel />
              </div>
            )}
          </div>

          {error && (
            <ErrorBanner
              message="Failed to load sessions"
              detail={error}
              onRetry={() => void fetchSessions()}
            />
          )}

          {/* Filter bar */}
          <div className="flex flex-col sm:flex-row gap-3 section-stagger-1">
            {/* Search */}
            <div className="relative flex-1 max-w-sm">
              <Search
                size={14}
                className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
              />
              <input
                type="search"
                value={searchInput}
                onChange={(e) => handleSearchChange(e.target.value)}
                placeholder="Search by ID, project, agent…"
                className="w-full pl-9 pr-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-sm text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
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

            {/* Project dropdown */}
            <select
              value={projectFilter}
              onChange={(e) => setProjectFilter(e.target.value)}
              className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-sm text-ds-gray-1000 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
            >
              <option value="all">All projects</option>
              {projects.map((p) => (
                <option key={p} value={p}>
                  {p}
                </option>
              ))}
            </select>

            {/* Status tabs */}
            <div className="flex items-center gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400">
              {(["all", "active", "idle", "completed"] as StatusFilter[]).map(
                (s) => (
                  <button
                    key={s}
                    type="button"
                    onClick={() => setStatusFilter(s)}
                    className={[
                      "px-3 py-1 rounded-md text-xs font-medium transition-colors capitalize",
                      statusFilter === s
                        ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                        : "text-ds-gray-900 hover:text-ds-gray-1000",
                    ].join(" ")}
                  >
                    {s}
                  </button>
                ),
              )}
            </div>
          </div>

          {/* Results count */}
          {!loading && (
            <p className="text-xs text-ds-gray-900 section-stagger-2">
              {filtered.length} session{filtered.length !== 1 ? "s" : ""}
              {deferredSearch ? ` matching "${deferredSearch}"` : ""}
            </p>
          )}

          {/* Skeleton */}
          {loading ? (
            <div className="space-y-2">
              {Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="h-28 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
                />
              ))}
            </div>
          ) : filtered.length === 0 ? (
            <EmptyState
              title="No sessions found"
              description={
                deferredSearch || statusFilter !== "all" || projectFilter !== "all"
                  ? "Try adjusting your filters."
                  : "Sessions will appear here when the daemon is active."
              }
              icon={<Layers size={24} aria-hidden="true" />}
            />
          ) : (
            <div key={statusFilter} className="animate-crossfade-in space-y-6 section-stagger-3">
              {/* Active */}
              {active.length > 0 && (
                <section className="space-y-2">
                  <SectionHeader
                    label="Active"
                    count={active.length}
                    statusDot="green"
                    statusLabel="Active sessions"
                  />
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    {active.map((s, idx) => (
                      <div
                        key={s.id}
                        className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
                      >
                        <EnhancedSessionCard
                          session={s}
                          selected={selectedId === s.id}
                          onSelect={(sess) =>
                            setSelectedId((prev) =>
                              prev === sess.id ? null : sess.id,
                            )
                          }
                        />
                      </div>
                    ))}
                  </div>
                </section>
              )}

              {/* Idle */}
              {idle.length > 0 && (
                <section className="space-y-2">
                  <SectionHeader
                    label="Idle"
                    count={idle.length}
                    statusDot="amber"
                    statusLabel="Idle sessions"
                  />
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    {idle.map((s, idx) => (
                      <div
                        key={s.id}
                        className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
                      >
                        <EnhancedSessionCard
                          session={s}
                          selected={selectedId === s.id}
                          onSelect={(sess) =>
                            setSelectedId((prev) =>
                              prev === sess.id ? null : sess.id,
                            )
                          }
                        />
                      </div>
                    ))}
                  </div>
                </section>
              )}

              {/* Completed */}
              {completed.length > 0 && (
                <section className="space-y-2">
                  <SectionHeader
                    label="Completed"
                    count={completed.length}
                    statusDot="muted"
                    statusLabel="Completed sessions"
                  />
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    {completed.map((s, idx) => (
                      <div
                        key={s.id}
                        className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
                      >
                        <EnhancedSessionCard
                          session={s}
                          selected={selectedId === s.id}
                          onSelect={(sess) =>
                            setSelectedId((prev) =>
                              prev === sess.id ? null : sess.id,
                            )
                          }
                        />
                      </div>
                    ))}
                  </div>
                </section>
              )}
            </div>
          )}
          {/* CC Sessions table (CcSessionManager) */}
          <ProjectSessionsTable />
        </div>
      </PageShell>

      {/* Session detail drawer */}
      <SessionDetailDrawer
        session={selectedSession}
        onClose={() => setSelectedId(null)}
      />
    </>
  );
}
