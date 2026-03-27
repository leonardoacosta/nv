"use client";

import { useCallback, useEffect, useState } from "react";
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
  ChevronDown,
  ChevronRight,
  Plus,
  Check,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import SectionHeader from "@/components/layout/SectionHeader";
import ErrorBanner from "@/components/layout/ErrorBanner";
import type {
  AutomationsGetResponse,
  AutomationReminder,
  AutomationSchedule,
  AutomationWatcher,
  AutomationSettingsResponse,
} from "@/types/api";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTRPC } from "@/lib/trpc/react";
// apiFetch retained for reminder creation (no tRPC procedure exists yet)
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
        "relative inline-flex h-5 w-9 items-center rounded-full transition-colors duration-200 focus:outline-hidden focus:ring-2 focus:ring-ds-gray-500 disabled:opacity-50",
        checked ? "bg-green-700" : "bg-ds-gray-400",
      ].join(" ")}
    >
      <span
        className={[
          "inline-block h-4 w-4 transform rounded-full bg-white shadow-sm transition-transform duration-200",
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
  promptValue: string;
  onPromptSave: (value: string) => Promise<void>;
}

function WatcherCard({ watcher, onUpdate, promptValue, onPromptSave }: WatcherCardProps) {
  const [localEnabled, setLocalEnabled] = useState(watcher.enabled);
  const [localInterval, setLocalInterval] = useState(watcher.interval_minutes);
  const [localQuietStart, setLocalQuietStart] = useState(watcher.quiet_start);
  const [localQuietEnd, setLocalQuietEnd] = useState(watcher.quiet_end);
  const [saving, setSaving] = useState<string | null>(null);
  const [fieldError, setFieldError] = useState<string | null>(null);
  const [promptOpen, setPromptOpen] = useState(false);
  const [localPrompt, setLocalPrompt] = useState(promptValue);
  const [promptSaveState, setPromptSaveState] = useState<"idle" | "saving" | "saved">("idle");

  // Sync local state when prop changes
  useEffect(() => {
    setLocalEnabled(watcher.enabled);
    setLocalInterval(watcher.interval_minutes);
    setLocalQuietStart(watcher.quiet_start);
    setLocalQuietEnd(watcher.quiet_end);
  }, [watcher]);

  // Sync prompt from parent
  useEffect(() => {
    setLocalPrompt(promptValue);
  }, [promptValue]);

  const handlePromptBlur = async () => {
    if (localPrompt === promptValue) return;
    setPromptSaveState("saving");
    try {
      await onPromptSave(localPrompt);
      setPromptSaveState("saved");
      setTimeout(() => setPromptSaveState("idle"), 2000);
    } catch {
      setPromptSaveState("idle");
      setFieldError("Failed to save prompt");
    }
  };

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
        <p className="text-[11px] text-red-700">{fieldError}</p>
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
              className="w-14 px-2 py-1 text-center text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded focus:outline-hidden focus:border-ds-gray-500 disabled:opacity-50"
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
              className="px-2 py-1 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded focus:outline-hidden focus:border-ds-gray-500 disabled:opacity-50"
            />
            <span className="text-copy-13 text-ds-gray-700">to</span>
            <input
              type="time"
              value={localQuietEnd}
              onChange={(e) => setLocalQuietEnd(e.target.value)}
              onBlur={() => void handleQuietEndBlur()}
              disabled={saving !== null}
              className="px-2 py-1 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded focus:outline-hidden focus:border-ds-gray-500 disabled:opacity-50"
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

        {/* Custom Prompt (task 3.1) */}
        <div className="border-t border-ds-gray-400 pt-3">
          <button
            type="button"
            onClick={() => setPromptOpen(!promptOpen)}
            className="flex items-center gap-1.5 text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
          >
            {promptOpen ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
            Custom Prompt
            {promptSaveState === "saving" && (
              <Loader2 size={11} className="animate-spin text-ds-gray-700 ml-1" />
            )}
            {promptSaveState === "saved" && (
              <Check size={11} className="text-green-700 ml-1" />
            )}
          </button>
          {promptOpen && (
            <textarea
              value={localPrompt}
              onChange={(e) => setLocalPrompt(e.target.value)}
              onBlur={() => void handlePromptBlur()}
              placeholder="Describe what the watcher should look for (e.g., overdue obligations, calendar conflicts)..."
              rows={3}
              className="mt-2 w-full px-3 py-2 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded-lg focus:outline-hidden focus:border-ds-gray-500 placeholder:text-ds-gray-700 resize-y"
            />
          )}
        </div>
      </div>
    </div>
  );
}

// ── Briefing Card ────────────────────────────────────────────────────────────

interface BriefingCardProps {
  lastGeneratedAt: string | null;
  nextGeneration: string | null;
  contentPreview: string | null;
  briefingHour: number;
  onGenerated: () => void;
  promptValue: string;
  onPromptSave: (value: string) => Promise<void>;
  onHourSave: (hour: number) => Promise<void>;
}

function BriefingCard({
  lastGeneratedAt,
  nextGeneration,
  contentPreview,
  briefingHour,
  onGenerated,
  promptValue,
  onPromptSave,
  onHourSave,
}: BriefingCardProps) {
  const trpc = useTRPC();
  const [generating, setGenerating] = useState(false);
  const [genError, setGenError] = useState<string | null>(null);
  const [promptOpen, setPromptOpen] = useState(false);
  const [localPrompt, setLocalPrompt] = useState(promptValue);
  const [promptSaveState, setPromptSaveState] = useState<"idle" | "saving" | "saved">("idle");
  const [localHour, setLocalHour] = useState(briefingHour);
  const [localNextGen, setLocalNextGen] = useState(nextGeneration);
  const [hourSaveState, setHourSaveState] = useState<"idle" | "saving" | "saved">("idle");

  // Sync prompt from parent
  useEffect(() => {
    setLocalPrompt(promptValue);
  }, [promptValue]);

  // Sync hour and next generation from parent
  useEffect(() => {
    setLocalHour(briefingHour);
  }, [briefingHour]);

  useEffect(() => {
    setLocalNextGen(nextGeneration);
  }, [nextGeneration]);

  const handlePromptBlur = async () => {
    if (localPrompt === promptValue) return;
    setPromptSaveState("saving");
    try {
      await onPromptSave(localPrompt);
      setPromptSaveState("saved");
      setTimeout(() => setPromptSaveState("idle"), 2000);
    } catch {
      setPromptSaveState("idle");
    }
  };

  const handleHourChange = async (newHour: number) => {
    setLocalHour(newHour);
    // Optimistically update next_generation display
    const now = new Date();
    const next = new Date(now);
    next.setHours(newHour, 0, 0, 0);
    if (next <= now) next.setDate(next.getDate() + 1);
    setLocalNextGen(next.toISOString());
    setHourSaveState("saving");
    try {
      await onHourSave(newHour);
      setHourSaveState("saved");
      setTimeout(() => setHourSaveState("idle"), 2000);
    } catch {
      setLocalHour(briefingHour);
      setLocalNextGen(nextGeneration);
      setHourSaveState("idle");
    }
  };

  const generateMutation = useMutation(trpc.briefing.generate.mutationOptions());

  const handleGenerateNow = async () => {
    setGenerating(true);
    setGenError(null);
    try {
      await generateMutation.mutateAsync();
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
            {localNextGen ? (
              <span suppressHydrationWarning>{formatRelativeTime(localNextGen)}</span>
            ) : (
              "--"
            )}
          </span>
        </div>

        {/* Hour picker (task 3.3) */}
        <div className="flex items-center justify-between gap-4">
          <span className="text-copy-13 text-ds-gray-900">Briefing hour</span>
          <div className="flex items-center gap-1.5">
            {hourSaveState === "saving" && <Loader2 size={12} className="animate-spin text-ds-gray-700" />}
            {hourSaveState === "saved" && <Check size={11} className="text-green-700" />}
            <select
              value={localHour}
              onChange={(e) => void handleHourChange(Number(e.target.value))}
              className="px-2 py-1 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded focus:outline-hidden focus:border-ds-gray-500"
            >
              {Array.from({ length: 24 }, (_, h) => {
                const period = h >= 12 ? "PM" : "AM";
                const h12 = h === 0 ? 12 : h > 12 ? h - 12 : h;
                return (
                  <option key={h} value={h}>
                    {`${h12}:00 ${period}`}
                  </option>
                );
              })}
            </select>
          </div>
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

      {/* Custom Prompt (task 3.2) */}
      <div className="border-t border-ds-gray-400 pt-3">
        <button
          type="button"
          onClick={() => setPromptOpen(!promptOpen)}
          className="flex items-center gap-1.5 text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
        >
          {promptOpen ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
          Custom Prompt
          {promptSaveState === "saving" && (
            <Loader2 size={11} className="animate-spin text-ds-gray-700 ml-1" />
          )}
          {promptSaveState === "saved" && (
            <Check size={11} className="text-green-700 ml-1" />
          )}
        </button>
        {promptOpen && (
          <textarea
            value={localPrompt}
            onChange={(e) => setLocalPrompt(e.target.value)}
            onBlur={() => void handlePromptBlur()}
            placeholder="Describe what the briefing should emphasize (e.g., today's meetings, urgent tasks)..."
            rows={3}
            className="mt-2 w-full px-3 py-2 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded-lg focus:outline-hidden focus:border-ds-gray-500 placeholder:text-ds-gray-700 resize-y"
          />
        )}
      </div>

      {genError && (
        <p className="text-[11px] text-red-700">{genError}</p>
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
        {/* View All Briefings (task 3.4) */}
        <Link
          href="/briefing"
          className="flex items-center gap-1 text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 transition-colors hover:underline underline-offset-2"
        >
          <ExternalLink size={12} />
          View All Briefings
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
  onRefetch,
}: {
  reminders: AutomationReminder[];
  actionPending: string | null;
  onCancel: (id: string) => void;
  onRefetch: () => void;
}) {
  const [formOpen, setFormOpen] = useState(false);
  const [formMessage, setFormMessage] = useState("");
  const [formDueAt, setFormDueAt] = useState("");
  const [formChannel, setFormChannel] = useState("dashboard");
  const [formSubmitting, setFormSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const handleFormSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setFormError(null);

    // Client validation
    if (!formMessage.trim()) {
      setFormError("Message is required");
      return;
    }
    if (formMessage.length > 500) {
      setFormError("Message must be 500 characters or fewer");
      return;
    }
    if (!formDueAt) {
      setFormError("Due date is required");
      return;
    }
    const dueDate = new Date(formDueAt);
    if (dueDate <= new Date()) {
      setFormError("Due date must be in the future");
      return;
    }

    setFormSubmitting(true);
    try {
      // No tRPC createReminder procedure exists yet -- using apiFetch
      const res = await apiFetch("/api/automations/reminders", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          message: formMessage.trim(),
          due_at: dueDate.toISOString(),
          channel: formChannel.trim() || "dashboard",
        }),
      });
      if (!res.ok) {
        const body = await res.text();
        throw new Error(body || `HTTP ${res.status}`);
      }
      // Reset form and close
      setFormMessage("");
      setFormDueAt("");
      setFormChannel("dashboard");
      setFormOpen(false);
      onRefetch();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : "Failed to create reminder");
    } finally {
      setFormSubmitting(false);
    }
  };

  return (
    <div>
      {/* Create Reminder button + form (task 3.6) */}
      <div className="mb-3 flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <p className="text-copy-13 text-ds-gray-700">
            Created via Telegram (&ldquo;remind me to...&rdquo;) or the API. One-time alerts delivered to a channel.
          </p>
          <button
            type="button"
            onClick={() => setFormOpen(!formOpen)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors shrink-0"
          >
            <Plus size={13} />
            Create Reminder
          </button>
        </div>

        {formOpen && (
          <form onSubmit={(e) => void handleFormSubmit(e)} className="surface-inset p-3 rounded-lg flex flex-col gap-3">
            {/* Message */}
            <div className="flex flex-col gap-1">
              <label htmlFor="reminder-message" className="text-label-12 text-ds-gray-700">
                Message
              </label>
              <textarea
                id="reminder-message"
                value={formMessage}
                onChange={(e) => setFormMessage(e.target.value)}
                maxLength={500}
                rows={2}
                required
                placeholder="What do you want to be reminded about?"
                className="w-full px-3 py-2 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded-lg focus:outline-hidden focus:border-ds-gray-500 placeholder:text-ds-gray-700 resize-y"
              />
              <span className="text-[11px] text-ds-gray-700 text-right">
                {formMessage.length}/500
              </span>
            </div>

            {/* Due date + Channel */}
            <div className="flex flex-col gap-3 sm:flex-row sm:gap-4">
              <div className="flex flex-col gap-1 flex-1">
                <label htmlFor="reminder-due" className="text-label-12 text-ds-gray-700">
                  Due at
                </label>
                <input
                  id="reminder-due"
                  type="datetime-local"
                  value={formDueAt}
                  onChange={(e) => setFormDueAt(e.target.value)}
                  required
                  className="px-3 py-2 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded-lg focus:outline-hidden focus:border-ds-gray-500"
                />
              </div>
              <div className="flex flex-col gap-1 flex-1">
                <label htmlFor="reminder-channel" className="text-label-12 text-ds-gray-700">
                  Channel (optional)
                </label>
                <input
                  id="reminder-channel"
                  type="text"
                  value={formChannel}
                  onChange={(e) => setFormChannel(e.target.value)}
                  placeholder="dashboard"
                  className="px-3 py-2 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded-lg focus:outline-hidden focus:border-ds-gray-500 placeholder:text-ds-gray-700"
                />
              </div>
            </div>

            {formError && (
              <p className="text-[11px] text-red-700">{formError}</p>
            )}

            <div className="flex items-center gap-2">
              <button
                type="submit"
                disabled={formSubmitting}
                className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
              >
                {formSubmitting ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Plus size={13} />
                )}
                {formSubmitting ? "Creating..." : "Create"}
              </button>
              <button
                type="button"
                onClick={() => {
                  setFormOpen(false);
                  setFormError(null);
                }}
                className="px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-700 hover:text-ds-gray-1000 transition-colors"
              >
                Cancel
              </button>
            </div>
          </form>
        )}
      </div>
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
                      className={`inline-flex items-center px-1.5 py-0.5 rounded text-label-13 ${
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
                      className="inline-flex items-center gap-1 px-2 py-1 rounded text-copy-13 text-red-700 hover:bg-red-700/10 transition-colors disabled:opacity-50"
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
                      className={`inline-flex items-center px-1.5 py-0.5 rounded text-label-13 ${
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
                      className="inline-flex items-center gap-1 px-2 py-1 rounded text-copy-13 text-ds-gray-900 hover:bg-ds-gray-alpha-200 transition-colors disabled:opacity-50"
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
  const trpc = useTRPC();
  const queryClient = useQueryClient();
  const [actionPending, setActionPending] = useState<string | null>(null);
  const [scheduledTab, setScheduledTab] = useState<ScheduledTab>("reminders");

  const automationsQuery = useQuery(
    trpc.automation.getAll.queryOptions(undefined, { refetchInterval: 30_000 }),
  );
  const data = (automationsQuery.data as AutomationsGetResponse | undefined) ?? null;
  const loading = automationsQuery.isLoading;
  const error = automationsQuery.error?.message ?? null;

  const fetchData = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: trpc.automation.getAll.queryKey() });
  }, [queryClient]);

  // ── Quick actions ────────────────────────────────────────────────────────

  const cancelReminderMut = useMutation(
    trpc.automation.updateReminder.mutationOptions({
      onSuccess: () => fetchData(),
    }),
  );

  const toggleScheduleMut = useMutation(
    trpc.automation.updateSchedule.mutationOptions({
      onSuccess: () => fetchData(),
    }),
  );

  const cancelReminder = useCallback(
    async (id: string) => {
      setActionPending(id);
      try {
        await cancelReminderMut.mutateAsync({ id, action: "cancel" });
      } catch {
        // Silently fail -- next refresh will show real state
      } finally {
        setActionPending(null);
      }
    },
    [cancelReminderMut],
  );

  const toggleSchedule = useCallback(
    async (id: string, enabled: boolean) => {
      setActionPending(id);
      try {
        await toggleScheduleMut.mutateAsync({ id, enabled });
      } catch {
        // Silently fail
      } finally {
        setActionPending(null);
      }
    },
    [toggleScheduleMut],
  );

  const patchWatcher = useCallback(
    async (patch: Partial<AutomationWatcher>) => {
      // No dedicated patchWatcher tRPC procedure -- use apiFetch
      const res = await apiFetch("/api/automations/watcher", {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(patch),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      fetchData();
    },
    [fetchData],
  );

  // -- Settings (shared by WatcherCard + BriefingCard) --
  const settingsQuery = useQuery(trpc.automation.getSettings.queryOptions());
  const settings = (settingsQuery.data as Record<string, string> | undefined)?.settings
    ? ((settingsQuery.data as { settings: Record<string, string> }).settings)
    : ({} as Record<string, string>);

  const updateSettingsMut = useMutation(
    trpc.automation.updateSettings.mutationOptions({
      onSuccess: () => {
        void queryClient.invalidateQueries({ queryKey: trpc.automation.getSettings.queryKey() });
      },
    }),
  );

  const saveSetting = useCallback(
    async (key: string, value: string) => {
      await updateSettingsMut.mutateAsync({ key, value });
    },
    [],
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
            <div className="flex flex-col gap-2">
              <WatcherCard
                watcher={data.watcher}
                onUpdate={patchWatcher}
                promptValue={settings["watcher_prompt"] ?? ""}
                onPromptSave={(value) => saveSetting("watcher_prompt", value)}
              />
              {/* View Watcher Sessions (task 3.4) */}
              <Link
                href="/sessions?command=proactive-followup"
                className="flex items-center gap-1 text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 transition-colors hover:underline underline-offset-2"
              >
                <ExternalLink size={12} />
                View Watcher Sessions
              </Link>
            </div>
            <div className="flex flex-col gap-2">
              <BriefingCard
                lastGeneratedAt={data.briefing.last_generated_at}
                nextGeneration={data.briefing.next_generation}
                contentPreview={data.briefing.content_preview}
                briefingHour={data.briefing.briefing_hour}
                onGenerated={() => void fetchData()}
                promptValue={settings["briefing_prompt"] ?? ""}
                onPromptSave={(value) => saveSetting("briefing_prompt", value)}
                onHourSave={(hour) => saveSetting("briefing_hour", String(hour))}
              />
              {/* View Briefing Sessions (task 3.4) */}
              <Link
                href="/sessions?command=morning-briefing"
                className="flex items-center gap-1 text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 transition-colors hover:underline underline-offset-2"
              >
                <ExternalLink size={12} />
                View Briefing Sessions
              </Link>
            </div>
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
                  className={`flex items-center gap-2 px-3 py-1.5 rounded text-label-14 transition-colors ${
                    scheduledTab === t.key
                      ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                      : "text-ds-gray-900 hover:text-ds-gray-1000"
                  }`}
                >
                  {t.icon}
                  <span>{t.label}</span>
                  <span className="text-copy-13 font-mono opacity-70">{t.count}</span>
                </button>
              ))}
            </div>

            {scheduledTab === "reminders" ? (
              <RemindersTab
                reminders={data.reminders}
                actionPending={actionPending}
                onCancel={(id) => void cancelReminder(id)}
                onRefetch={() => void fetchData()}
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
