"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  Timer,
  RefreshCw,
  Bell,
  Calendar,
  Eye,
  Sun,
  Layers,
  XCircle,
  Pause,
  Play,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import type {
  AutomationsGetResponse,
  AutomationReminder,
  AutomationSchedule,
} from "@/types/api";
import { apiFetch } from "@/lib/api-client";

// ── Cron-to-human helper ─────────────────────────────────────────────────────

function cronToHuman(expr: string): string {
  const parts = expr.trim().split(/\s+/);
  if (parts.length !== 5) return expr;

  const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

  // "*/N * * * *" — every N minutes
  if (
    minute!.startsWith("*/") &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    const n = parseInt(minute!.slice(2), 10);
    if (!isNaN(n)) return n === 1 ? "Every minute" : `Every ${n} minutes`;
  }

  // "N * * * *" — every hour at :N
  if (
    /^\d+$/.test(minute!) &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    return `Every hour at :${minute!.padStart(2, "0")}`;
  }

  // "M H * * *" — daily at H:M
  if (
    /^\d+$/.test(minute!) &&
    /^\d+$/.test(hour!) &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    const h = parseInt(hour!, 10);
    const m = parseInt(minute!, 10);
    const period = h >= 12 ? "PM" : "AM";
    const h12 = h === 0 ? 12 : h > 12 ? h - 12 : h;
    return `Every day at ${h12}:${String(m).padStart(2, "0")} ${period}`;
  }

  // "M H * * 1-5" — weekdays at H:M
  if (
    /^\d+$/.test(minute!) &&
    /^\d+$/.test(hour!) &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "1-5"
  ) {
    const h = parseInt(hour!, 10);
    const m = parseInt(minute!, 10);
    const period = h >= 12 ? "PM" : "AM";
    const h12 = h === 0 ? 12 : h > 12 ? h - 12 : h;
    return `Weekdays at ${h12}:${String(m).padStart(2, "0")} ${period}`;
  }

  return expr;
}

// ── Time formatting ──────────────────────────────────────────────────────────

function formatRelativeTime(isoStr: string): string {
  const date = new Date(isoStr);
  const now = new Date();
  const diffMs = date.getTime() - now.getTime();
  const absDiffMs = Math.abs(diffMs);

  if (absDiffMs < 60_000) return "just now";

  const minutes = Math.floor(absDiffMs / 60_000);
  const hours = Math.floor(absDiffMs / 3_600_000);
  const days = Math.floor(absDiffMs / 86_400_000);

  const label =
    days > 0
      ? `${days}d ${Math.floor((absDiffMs % 86_400_000) / 3_600_000)}h`
      : hours > 0
        ? `${hours}h ${minutes % 60}m`
        : `${minutes}m`;

  return diffMs > 0 ? `in ${label}` : `${label} ago`;
}

function formatShortTime(isoStr: string): string {
  return new Date(isoStr).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

// ── AutomationsPage ──────────────────────────────────────────────────────────

export default function AutomationsPage() {
  const [data, setData] = useState<AutomationsGetResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionPending, setActionPending] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const res = await apiFetch("/api/automations");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = (await res.json()) as AutomationsGetResponse;
      setData(json);
      setError(null);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load automations",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial fetch + 30s auto-refresh
  useEffect(() => {
    void fetchData();
    intervalRef.current = setInterval(() => void fetchData(), 30_000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchData]);

  // ── Quick actions ────────────────────────────────────────────────────────

  const cancelReminder = useCallback(
    async (id: string) => {
      setActionPending(id);
      try {
        const res = await apiFetch(`/api/automations/reminders/${id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ action: "cancel" }),
        });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        await fetchData();
      } catch {
        // Silently fail — next refresh will show real state
      } finally {
        setActionPending(null);
      }
    },
    [fetchData],
  );

  const toggleSchedule = useCallback(
    async (id: string, enabled: boolean) => {
      setActionPending(id);
      try {
        const res = await apiFetch(`/api/automations/schedules/${id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ enabled }),
        });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        await fetchData();
      } catch {
        // Silently fail
      } finally {
        setActionPending(null);
      }
    },
    [fetchData],
  );

  // ── Render ───────────────────────────────────────────────────────────────

  return (
    <PageShell
      title="Automations"
      subtitle="Reminders, schedules, watcher, and active sessions"
      action={
        <button
          type="button"
          onClick={() => void fetchData()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50 shrink-0"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      }
    >
      {/* Error state */}
      {error && (
        <ErrorBanner
          message="Failed to load automations"
          detail={error}
          onRetry={() => void fetchData()}
        />
      )}

      {/* Loading skeleton */}
      {loading && !data && (
        <div className="space-y-6">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i}>
              <div className="h-4 w-32 animate-pulse rounded bg-ds-gray-100 mb-2" />
              <div className="h-10 animate-pulse rounded bg-ds-gray-100 border border-ds-gray-400" />
              <div className="h-10 animate-pulse rounded bg-ds-gray-100 border-x border-b border-ds-gray-400" />
            </div>
          ))}
        </div>
      )}

      {/* Data loaded */}
      {data && (
        <div className="space-y-6">
          {/* ── Active Reminders ─────────────────────────────────────────── */}
          <section>
            <SectionHeader
              label="Active Reminders"
              count={data.reminders.length}
              statusDot={data.reminders.some((r) => r.status === "overdue") ? "amber" : "green"}
              statusLabel={
                data.reminders.some((r) => r.status === "overdue")
                  ? "Has overdue reminders"
                  : "All reminders on time"
              }
            />
            {data.reminders.length === 0 ? (
              <p className="text-copy-13 text-ds-gray-700 py-2">
                No active reminders
              </p>
            ) : (
              <div className="border border-ds-gray-400 rounded-md overflow-hidden">
                <table className="w-full text-copy-13">
                  <thead>
                    <tr className="border-b border-ds-gray-400 bg-ds-gray-alpha-100">
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Message</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Due</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Channel</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Status</th>
                      <th className="text-right text-label-12 text-ds-gray-700 px-3 py-1.5">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.reminders.map((r: AutomationReminder) => (
                      <tr
                        key={r.id}
                        className="border-b border-ds-gray-400 last:border-b-0 hover:bg-ds-gray-alpha-100 transition-colors"
                      >
                        <td className="px-3 py-1.5 text-ds-gray-1000 max-w-xs truncate">
                          {r.message}
                        </td>
                        <td className="px-3 py-1.5 text-ds-gray-900 whitespace-nowrap">
                          <span title={formatShortTime(r.due_at)}>
                            {formatRelativeTime(r.due_at)}
                          </span>
                        </td>
                        <td className="px-3 py-1.5 text-ds-gray-900">{r.channel}</td>
                        <td className="px-3 py-1.5">
                          <span
                            className={`inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium ${
                              r.status === "overdue"
                                ? "bg-amber-700/20 text-amber-700"
                                : "bg-green-700/20 text-green-700"
                            }`}
                          >
                            {r.status}
                          </span>
                        </td>
                        <td className="px-3 py-1.5 text-right">
                          <button
                            type="button"
                            onClick={() => void cancelReminder(r.id)}
                            disabled={actionPending === r.id}
                            className="inline-flex items-center gap-1 px-2 py-1 rounded text-xs text-red-700 hover:bg-red-700/10 transition-colors disabled:opacity-50"
                            title="Cancel reminder"
                          >
                            <XCircle size={12} />
                            Cancel
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {/* ── Scheduled Jobs ───────────────────────────────────────────── */}
          <section>
            <SectionHeader
              label="Scheduled Jobs"
              count={data.schedules.length}
              statusDot={data.schedules.some((s) => s.enabled) ? "green" : "muted"}
              statusLabel={
                data.schedules.some((s) => s.enabled)
                  ? "Active schedules running"
                  : "No active schedules"
              }
            />
            {data.schedules.length === 0 ? (
              <p className="text-copy-13 text-ds-gray-700 py-2">
                No scheduled jobs
              </p>
            ) : (
              <div className="border border-ds-gray-400 rounded-md overflow-hidden">
                <table className="w-full text-copy-13">
                  <thead>
                    <tr className="border-b border-ds-gray-400 bg-ds-gray-alpha-100">
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Name</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Schedule</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Last Run</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Next Run</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Status</th>
                      <th className="text-right text-label-12 text-ds-gray-700 px-3 py-1.5">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.schedules.map((s: AutomationSchedule) => (
                      <tr
                        key={s.id}
                        className="border-b border-ds-gray-400 last:border-b-0 hover:bg-ds-gray-alpha-100 transition-colors"
                      >
                        <td className="px-3 py-1.5 text-ds-gray-1000 font-medium">
                          {s.name}
                        </td>
                        <td className="px-3 py-1.5 text-ds-gray-900" title={s.cron_expr}>
                          {cronToHuman(s.cron_expr)}
                        </td>
                        <td className="px-3 py-1.5 text-ds-gray-900 whitespace-nowrap">
                          {s.last_run_at ? (
                            <span title={formatShortTime(s.last_run_at)}>
                              {formatRelativeTime(s.last_run_at)}
                            </span>
                          ) : (
                            <span className="text-ds-gray-600">Never</span>
                          )}
                        </td>
                        <td className="px-3 py-1.5 text-ds-gray-900 whitespace-nowrap">
                          {s.next_run ? (
                            <span title={formatShortTime(s.next_run)}>
                              {formatRelativeTime(s.next_run)}
                            </span>
                          ) : (
                            <span className="text-ds-gray-600">--</span>
                          )}
                        </td>
                        <td className="px-3 py-1.5">
                          <span
                            className={`inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium ${
                              s.enabled
                                ? "bg-green-700/20 text-green-700"
                                : "bg-ds-gray-alpha-200 text-ds-gray-700"
                            }`}
                          >
                            {s.enabled ? "enabled" : "paused"}
                          </span>
                        </td>
                        <td className="px-3 py-1.5 text-right">
                          <button
                            type="button"
                            onClick={() => void toggleSchedule(s.id, !s.enabled)}
                            disabled={actionPending === s.id}
                            className="inline-flex items-center gap-1 px-2 py-1 rounded text-xs text-ds-gray-900 hover:bg-ds-gray-alpha-200 transition-colors disabled:opacity-50"
                            title={s.enabled ? "Pause schedule" : "Resume schedule"}
                          >
                            {s.enabled ? <Pause size={12} /> : <Play size={12} />}
                            {s.enabled ? "Pause" : "Resume"}
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {/* ── Proactive Watcher ────────────────────────────────────────── */}
          <section>
            <SectionHeader
              label="Proactive Watcher"
              statusDot={data.watcher.enabled ? "green" : "muted"}
              statusLabel={data.watcher.enabled ? "Watcher active" : "Watcher disabled"}
            />
            <div className="border border-ds-gray-400 rounded-md overflow-hidden">
              <div className="flex flex-wrap gap-x-8 gap-y-1 px-3 py-2 text-copy-13">
                <div>
                  <span className="text-ds-gray-700">Status: </span>
                  <span
                    className={`inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium ${
                      data.watcher.enabled
                        ? "bg-green-700/20 text-green-700"
                        : "bg-ds-gray-alpha-200 text-ds-gray-700"
                    }`}
                  >
                    {data.watcher.enabled ? "active" : "disabled"}
                  </span>
                </div>
                <div>
                  <span className="text-ds-gray-700">Interval: </span>
                  <span className="text-ds-gray-1000">
                    every {data.watcher.interval_minutes}m
                  </span>
                </div>
                <div>
                  <span className="text-ds-gray-700">Quiet hours: </span>
                  <span className="text-ds-gray-1000">
                    {data.watcher.quiet_start} -- {data.watcher.quiet_end}
                  </span>
                </div>
                {data.watcher.last_run_at && (
                  <div>
                    <span className="text-ds-gray-700">Last run: </span>
                    <span className="text-ds-gray-1000">
                      {formatRelativeTime(data.watcher.last_run_at)}
                    </span>
                  </div>
                )}
              </div>
            </div>
          </section>

          {/* ── Briefing Schedule ────────────────────────────────────────── */}
          <section>
            <SectionHeader
              label="Briefing Schedule"
              statusDot={data.briefing.last_generated_at ? "green" : "muted"}
              statusLabel={
                data.briefing.last_generated_at
                  ? "Briefing generated"
                  : "No briefing yet"
              }
            />
            <div className="border border-ds-gray-400 rounded-md overflow-hidden">
              <div className="flex flex-wrap gap-x-8 gap-y-1 px-3 py-2 text-copy-13">
                <div>
                  <span className="text-ds-gray-700">Last generated: </span>
                  <span className="text-ds-gray-1000">
                    {data.briefing.last_generated_at
                      ? formatShortTime(data.briefing.last_generated_at)
                      : "Never"}
                  </span>
                </div>
                <div>
                  <span className="text-ds-gray-700">Next generation: </span>
                  <span className="text-ds-gray-1000">
                    {data.briefing.next_generation
                      ? formatRelativeTime(data.briefing.next_generation)
                      : "--"}
                  </span>
                </div>
              </div>
            </div>
          </section>

          {/* ── Active Sessions ──────────────────────────────────────────── */}
          <section>
            <SectionHeader
              label="Active Sessions"
              count={data.active_sessions.length}
              statusDot={data.active_sessions.length > 0 ? "green" : "muted"}
              statusLabel={
                data.active_sessions.length > 0
                  ? `${data.active_sessions.length} running`
                  : "No active sessions"
              }
            />
            {data.active_sessions.length === 0 ? (
              <p className="text-copy-13 text-ds-gray-700 py-2">
                No active sessions
              </p>
            ) : (
              <div className="border border-ds-gray-400 rounded-md overflow-hidden">
                <table className="w-full text-copy-13">
                  <thead>
                    <tr className="border-b border-ds-gray-400 bg-ds-gray-alpha-100">
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Project</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Command</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Status</th>
                      <th className="text-left text-label-12 text-ds-gray-700 px-3 py-1.5">Started</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.active_sessions.map((s) => (
                      <tr
                        key={s.id}
                        className="border-b border-ds-gray-400 last:border-b-0 hover:bg-ds-gray-alpha-100 transition-colors"
                      >
                        <td className="px-3 py-1.5 text-ds-gray-1000 font-medium">
                          {s.project}
                        </td>
                        <td className="px-3 py-1.5 text-ds-gray-900 font-mono text-xs">
                          {s.command}
                        </td>
                        <td className="px-3 py-1.5">
                          <span className="inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium bg-green-700/20 text-green-700">
                            {s.status}
                          </span>
                        </td>
                        <td className="px-3 py-1.5 text-ds-gray-900 whitespace-nowrap">
                          <span title={formatShortTime(s.started_at)}>
                            {formatRelativeTime(s.started_at)}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        </div>
      )}
    </PageShell>
  );
}
