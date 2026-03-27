"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import {
  RefreshCw,
  Bell,
  Calendar,
  Eye,
  XCircle,
  Pause,
  Play,
  Loader2,
  Info,
  ExternalLink,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import type {
  AutomationsGetResponse,
  AutomationReminder,
  AutomationSchedule,
  AutomationWatcher,
} from "@/types/api";
import { apiFetch } from "@/lib/api-client";

// ── Cron-to-human helper ─────────────────────────────────────────────────────

function cronToHuman(expr: string): string {
  const parts = expr.trim().split(/\s+/);
  if (parts.length !== 5) return expr;
  const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

  if (minute!.startsWith("*/") && hour === "*" && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    const n = parseInt(minute!.slice(2), 10);
    if (!isNaN(n)) return n === 1 ? "Every minute" : `Every ${n} minutes`;
  }
  if (/^\d+$/.test(minute!) && hour === "*" && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    return `Every hour at :${minute!.padStart(2, "0")}`;
  }
  if (/^\d+$/.test(minute!) && /^\d+$/.test(hour!) && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    const h = parseInt(hour!, 10);
    const m = parseInt(minute!, 10);
    const period = h >= 12 ? "PM" : "AM";
    const h12 = h === 0 ? 12 : h > 12 ? h - 12 : h;
    return `Every day at ${h12}:${String(m).padStart(2, "0")} ${period}`;
  }
  if (/^\d+$/.test(minute!) && /^\d+$/.test(hour!) && dayOfMonth === "*" && month === "*" && dayOfWeek === "1-5") {
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

// ── Toggle Switch ────────────────────────────────────────────────────────────

function ToggleSwitch({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean;
  onChange: (val: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={[
        "relative inline-flex h-5 w-9 items-center rounded-full transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-ds-gray-500 disabled:opacity-50",
        checked ? "bg-green-600" : "bg-ds-gray-400",
      ].join(" ")}
    >
      <span
        className={[
          "inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform duration-200",
          checked ? "translate-x-4" : "translate-x-0.5",
        ].join(" ")}
      />
    </button>
  );
}

// ── Watcher Card ─────────────────────────────────────────────────────────────

interface WatcherCardProps {
  watcher: AutomationWatcher;
  onUpdate: (patch: Partial<AutomationWatcher>) => Promise<void>;
}

function WatcherCard({ watcher, onUpdate }: WatcherCardProps) {
  const [localEnabled, setLocalEnabled] = useState(watcher.enabled);
  const [localInterval, setLocalInterval] = useState(watcher.interval_minutes);
  const [localQuietStart, setLocalQuietStart] = useState(watcher.quiet_start);
  const [localQuietEnd, setLocalQuietEnd] = useState(watcher.quiet_end);
  const [saving, setSaving] = useState<string | null>(null);
  const [fieldError, setFieldError] = useState<string | null>(null);

  // Sync local state when prop changes
  useEffect(() => {
    setLocalEnabled(watcher.enabled);
    setLocalInterval(watcher.interval_minutes);
    setLocalQuietStart(watcher.quiet_start);
    setLocalQuietEnd(watcher.quiet_end);
  }, [watcher]);

  const patch = useCallback(
    async (field: string, value: Partial<AutomationWatcher>) => {
      setSaving(field);
      setFieldError(null);
      try {
        await onUpdate(value);
      } catch (err) {
        // Revert local state on failure
        setLocalEnabled(watcher.enabled);
        setLocalInterval(watcher.interval_minutes);
        setLocalQuietStart(watcher.quiet_start);
        setLocalQuietEnd(watcher.quiet_end);
        setFieldError(err instanceof Error ? err.message : "Failed to save");
      } finally {
        setSaving(null);
      }
    },
    [onUpdate, watcher],
  );

  const handleToggle = async (val: boolean) => {
    setLocalEnabled(val);
    await patch("enabled", { enabled: val });
  };

  const handleIntervalBlur = async () => {
    if (localInterval !== watcher.interval_minutes) {
      await patch("interval", { interval_minutes: localInterval });
    }
  };

  const handleIntervalChange = (val: number) => {
    const clamped = Math.max(5, Math.min(120, Math.round(val / 5) * 5));
    setLocalInterval(clamped);
  };

  const handleQuietStartBlur = async () => {
    if (localQuietStart !== watcher.quiet_start) {
      await patch("quiet_start", { quiet_start: localQuietStart });
    }
  };

  const handleQuietEndBlur = async () => {
    if (localQuietEnd !== watcher.quiet_end) {
      await patch("quiet_end", { quiet_end: localQuietEnd });
    }
  };

  return (
    <div className="surface-card p-4 flex flex-col gap-3 min-h-[120px]">
      <SectionHeader
        label="Proactive Watcher"
        statusDot={localEnabled ? "green" : "muted"}
        statusLabel={localEnabled ? "Watcher active" : "Watcher disabled"}
      />
      <p className="text-copy-13 text-ds-gray-700">
        Monitors Telegram channels for actionable items. Configured via daemon watcher settings.
      </p>

      {fieldError && (
        <p className="text-[11px] text-red-500">{fieldError}</p>
      )}

      <div className="space-y-3">
        {/* Enabled toggle */}
        <div className="flex items-center justify-between gap-4">
          <span className="text-copy-13 text-ds-gray-900">Enabled</span>
          <div className="flex items-center gap-2">
            {saving === "enabled" && <Loader2 size={12} className="animate-spin text-ds-gray-700" />}
            <ToggleSwitch
              checked={localEnabled}
              onChange={(val) => void handleToggle(val)}
              disabled={saving !== null}
              label="Toggle watcher enabled"
            />
          </div>
        </div>

        {/* Interval stepper */}
        <div className="flex items-center justify-between gap-4">
          <span className="text-copy-13 text-ds-gray-900">Interval</span>
          <div className="flex items-center gap-1.5">
            {saving === "interval" && <Loader2 size={12} className="animate-spin text-ds-gray-700" />}
            <button
              type="button"
              onClick={() => handleIntervalChange(localInterval - 5)}
              disabled={localInterval <= 5 || saving !== null}
              className="flex items-center justify-center w-6 h-6 rounded border border-ds-gray-400 text-ds-gray-900 hover:bg-ds-gray-alpha-200 transition-colors disabled:opacity-40"
              aria-label="Decrease interval"
            >
              −
            </button>
            <input
              type="number"
              min={5}
              max={120}
              step={5}
              value={localInterval}
              onChange={(e) => handleIntervalChange(Number(e.target.value))}
              onBlur={() => void handleIntervalBlur()}
              disabled={saving !== null}
              className="w-14 px-2 py-1 text-center text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded focus:outline-none focus:border-ds-gray-500 disabled:opacity-50"
            />
            <button
              type="button"
              onClick={() => handleIntervalChange(localInterval + 5)}
              disabled={localInterval >= 120 || saving !== null}
              className="flex items-center justify-center w-6 h-6 rounded border border-ds-gray-400 text-ds-gray-900 hover:bg-ds-gray-alpha-200 transition-colors disabled:opacity-40"
              aria-label="Increase interval"
            >
              +
            </button>
            <span className="text-copy-13 text-ds-gray-700">min</span>
          </div>
        </div>

        {/* Quiet hours */}
        <div className="flex items-center justify-between gap-4">
          <span className="text-copy-13 text-ds-gray-900">Quiet hours</span>
          <div className="flex items-center gap-1.5">
            {saving === "quiet_start" || saving === "quiet_end" ? (
              <Loader2 size={12} className="animate-spin text-ds-gray-700" />
            ) : null}
            <input
              type="time"
              value={localQuietStart}
              onChange={(e) => setLocalQuietStart(e.target.value)}
              onBlur={() => void handleQuietStartBlur()}
              disabled={saving !== null}
              className="px-2 py-1 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded focus:outline-none focus:border-ds-gray-500 disabled:opacity-50"
            />
            <span className="text-copy-13 text-ds-gray-700">to</span>
            <input
              type="time"
              value={localQuietEnd}
              onChange={(e) => setLocalQuietEnd(e.target.value)}
              onBlur={() => void handleQuietEndBlur()}
              disabled={saving !== null}
              className="px-2 py-1 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded focus:outline-none focus:border-ds-gray-500 disabled:opacity-50"
            />
          </div>
        </div>

        {/* Last run */}
        {watcher.last_run_at && (
          <div className="flex items-center justify-between">
            <span className="text-copy-13 text-ds-gray-700">Last run</span>
            <span className="text-copy-13 text-ds-gray-900" suppressHydrationWarning>
              {formatRelativeTime(watcher.last_run_at)}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Briefing Card ────────────────────────────────────────────────────────────

interface BriefingCardProps {
  lastGeneratedAt: string | null;
  nextGeneration: string | null;
  contentPreview: string | null;
  onGenerated: () => void;
}

function BriefingCard({
  lastGeneratedAt,
  nextGeneration,
  contentPreview,
  onGenerated,
}: BriefingCardProps) {
  const [generating, setGenerating] = useState(false);
  const [genError, setGenError] = useState<string | null>(null);

  const handleGenerateNow = async () => {
    setGenerating(true);
    setGenError(null);
    try {
      const res = await apiFetch("/api/briefing/generate", { method: "POST" });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      onGenerated();
    } catch (err) {
      setGenError(err instanceof Error ? err.message : "Failed to generate briefing");
    } finally {
      setGenerating(false);
    }
  };

  return (
    <div className="surface-card p-4 flex flex-col gap-3 min-h-[120px]">
      <SectionHeader
        label="Briefing Schedule"
        statusDot={lastGeneratedAt ? "green" : "muted"}
        statusLabel={lastGeneratedAt ? "Briefing generated" : "No briefing yet"}
      />
      <p className="text-copy-13 text-ds-gray-700">
        Generates a daily summary at the configured time. Trigger manually with Generate Now.
      </p>

      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-copy-13 text-ds-gray-700">Last generated</span>
          <span className="text-copy-13 text-ds-gray-900">
            {lastGeneratedAt ? (
              <span title={formatShortTime(lastGeneratedAt)} suppressHydrationWarning>
                {formatShortTime(lastGeneratedAt)}
              </span>
            ) : (
              "Never"
            )}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-copy-13 text-ds-gray-700">Next generation</span>
          <span className="text-copy-13 text-ds-gray-900">
            {nextGeneration ? (
              <span suppressHydrationWarning>{formatRelativeTime(nextGeneration)}</span>
            ) : (
              "--"
            )}
          </span>
        </div>
      </div>

      {/* Content preview */}
      {contentPreview && (
        <Link
          href="/briefing"
          className="block text-copy-13 text-ds-gray-900 bg-ds-gray-100 border border-ds-gray-400 rounded-lg px-3 py-2 line-clamp-3 hover:bg-ds-gray-200 hover:border-ds-gray-500 transition-colors"
        >
          {contentPreview}
        </Link>
      )}

      {genError && (
        <p className="text-[11px] text-red-500">{genError}</p>
      )}

      <div className="flex items-center gap-3 mt-1">
        <button
          type="button"
          onClick={() => void handleGenerateNow()}
          disabled={generating}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          {generating ? (
            <Loader2 size={13} className="animate-spin" />
          ) : (
            <Eye size={13} />
          )}
          {generating ? "Generating..." : "Generate Now"}
        </button>
        <Link
          href="/briefing"
          className="flex items-center gap-1 text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 transition-colors hover:underline underline-offset-2"
        >
          <ExternalLink size={12} />
          View Briefing
        </Link>
      </div>
    </div>
  );
}

// ── Reminders table ──────────────────────────────────────────────────────────

function RemindersTab({
  reminders,
  actionPending,
  onCancel,
}: {
  reminders: AutomationReminder[];
  actionPending: string | null;
  onCancel: (id: string) => void;
}) {
  return (
    <div>
      <p className="text-copy-13 text-ds-gray-700 mb-3">
        Created via Telegram ("remind me to...") or the API. One-time alerts delivered to a channel.
      </p>
      {reminders.length === 0 ? (
        <div className="flex items-start gap-2 py-4 px-3 rounded-lg bg-ds-gray-alpha-100 border border-ds-gray-400">
          <Info size={14} className="text-ds-gray-700 shrink-0 mt-0.5" />
          <p className="text-copy-13 text-ds-gray-700">
            No active reminders. Tell Nova &ldquo;remind me to...&rdquo; in Telegram, or create one via the API.
          </p>
        </div>
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
              {reminders.map((r: AutomationReminder) => (
                <tr
                  key={r.id}
                  className="border-b border-ds-gray-400 last:border-b-0 hover:bg-ds-gray-alpha-100 transition-colors"
                >
                  <td className="px-3 py-1.5 text-ds-gray-1000 max-w-xs truncate">{r.message}</td>
                  <td className="px-3 py-1.5 text-ds-gray-900 whitespace-nowrap">
                    <span title={formatShortTime(r.due_at)} suppressHydrationWarning>
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
                      onClick={() => onCancel(r.id)}
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
    </div>
  );
}

// ── Schedules table ──────────────────────────────────────────────────────────

function SchedulesTab({
  schedules,
  actionPending,
  onToggle,
}: {
  schedules: AutomationSchedule[];
  actionPending: string | null;
  onToggle: (id: string, enabled: boolean) => void;
}) {
  return (
    <div>
      <p className="text-copy-13 text-ds-gray-700 mb-3">
        Recurring jobs configured in daemon schedule-svc config. Toggle enabled/disabled here.
      </p>
      {schedules.length === 0 ? (
        <div className="flex items-start gap-2 py-4 px-3 rounded-lg bg-ds-gray-alpha-100 border border-ds-gray-400">
          <Info size={14} className="text-ds-gray-700 shrink-0 mt-0.5" />
          <p className="text-copy-13 text-ds-gray-700">
            No scheduled jobs. Schedules are configured in the daemon schedule-svc config.
          </p>
        </div>
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
              {schedules.map((s: AutomationSchedule) => (
                <tr
                  key={s.id}
                  className="border-b border-ds-gray-400 last:border-b-0 hover:bg-ds-gray-alpha-100 transition-colors"
                >
                  <td className="px-3 py-1.5 text-ds-gray-1000 font-medium">{s.name}</td>
                  <td className="px-3 py-1.5 text-ds-gray-900" title={s.cron_expr}>
                    {cronToHuman(s.cron_expr)}
                  </td>
                  <td className="px-3 py-1.5 text-ds-gray-900 whitespace-nowrap">
                    {s.last_run_at ? (
                      <span title={formatShortTime(s.last_run_at)} suppressHydrationWarning>
                        {formatRelativeTime(s.last_run_at)}
                      </span>
                    ) : (
                      <span className="text-ds-gray-600">Never</span>
                    )}
                  </td>
                  <td className="px-3 py-1.5 text-ds-gray-900 whitespace-nowrap">
                    {s.next_run ? (
                      <span title={formatShortTime(s.next_run)} suppressHydrationWarning>
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
                      onClick={() => onToggle(s.id, !s.enabled)}
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
    </div>
  );
}

// ── AutomationsPage ──────────────────────────────────────────────────────────

type ScheduledTab = "reminders" | "schedules";

export default function AutomationsPage() {
  const [data, setData] = useState<AutomationsGetResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionPending, setActionPending] = useState<string | null>(null);
  const [scheduledTab, setScheduledTab] = useState<ScheduledTab>("reminders");
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const res = await apiFetch("/api/automations");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = (await res.json()) as AutomationsGetResponse;
      setData(json);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load automations");
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

  const patchWatcher = useCallback(
    async (patch: Partial<AutomationWatcher>) => {
      const res = await apiFetch("/api/automations/watcher", {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(patch),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      await fetchData();
    },
    [fetchData],
  );

  // Sessions count for cross-link
  const sessionCount = data?.active_sessions.length ?? 0;

  // ── Render ───────────────────────────────────────────────────────────────

  return (
    <PageShell
      title="Automations"
      subtitle="Watcher, briefing, and scheduled tasks"
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
        <div className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {[0, 1].map((i) => (
              <div key={i} className="h-40 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400" />
            ))}
          </div>
          <div className="h-10 animate-pulse rounded bg-ds-gray-100 border border-ds-gray-400" />
          <div className="h-10 animate-pulse rounded bg-ds-gray-100 border-x border-b border-ds-gray-400" />
        </div>
      )}

      {/* Data loaded */}
      {data && (
        <div className="space-y-5">
          {/* ── Sessions cross-link ──────────────────────────────────────── */}
          <div className="flex items-center gap-1">
            <Link
              href="/sessions"
              className="text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 underline-offset-2 hover:underline transition-colors"
            >
              {sessionCount > 0
                ? `${sessionCount} active session${sessionCount !== 1 ? "s" : ""}`
                : "No active sessions"}
            </Link>
            <ExternalLink size={11} className="text-ds-gray-700" />
          </div>

          {/* ── Top row: Watcher + Briefing ──────────────────────────────── */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <WatcherCard watcher={data.watcher} onUpdate={patchWatcher} />
            <BriefingCard
              lastGeneratedAt={data.briefing.last_generated_at}
              nextGeneration={data.briefing.next_generation}
              contentPreview={data.briefing.content_preview}
              onGenerated={() => void fetchData()}
            />
          </div>

          {/* ── Bottom row: Scheduled Automations ────────────────────────── */}
          <section>
            <SectionHeader
              label="Scheduled Automations"
              count={data.reminders.length + data.schedules.length}
              statusDot={
                data.reminders.some((r) => r.status === "overdue")
                  ? "amber"
                  : data.schedules.some((s) => s.enabled)
                    ? "green"
                    : "muted"
              }
              statusLabel="Scheduled automations"
            />

            {/* Segmented control tabs */}
            <div className="flex gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400 w-fit mt-2 mb-4">
              {(
                [
                  { key: "reminders" as const, icon: <Bell size={13} />, label: "Reminders", count: data.reminders.length },
                  { key: "schedules" as const, icon: <Calendar size={13} />, label: "Schedules", count: data.schedules.length },
                ] as const
              ).map((t) => (
                <button
                  key={t.key}
                  type="button"
                  onClick={() => setScheduledTab(t.key)}
                  className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm font-medium transition-colors ${
                    scheduledTab === t.key
                      ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                      : "text-ds-gray-900 hover:text-ds-gray-1000"
                  }`}
                >
                  {t.icon}
                  <span>{t.label}</span>
                  <span className="text-xs font-mono opacity-70">{t.count}</span>
                </button>
              ))}
            </div>

            {scheduledTab === "reminders" ? (
              <RemindersTab
                reminders={data.reminders}
                actionPending={actionPending}
                onCancel={(id) => void cancelReminder(id)}
              />
            ) : (
              <SchedulesTab
                schedules={data.schedules}
                actionPending={actionPending}
                onToggle={(id, enabled) => void toggleSchedule(id, enabled)}
              />
            )}
          </section>
        </div>
      )}
    </PageShell>
  );
}
