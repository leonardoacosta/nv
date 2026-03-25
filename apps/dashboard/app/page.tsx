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
import StatCard from "@/components/layout/StatCard";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import SessionWidget from "@/components/SessionWidget";
import ActiveSession, {
  type ActiveSessionData,
} from "@/components/ActiveSession";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";
import type {
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
  title: string;
  owner?: string;
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

function healthColor(status: HealthSummary["status"] | undefined) {
  if (!status || status === "ok") return { bg: "bg-emerald-500/20", text: "text-emerald-400" };
  if (status === "degraded") return { bg: "bg-amber-500/20", text: "text-amber-400" };
  return { bg: "bg-red-500/20", text: "text-red-400" };
}

function cpuColor(pct: number) {
  if (pct >= 90) return { bg: "bg-red-500/20", text: "text-red-400" };
  if (pct >= 70) return { bg: "bg-amber-500/20", text: "text-amber-400" };
  return { bg: "bg-emerald-500/20", text: "text-emerald-400" };
}

function memColor(used: number, total: number) {
  if (total === 0) return { bg: "bg-cosmic-purple/20", text: "text-cosmic-purple" };
  const pct = used / total;
  if (pct >= 0.9) return { bg: "bg-red-500/20", text: "text-red-400" };
  if (pct >= 0.8) return { bg: "bg-amber-500/20", text: "text-amber-400" };
  return { bg: "bg-emerald-500/20", text: "text-emerald-400" };
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
          className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-cosmic-surface/50 transition-colors"
        >
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-cosmic-purple/60 shrink-0" />
          <span className="flex-1 min-w-0 text-sm text-cosmic-text truncate">
            {ev.label}
          </span>
          <span
            className="shrink-0 text-xs text-cosmic-muted font-mono"
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
    (o) => !o.status || o.status === "pending",
  );
  const done = obligations.filter((o) => o.status === "completed");

  return (
    <div className="p-4 rounded-cosmic bg-cosmic-surface border border-cosmic-border space-y-4">
      <div className="flex items-center justify-between">
        <SectionHeader label="Obligations" count={obligations.length} />
        <Link
          href="/obligations"
          className="flex items-center gap-1 text-xs text-cosmic-muted hover:text-cosmic-purple transition-colors"
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
              className="h-8 animate-pulse rounded-lg bg-cosmic-border"
            />
          ))}
        </div>
      ) : obligations.length === 0 ? (
        <p className="text-xs text-cosmic-muted py-3 text-center">
          No obligations
        </p>
      ) : (
        <div className="space-y-2">
          <div className="flex items-center justify-between text-xs text-cosmic-muted">
            <span>Pending</span>
            <span className="font-mono text-amber-400">{pending.length}</span>
          </div>
          <div className="flex items-center justify-between text-xs text-cosmic-muted">
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
                  <span className="text-cosmic-muted truncate max-w-[120px]">
                    @{owner}
                  </span>
                  <span className="font-mono text-cosmic-text">{count}</span>
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
      const [oblRes, projRes, sessRes, healthRes] = await Promise.allSettled([
        fetch("/api/obligations"),
        fetch("/api/projects"),
        fetch("/api/sessions"),
        fetch("/api/server-health"),
      ]);

      // Obligations
      const oblList: ApiObligation[] =
        oblRes.status === "fulfilled" && oblRes.value.ok
          ? ((await oblRes.value.json()) as ApiObligation[])
          : [];
      setObligations(oblList);

      // Projects
      const projectsCount =
        projRes.status === "fulfilled" && projRes.value.ok
          ? ((await projRes.value.json()) as ProjectsGetResponse).projects.length
          : 0;

      // Sessions
      let sessData: ActiveSessionData[] = [];
      if (sessRes.status === "fulfilled" && sessRes.value.ok) {
        const raw = (await sessRes.value.json()) as SessionsGetResponse;
        sessData = (raw.sessions ?? []).map(mapNexusSession);
      }
      setSessions(sessData);

      // Health
      if (healthRes.status === "fulfilled" && healthRes.value.ok) {
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
      }

      // Summary
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
  const hColor = healthColor(health?.status);
  const cColor = cpuColor(health?.cpu_percent ?? 0);
  const mColor = memColor(
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
            ? "bg-cosmic-purple/20 text-cosmic-purple border-cosmic-purple/40 hover:bg-cosmic-purple/30"
            : "text-cosmic-muted border-cosmic-border hover:border-cosmic-purple/50 hover:text-cosmic-text",
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
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-cosmic-muted border border-cosmic-border hover:text-cosmic-text hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
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
      <div className="space-y-6">
        {error && (
          <ErrorBanner
            message="Failed to load dashboard data"
            detail={error}
            onRetry={() => void fetchData()}
          />
        )}

        {/* Stat cards — 3 columns on desktop */}
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
          {loading ? (
            Array.from({ length: 6 }).map((_, i) => (
              <div
                key={i}
                className="h-24 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
              />
            ))
          ) : (
            <>
              <StatCard
                icon={<CheckSquare size={16} />}
                label="Obligations"
                value={summary?.obligations_count ?? 0}
                accentBg="bg-cosmic-purple/20"
                accentText="text-cosmic-purple"
              />
              <StatCard
                icon={<Layers size={16} />}
                label="Active"
                value={summary?.active_sessions ?? 0}
                accentBg="bg-emerald-500/20"
                accentText="text-emerald-400"
              />
              <StatCard
                icon={<TrendingUp size={16} />}
                label="Projects"
                value={summary?.projects_count ?? 0}
                accentBg="bg-cosmic-purple/20"
                accentText="text-cosmic-purple"
              />
              <StatCard
                icon={<Heart size={16} />}
                label="Health"
                value={health?.status ?? "—"}
                accentBg={hColor.bg}
                accentText={hColor.text}
              />
              <StatCard
                icon={<Cpu size={16} />}
                label="CPU"
                value={
                  health ? `${health.cpu_percent.toFixed(1)}%` : "—"
                }
                accentBg={cColor.bg}
                accentText={cColor.text}
              />
              <StatCard
                icon={<MemoryStick size={16} />}
                label="Memory"
                value={
                  health && health.memory_total_mb > 0
                    ? `${((health.memory_used_mb / health.memory_total_mb) * 100).toFixed(0)}%`
                    : "—"
                }
                accentBg={mColor.bg}
                accentText={mColor.text}
                sublabel={
                  health?.uptime_seconds
                    ? `up ${formatUptime(health.uptime_seconds)}`
                    : undefined
                }
              />
            </>
          )}
        </div>

        {/* Two-column layout — 2/3 main + 1/3 sidebar */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Main column — sessions + activity */}
          <div className="lg:col-span-2 space-y-6">
            {/* CC Session widget */}
            <div className="space-y-2">
              <SectionHeader label="CC Session" statusDot="purple" />
              <SessionWidget />
            </div>

            {/* Active sessions summary */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <SectionHeader
                  label="Active Sessions"
                  count={activeSessions.length}
                  statusDot={activeSessions.length > 0 ? "green" : "muted"}
                />
                <Link
                  href="/sessions"
                  className="flex items-center gap-1 text-xs text-cosmic-muted hover:text-cosmic-purple transition-colors"
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
                      className="h-24 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
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
            <div className="space-y-2">
              <SectionHeader
                label="Recent Activity"
                count={activityFeed.length}
              />
              <div className="rounded-cosmic bg-cosmic-surface border border-cosmic-border p-2">
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
            <div className="p-4 rounded-cosmic bg-cosmic-surface border border-cosmic-border space-y-3">
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
                  color: "text-cosmic-muted",
                },
                {
                  label: "Total",
                  value: sessions.length,
                  color: "text-cosmic-text",
                },
              ].map(({ label, value, color }) => (
                <div
                  key={label}
                  className="flex items-center justify-between text-sm"
                >
                  <span className="text-cosmic-muted">{label}</span>
                  <span className={`font-mono font-semibold ${color}`}>
                    {value}
                  </span>
                </div>
              ))}
              <div className="pt-2 border-t border-cosmic-border">
                <Link
                  href="/sessions"
                  className="flex items-center justify-center gap-1.5 w-full py-1.5 rounded-lg text-xs text-cosmic-muted border border-cosmic-border hover:text-cosmic-purple hover:border-cosmic-purple/50 transition-colors"
                >
                  <MessageSquare size={12} />
                  View Sessions
                </Link>
              </div>
            </div>

            {/* Messages link */}
            <div className="p-4 rounded-cosmic bg-cosmic-surface border border-cosmic-border space-y-3">
              <SectionHeader label="Messages" />
              <p className="text-xs text-cosmic-muted">
                View channel messages, search history, and filter by date.
              </p>
              <Link
                href="/messages"
                className="flex items-center justify-center gap-1.5 w-full py-1.5 rounded-lg text-xs text-cosmic-muted border border-cosmic-border hover:text-cosmic-purple hover:border-cosmic-purple/50 transition-colors"
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
