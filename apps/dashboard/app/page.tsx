"use client";

import { useEffect, useState, useCallback, useRef } from "react";
import {
  CheckSquare,
  Layers,
  MessageSquare,
  Terminal,
  TrendingUp,
  RefreshCw,
  Cpu,
  MemoryStick,
  Heart,
  Activity,
  ArrowRight,
  Timer,
} from "lucide-react";
import Link from "next/link";
import PageShell from "@/components/layout/PageShell";
import StatCard, { type StatCardVariant } from "@/components/layout/StatCard";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import SessionWidget from "@/components/SessionWidget";
import ActiveSession, {
  type ActiveSessionData,
} from "@/components/ActiveSession";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";
import type {
  ObligationsGetResponse,
  ProjectsGetResponse,
  SessionsGetResponse,
  ServerHealthGetResponse,
  NexusSessionRaw,
} from "@/types/api";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface SummaryData {
  obligations_count: number;
  active_sessions: number;
  idle_sessions: number;
  projects_count: number;
  messages_today: number;
  tools_today: number;
  cost_today_usd: number;
}

interface HealthSummary {
  status: "ok" | "degraded" | "critical" | "down";
  cpu_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  uptime_seconds: number;
}

interface ApiObligation {
  id: string;
  detected_action: string;
  owner?: string;
  /** "open" | "in_progress" | "done" | "dismissed" */
  status?: string;
}

interface ActivityEvent {
  id: string;
  type: string;
  label: string;
  ts: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function mapNexusSession(s: NexusSessionRaw): ActiveSessionData {
  const mapStatus = (raw: string): ActiveSessionData["status"] => {
    if (raw === "active") return "active";
    if (raw === "idle") return "idle";
    return "completed";
  };
  return {
    id: s.id,
    service: s.agent_name,
    status: mapStatus(s.status),
    messages: 0,
    tools_executed: 0,
    started_at: s.started_at ?? new Date().toISOString(),
    user: s.project ?? undefined,
    progress: s.progress?.progress_pct,
    current_task: s.progress?.phase_label,
  };
}

function formatUptime(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const mins = Math.floor(seconds / 60);
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ${mins % 60}m`;
  return `${Math.floor(hrs / 24)}d ${hrs % 24}h`;
}

function healthVariant(status: HealthSummary["status"] | undefined): StatCardVariant {
  if (!status || status === "ok") return "success";
  if (status === "degraded") return "warning";
  return "error";
}

function cpuVariant(pct: number): StatCardVariant {
  if (pct >= 90) return "error";
  if (pct >= 70) return "warning";
  return "success";
}

function memVariant(used: number, total: number): StatCardVariant {
  if (total === 0) return "default";
  const pct = used / total;
  if (pct >= 0.9) return "error";
  if (pct >= 0.8) return "warning";
  return "success";
}

// ---------------------------------------------------------------------------
// Activity Feed
// ---------------------------------------------------------------------------

function ActivityFeed({ events }: { events: ActivityEvent[] }) {
  if (events.length === 0) {
    return (
      <EmptyState
        title="No recent events"
        description="Events will appear here as the daemon processes activity."
        icon={<Activity size={24} aria-hidden="true" />}
      />
    );
  }

  return (
    <ul className="space-y-1">
      {events.map((ev) => (
        <li
          key={ev.id}
          className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-ds-gray-100/50 transition-colors"
        >
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-ds-gray-700/60 shrink-0" />
          <span className="flex-1 min-w-0 text-sm text-ds-gray-1000 truncate">
            {ev.label}
          </span>
          <span
            className="shrink-0 text-xs text-ds-gray-900 font-mono"
            suppressHydrationWarning
          >
            {new Date(ev.ts).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </span>
        </li>
      ))}
    </ul>
  );
}

// ---------------------------------------------------------------------------
// Obligations Sidebar Panel
// ---------------------------------------------------------------------------

function ObligationsSidebar({
  obligations,
  loading,
}: {
  obligations: ApiObligation[];
  loading: boolean;
}) {
  const pending = obligations.filter(
    (o) => !o.status || o.status === "open" || o.status === "in_progress",
  );
  const done = obligations.filter((o) => o.status === "done");

  return (
    <div className="surface-card p-4 space-y-4">
      <div className="flex items-center justify-between">
        <SectionHeader label="Obligations" count={obligations.length} />
        <Link
          href="/obligations"
          className="flex items-center gap-1 text-xs text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
        >
          View all
          <ArrowRight size={12} />
        </Link>
      </div>

      {loading ? (
        <div className="space-y-2">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="h-8 animate-pulse rounded-lg bg-ds-gray-400"
            />
          ))}
        </div>
      ) : obligations.length === 0 ? (
        <p className="text-xs text-ds-gray-900 py-3 text-center">
          No obligations
        </p>
      ) : (
        <div className="space-y-2">
          <div className="flex items-center justify-between text-xs text-ds-gray-900">
            <span>Pending</span>
            <span className="font-mono text-amber-400">{pending.length}</span>
          </div>
          <div className="flex items-center justify-between text-xs text-ds-gray-900">
            <span>Completed</span>
            <span className="font-mono text-emerald-400">{done.length}</span>
          </div>
          {/* Owner breakdown */}
          {(() => {
            const byOwner: Record<string, number> = {};
            for (const o of obligations) {
              const owner = o.owner ?? "unassigned";
              byOwner[owner] = (byOwner[owner] ?? 0) + 1;
            }
            return Object.entries(byOwner)
              .sort((a, b) => b[1] - a[1])
              .slice(0, 4)
              .map(([owner, count]) => (
                <div
                  key={owner}
                  className="flex items-center justify-between text-xs"
                >
                  <span className="text-ds-gray-900 truncate max-w-[120px]">
                    @{owner}
                  </span>
                  <span className="font-mono text-ds-gray-1000">{count}</span>
                </div>
              ));
          })()}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Dashboard Page
// ---------------------------------------------------------------------------

export default function DashboardPage() {
  // 1. State
  const [summary, setSummary] = useState<SummaryData | null>(null);
  const [sessions, setSessions] = useState<ActiveSessionData[]>([]);
  const [health, setHealth] = useState<HealthSummary | null>(null);
  const [obligations, setObligations] = useState<ApiObligation[]>([]);
  const [activityFeed, setActivityFeed] = useState<ActivityEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const activityIdRef = useRef(0);

  // 2. WebSocket subscription — prepend events to activity feed (capped at 10)
  useDaemonEvents(
    (ev) => {
      const label =
        typeof ev.payload === "object" &&
        ev.payload !== null &&
        "label" in ev.payload
          ? String((ev.payload as { label?: unknown }).label)
          : ev.type;
      setActivityFeed((prev) => {
        const next = [
          {
            id: String(++activityIdRef.current),
            type: ev.type,
            label,
            ts: ev.ts,
          },
          ...prev,
        ].slice(0, 10);
        return next;
      });
    },
  );

  // 3. Data fetch
  const fetchData = useCallback(async () => {
    setError(null);
    try {
      // 8-second timeout per call so a stalled daemon never freezes the skeleton
      const timeout = () => AbortSignal.timeout(8000);
      const [oblRes, projRes, sessRes, healthRes] = await Promise.allSettled([
        fetch("/api/obligations", { signal: timeout() }),
        fetch("/api/projects", { signal: timeout() }),
        fetch("/api/sessions", { signal: timeout() }),
        fetch("/api/server-health", { signal: timeout() }),
      ]);

      // Obligations — daemon returns { obligations: [...] }
      let oblList: ApiObligation[] = [];
      if (oblRes.status === "fulfilled" && oblRes.value.ok) {
        try {
          const oblData = (await oblRes.value.json()) as ObligationsGetResponse;
          oblList = oblData.obligations as ApiObligation[];
        } catch {
          // JSON parse failure — keep empty list
        }
      }
      setObligations(oblList);

      // Projects
      let projectsCount = 0;
      if (projRes.status === "fulfilled" && projRes.value.ok) {
        try {
          projectsCount = ((await projRes.value.json()) as ProjectsGetResponse).projects.length;
        } catch {
          // JSON parse failure — keep 0
        }
      }

      // Sessions
      let sessData: ActiveSessionData[] = [];
      if (sessRes.status === "fulfilled" && sessRes.value.ok) {
        try {
          const raw = (await sessRes.value.json()) as SessionsGetResponse;
          sessData = (raw.sessions ?? []).map(mapNexusSession);
        } catch {
          // JSON parse failure — keep empty list
        }
      }
      setSessions(sessData);

      // Health
      if (healthRes.status === "fulfilled" && healthRes.value.ok) {
        try {
          const hData = (await healthRes.value.json()) as ServerHealthGetResponse;
          if (hData.latest) {
            const mapStatus = (
              s: ServerHealthGetResponse["status"],
            ): HealthSummary["status"] => {
              if (s === "healthy") return "ok";
              if (s === "critical") return "critical";
              return s as "degraded";
            };
            setHealth({
              status: mapStatus(hData.status),
              cpu_percent: hData.latest.cpu_percent ?? 0,
              memory_used_mb: hData.latest.memory_used_mb ?? 0,
              memory_total_mb: hData.latest.memory_total_mb ?? 0,
              uptime_seconds: hData.latest.uptime_seconds ?? 0,
            });
          }
        } catch {
          // JSON parse failure — health stays null
        }
      }

      // Summary — always set so stat cards never remain as skeletons
      setSummary({
        obligations_count: oblList.length,
        active_sessions: sessData.filter((s) => s.status === "active").length,
        idle_sessions: sessData.filter((s) => s.status === "idle").length,
        projects_count: projectsCount,
        messages_today: 0,
        tools_today: 0,
        cost_today_usd: 0,
      });

      // Seed activity from sessions on first load if feed is empty
      setActivityFeed((prev) => {
        if (prev.length > 0) return prev;
        return sessData.slice(0, 10).map((s, i) => ({
          id: String(i + 1),
          type: "session.loaded",
          label: `Session ${s.service} — ${s.status}`,
          ts: new Date(s.started_at).getTime(),
        }));
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load data");
    } finally {
      setLoading(false);
    }
  }, []);

  // 4. Effects — initial load + auto-refresh interval
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

  // 5. Derived
  const topSessions = sessions.slice(0, 5);
  const hVariant = healthVariant(health?.status);
  const cVariant = cpuVariant(health?.cpu_percent ?? 0);
  const mVariant = memVariant(
    health?.memory_used_mb ?? 0,
    health?.memory_total_mb ?? 0,
  );
  const activeSessions = sessions.filter((s) => s.status === "active");
  const isRefreshing = loading;

  // 6. Header action — refresh toggle
  const headerAction = (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={() => setAutoRefresh((v) => !v)}
        className={[
          "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border transition-colors",
          autoRefresh
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
        disabled={isRefreshing}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
      >
        <RefreshCw size={12} className={isRefreshing ? "animate-spin" : ""} />
        Refresh
      </button>
    </div>
  );

  return (
    <PageShell
      title="Dashboard"
      subtitle="Nova activity overview"
      action={headerAction}
    >
      <div className="space-y-6 animate-fade-in-up">
        {error && (
          <ErrorBanner
            message="Failed to load dashboard data"
            detail={error}
            onRetry={() => void fetchData()}
          />
        )}

        {/* Stat cards — 3 columns on desktop */}
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3 section-stagger-1">
          {loading ? (
            Array.from({ length: 6 }).map((_, i) => (
              <div
                key={i}
                className="h-24 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
              />
            ))
          ) : (
            <>
              <div className="animate-fade-in-up stagger-1">
                <StatCard
                  icon={<CheckSquare size={16} />}
                  label="Obligations"
                  value={summary?.obligations_count ?? 0}
                />
              </div>
              <div className="animate-fade-in-up stagger-2">
                <StatCard
                  icon={<Layers size={16} />}
                  label="Active"
                  value={summary?.active_sessions ?? 0}
                  variant="success"
                />
              </div>
              <div className="animate-fade-in-up stagger-3">
                <StatCard
                  icon={<TrendingUp size={16} />}
                  label="Projects"
                  value={summary?.projects_count ?? 0}
                />
              </div>
              <div className="animate-fade-in-up stagger-4">
                <StatCard
                  icon={<Heart size={16} />}
                  label="Health"
                  value={health?.status ?? "—"}
                  variant={hVariant}
                />
              </div>
              <div className="animate-fade-in-up stagger-5">
                <StatCard
                  icon={<Cpu size={16} />}
                  label="CPU"
                  value={
                    health ? `${health.cpu_percent.toFixed(1)}%` : "—"
                  }
                  variant={cVariant}
                />
              </div>
              <div className="animate-fade-in-up stagger-6">
                <StatCard
                  icon={<MemoryStick size={16} />}
                  label="Memory"
                  value={
                    health && health.memory_total_mb > 0
                      ? `${((health.memory_used_mb / health.memory_total_mb) * 100).toFixed(0)}%`
                      : "—"
                  }
                  variant={mVariant}
                  sublabel={
                    health?.uptime_seconds
                      ? `up ${formatUptime(health.uptime_seconds)}`
                      : undefined
                  }
                />
              </div>
            </>
          )}
        </div>

        {/* Two-column layout — 2/3 main + 1/3 sidebar */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Main column — sessions + activity */}
          <div className="lg:col-span-2 space-y-6">
            {/* CC Session widget */}
            <div className="space-y-2 section-stagger-2">
              <SectionHeader label="CC Session" statusDot="muted" />
              <SessionWidget />
            </div>

            {/* Active sessions summary */}
            <div className="space-y-2 section-stagger-3">
              <div className="flex items-center justify-between">
                <SectionHeader
                  label="Active Sessions"
                  count={activeSessions.length}
                  statusDot={activeSessions.length > 0 ? "green" : "muted"}
                />
                <Link
                  href="/sessions"
                  className="flex items-center gap-1 text-xs text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
                >
                  All sessions
                  <ArrowRight size={12} />
                </Link>
              </div>

              {loading ? (
                <div className="space-y-2">
                  {Array.from({ length: 3 }).map((_, i) => (
                    <div
                      key={i}
                      className="h-24 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
                    />
                  ))}
                </div>
              ) : topSessions.length === 0 ? (
                <EmptyState
                  title="No sessions"
                  description="Sessions will appear here when the daemon is active."
                  icon={<Layers size={24} aria-hidden="true" />}
                />
              ) : (
                <div className="space-y-2">
                  {topSessions.map((s) => (
                    <ActiveSession key={s.id} session={s} />
                  ))}
                </div>
              )}
            </div>

            {/* Recent activity feed */}
            <div className="space-y-2 section-stagger-4">
              <SectionHeader
                label="Recent Activity"
                count={activityFeed.length}
              />
              <div className="surface-card p-2">
                <ActivityFeed events={activityFeed} />
              </div>
            </div>
          </div>

          {/* Sidebar column — obligations + metrics */}
          <div className="space-y-4">
            <ObligationsSidebar
              obligations={obligations}
              loading={loading}
            />

            {/* Quick session stats */}
            <div className="surface-card p-4 space-y-3">
              <SectionHeader label="Session Breakdown" />
              {[
                {
                  label: "Active",
                  value: sessions.filter((s) => s.status === "active").length,
                  color: "text-emerald-400",
                },
                {
                  label: "Idle",
                  value: sessions.filter((s) => s.status === "idle").length,
                  color: "text-amber-400",
                },
                {
                  label: "Completed",
                  value: sessions.filter((s) => s.status === "completed").length,
                  color: "text-ds-gray-900",
                },
                {
                  label: "Total",
                  value: sessions.length,
                  color: "text-ds-gray-1000",
                },
              ].map(({ label, value, color }) => (
                <div
                  key={label}
                  className="flex items-center justify-between text-sm"
                >
                  <span className="text-ds-gray-900">{label}</span>
                  <span className={`font-mono font-semibold ${color}`}>
                    {value}
                  </span>
                </div>
              ))}
              <div className="pt-2 border-t border-ds-gray-400">
                <Link
                  href="/sessions"
                  className="flex items-center justify-center gap-1.5 w-full py-1.5 rounded-lg text-xs text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors"
                >
                  <MessageSquare size={12} />
                  View Sessions
                </Link>
              </div>
            </div>

            {/* Messages link */}
            <div className="surface-card p-4 space-y-3">
              <SectionHeader label="Messages" />
              <p className="text-xs text-ds-gray-900">
                View channel messages, search history, and filter by date.
              </p>
              <Link
                href="/messages"
                className="flex items-center justify-center gap-1.5 w-full py-1.5 rounded-lg text-xs text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors"
              >
                <Terminal size={12} />
                Browse Messages
              </Link>
            </div>
          </div>
        </div>
      </div>
    </PageShell>
  );
}
