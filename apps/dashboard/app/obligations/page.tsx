"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  CheckSquare,
  RefreshCw,
  Clock,
  CheckCircle,
  XCircle,
  ChevronDown,
  ChevronUp,
  Play,
  Radio,
  FolderOpen,
  AlertTriangle,
  ListTodo,
  Hourglass,
  CalendarCheck,
} from "lucide-react";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import StatCard from "@/components/layout/StatCard";
import ActivityFeed from "@/components/ActivityFeed";
import type {
  DaemonObligation,
  ObligationNote,
  ObligationStats,
  ObligationsGetResponse,
} from "@/types/api";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function relativeTime(ts: string): string {
  const diff = Date.now() - new Date(ts).getTime();
  const s = Math.floor(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  return text.slice(0, max) + "…";
}

// ---------------------------------------------------------------------------
// Status badge
// ---------------------------------------------------------------------------

const STATUS_BADGE: Record<string, string> = {
  open: "bg-ds-gray-alpha-200 text-ds-gray-1000",
  in_progress: "bg-amber-500/20 text-amber-500",
  proposed_done: "bg-blue-500/20 text-blue-400",
  done: "bg-green-700/20 text-green-600",
  dismissed: "bg-ds-gray-alpha-100 text-ds-gray-700",
};

const STATUS_LABEL: Record<string, string> = {
  open: "Open",
  in_progress: "In Progress",
  proposed_done: "Proposed Done",
  done: "Done",
  dismissed: "Dismissed",
};

// ---------------------------------------------------------------------------
// Priority config
// ---------------------------------------------------------------------------

const PRIORITY_BAR: Record<number, string> = {
  0: "bg-[#EF4444]",
  1: "bg-[#F97316]",
  2: "bg-ds-gray-700",
  3: "bg-[#6B7280]",
  4: "bg-[#374151]",
};

const PRIORITY_TEXT: Record<number, string> = {
  0: "text-[#EF4444]",
  1: "text-[#F97316]",
  2: "text-ds-gray-1000",
  3: "text-[#6B7280]",
  4: "text-[#374151]",
};

// ---------------------------------------------------------------------------
// Owner badge
// ---------------------------------------------------------------------------

function OwnerBadge({ owner }: { owner: string }) {
  if (owner === "nova") {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-mono bg-ds-gray-700/30 text-ds-gray-1000">
        <span className="font-bold">N</span> Nova
      </span>
    );
  }
  if (owner === "leo") {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-mono bg-red-700/20 text-red-600">
        <span className="font-bold">L</span> Leo
      </span>
    );
  }
  return (
    <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-mono bg-ds-gray-alpha-100 text-ds-gray-900">
      {owner}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Execution history timeline
// ---------------------------------------------------------------------------

function NoteRow({ note, expanded }: { note: ObligationNote; expanded: boolean }) {
  const [open, setOpen] = useState(expanded);
  return (
    <div className="flex gap-2 text-xs">
      <div className="w-1 bg-ds-gray-400 rounded-full shrink-0 self-stretch mt-1" />
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-mono text-ds-gray-700" suppressHydrationWarning>
            {relativeTime(note.created_at)}
          </span>
          <span className="text-ds-gray-900 font-mono uppercase text-[10px]">
            {note.note_type}
          </span>
          {note.content.length > 120 && (
            <button
              type="button"
              onClick={() => setOpen((v) => !v)}
              className="text-ds-gray-700 hover:text-ds-gray-1000 ml-auto"
            >
              {open ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
            </button>
          )}
        </div>
        <p
          className={`mt-0.5 text-ds-gray-900 leading-snug ${!open && note.content.length > 120 ? "line-clamp-1" : ""}`}
        >
          {note.content}
        </p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Rich obligation card
// ---------------------------------------------------------------------------

interface ObligationCardProps {
  obligation: DaemonObligation;
  onRefresh: () => void;
}

function ObligationCard({ obligation, onRefresh }: ObligationCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [actionPending, setActionPending] = useState(false);

  const priorityBar = PRIORITY_BAR[obligation.priority] ?? PRIORITY_BAR[2];
  const priorityText = PRIORITY_TEXT[obligation.priority] ?? PRIORITY_TEXT[2];
  const statusBadge = STATUS_BADGE[obligation.status] ?? STATUS_BADGE["open"];
  const statusLabel = STATUS_LABEL[obligation.status] ?? obligation.status;

  const notes = obligation.notes ?? [];
  const mostRecentNote = notes[0];
  const olderNotes = notes.slice(1);

  async function patchStatus(status: string) {
    setActionPending(true);
    try {
      const res = await fetch(`/api/obligations/${obligation.id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ status }),
      });
      if (res.ok) onRefresh();
    } catch {
      // ignore — user can retry
    } finally {
      setActionPending(false);
    }
  }

  async function handleStart() {
    setActionPending(true);
    try {
      const res = await fetch(`/api/obligations/${obligation.id}/execute`, {
        method: "POST",
      });
      if (res.ok) onRefresh();
    } catch {
      // ignore
    } finally {
      setActionPending(false);
    }
  }

  return (
    <div
      id={`obligation-${obligation.id}`}
      className="surface-card relative overflow-hidden scroll-mt-4"
    >
      {/* Priority bar */}
      <div className={`absolute left-0 top-0 bottom-0 w-1 ${priorityBar}`} aria-hidden="true" />

      <div className="pl-4 pr-4 pt-4 pb-3 space-y-3">
        {/* Header row */}
        <div className="flex items-start gap-2 flex-wrap">
          <span className={`text-xs font-mono font-bold ${priorityText} shrink-0`}>
            P{obligation.priority}
          </span>
          <span className="text-sm font-semibold text-ds-gray-1000 flex-1 min-w-0">
            {obligation.detected_action}
          </span>
          <div className="flex items-center gap-2 shrink-0 flex-wrap">
            <span className={`text-xs px-2 py-0.5 rounded font-mono ${statusBadge}`}>
              {statusLabel}
            </span>
            <OwnerBadge owner={obligation.owner} />
          </div>
        </div>

        {/* Context: source channel + message */}
        {(obligation.source_channel || obligation.source_message) && (
          <SourceContext
            channel={obligation.source_channel}
            message={obligation.source_message}
          />
        )}

        {/* Execution history */}
        {notes.length > 0 && (
          <div className="space-y-1.5">
            <span className="text-label-12 text-ds-gray-900 uppercase tracking-wide">
              Execution History
            </span>
            <div className="space-y-2 pl-1">
              {mostRecentNote && (
                <NoteRow key={mostRecentNote.id} note={mostRecentNote} expanded />
              )}
              {olderNotes.length > 0 && (
                <>
                  {expanded &&
                    olderNotes.map((n) => (
                      <NoteRow key={n.id} note={n} expanded={false} />
                    ))}
                  <button
                    type="button"
                    onClick={() => setExpanded((v) => !v)}
                    className="flex items-center gap-1 text-xs text-ds-gray-700 hover:text-ds-gray-1000 transition-colors"
                  >
                    {expanded ? (
                      <>
                        <ChevronUp size={11} /> Hide {olderNotes.length} older
                      </>
                    ) : (
                      <>
                        <ChevronDown size={11} /> Show {olderNotes.length} older
                      </>
                    )}
                  </button>
                </>
              )}
            </div>
          </div>
        )}

        {/* Meta row */}
        <div className="flex items-center gap-4 flex-wrap text-xs text-ds-gray-900">
          {obligation.project_code && (
            <span className="flex items-center gap-1 font-mono">
              <FolderOpen size={11} />
              {obligation.project_code}
            </span>
          )}
          <span className="flex items-center gap-1 font-mono">
            <Clock size={11} />
            <span suppressHydrationWarning>{relativeTime(obligation.created_at)}</span>
          </span>
          {obligation.attempt_count > 0 && (
            <span className="flex items-center gap-1 font-mono text-ds-gray-700">
              {obligation.attempt_count} attempt{obligation.attempt_count !== 1 ? "s" : ""}
            </span>
          )}
        </div>

        {/* Action buttons */}
        <ActionButtons
          status={obligation.status}
          pending={actionPending}
          onStart={handleStart}
          onPatch={patchStatus}
        />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Source context
// ---------------------------------------------------------------------------

function SourceContext({
  channel,
  message,
}: {
  channel: string;
  message: string | null;
}) {
  const [expanded, setExpanded] = useState(false);
  const truncated = message ? truncate(message, 200) : null;
  const needsExpand = message && message.length > 200;

  return (
    <div className="flex gap-2 text-xs text-ds-gray-900 bg-ds-gray-alpha-100 rounded-lg px-3 py-2">
      <Radio size={12} className="shrink-0 mt-0.5 text-ds-gray-700" />
      <div className="flex-1 min-w-0">
        <span className="font-mono text-ds-gray-700 uppercase text-[10px]">{channel}</span>
        {message && (
          <p className="mt-0.5 text-ds-gray-1000 leading-snug">
            {expanded ? message : truncated}
            {needsExpand && (
              <button
                type="button"
                onClick={() => setExpanded((v) => !v)}
                className="ml-1 text-ds-gray-700 hover:text-ds-gray-1000 underline"
              >
                {expanded ? "Show less" : "Show more"}
              </button>
            )}
          </p>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Action buttons
// ---------------------------------------------------------------------------

function ActionButtons({
  status,
  pending,
  onStart,
  onPatch,
}: {
  status: string;
  pending: boolean;
  onStart: () => void;
  onPatch: (status: string) => void;
}) {
  if (status === "open") {
    return (
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onStart}
          disabled={pending}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-ds-gray-alpha-200 text-ds-gray-1000 hover:bg-ds-gray-alpha-300 border border-ds-gray-400 transition-colors disabled:opacity-50"
        >
          <Play size={11} />
          {pending ? "Starting…" : "Start"}
        </button>
      </div>
    );
  }

  if (status === "in_progress") {
    return (
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onPatch("dismissed")}
          disabled={pending}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-ds-gray-alpha-100 text-ds-gray-900 hover:bg-ds-gray-alpha-200 border border-ds-gray-400 transition-colors disabled:opacity-50"
        >
          <XCircle size={11} />
          {pending ? "Cancelling…" : "Cancel"}
        </button>
      </div>
    );
  }

  if (status === "proposed_done") {
    return (
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onPatch("done")}
          disabled={pending}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-green-700/20 text-green-600 hover:bg-green-700/30 border border-green-700/30 transition-colors disabled:opacity-50"
        >
          <CheckCircle size={11} />
          {pending ? "Confirming…" : "Confirm Done"}
        </button>
        <button
          type="button"
          onClick={() => onPatch("open")}
          disabled={pending}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-ds-gray-alpha-100 text-ds-gray-900 hover:bg-ds-gray-alpha-200 border border-ds-gray-400 transition-colors disabled:opacity-50"
        >
          Reopen
        </button>
      </div>
    );
  }

  if (status === "done") {
    return (
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onPatch("open")}
          disabled={pending}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-ds-gray-alpha-100 text-ds-gray-900 hover:bg-ds-gray-alpha-200 border border-ds-gray-400 transition-colors disabled:opacity-50"
        >
          Reopen
        </button>
      </div>
    );
  }

  return null;
}

// ---------------------------------------------------------------------------
// Section header
// ---------------------------------------------------------------------------

function SectionHeading({
  label,
  count,
  colorClass,
  initial,
}: {
  label: string;
  count: number;
  colorClass: string;
  initial: string;
}) {
  return (
    <div className="flex items-center gap-2 mb-3">
      <div
        className={`w-6 h-6 rounded flex items-center justify-center ${colorClass}`}
      >
        <span className="text-xs font-bold font-mono">{initial}</span>
      </div>
      <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide">
        {label}
      </h2>
      <span className="text-xs font-mono text-ds-gray-900">{count}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

type TabKey = "open" | "history";

export default function ObligationsPage() {
  const [obligations, setObligations] = useState<DaemonObligation[]>([]);
  const [stats, setStats] = useState<ObligationStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<TabKey>("open");
  const listRef = useRef<HTMLDivElement>(null);

  const fetchObligations = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [oblRes, statsRes] = await Promise.all([
        fetch("/api/obligations"),
        fetch("/api/obligations/stats"),
      ]);
      if (!oblRes.ok) throw new Error(`HTTP ${oblRes.status}`);
      const data = (await oblRes.json()) as ObligationsGetResponse;
      setObligations(data.obligations ?? []);
      if (statsRes.ok) {
        const statsData = (await statsRes.json()) as ObligationStats;
        setStats(statsData);
      }
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load obligations",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchObligations();
  }, [fetchObligations]);

  const scrollToObligation = useCallback((id: string) => {
    const el = document.getElementById(`obligation-${id}`);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "start" });
      el.classList.add("ring-2", "ring-ds-gray-700");
      setTimeout(() => el.classList.remove("ring-2", "ring-ds-gray-700"), 2000);
    }
  }, []);

  const sortByPriority = (items: DaemonObligation[]) =>
    [...items].sort((a, b) => a.priority - b.priority);

  const activeStatuses = ["open", "in_progress", "proposed_done"];
  const open = obligations.filter((o) => activeStatuses.includes(o.status));
  const history = obligations.filter(
    (o) => o.status === "done" || o.status === "dismissed",
  );

  const nova = sortByPriority(open.filter((o) => o.owner === "nova"));
  const leo = sortByPriority(open.filter((o) => o.owner === "leo"));
  const other = sortByPriority(
    open.filter((o) => o.owner !== "nova" && o.owner !== "leo"),
  );

  return (
    <div className="p-8 space-y-6 max-w-7xl animate-fade-in-up">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-heading-24 text-ds-gray-1000">Obligations</h1>
          <p className="mt-1 text-copy-14 text-ds-gray-900">
            Active tasks and commitments
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchObligations()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Stats bar */}
      {stats && (
        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3">
          <StatCard
            icon={<ListTodo size={16} aria-hidden="true" />}
            label="Open (Nova)"
            value={stats.open_nova}
            variant="default"
          />
          <StatCard
            icon={<Hourglass size={16} aria-hidden="true" />}
            label="In Progress"
            value={stats.in_progress}
            variant="warning"
          />
          <StatCard
            icon={<CheckSquare size={16} aria-hidden="true" />}
            label="Proposed Done"
            value={stats.proposed_done}
            variant="success"
          />
          <StatCard
            icon={<CalendarCheck size={16} aria-hidden="true" />}
            label="Done Today"
            value={stats.done_today}
            variant="success"
          />
          <StatCard
            icon={<AlertTriangle size={16} aria-hidden="true" />}
            label="Open (Leo)"
            value={stats.open_leo}
            variant="default"
          />
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400 w-fit">
        {(["open", "history"] as TabKey[]).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={`flex items-center gap-2 px-4 py-1.5 rounded text-sm font-medium transition-colors ${
              tab === t
                ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                : "text-ds-gray-900 hover:text-ds-gray-1000"
            }`}
          >
            {t === "open" ? <CheckSquare size={14} /> : <Clock size={14} />}
            <span className="capitalize">
              {t === "open" ? "Active" : "History"}
            </span>
            <span className="text-xs font-mono opacity-70">
              {t === "open" ? open.length : history.length}
            </span>
          </button>
        ))}
      </div>

      {error && (
        <ErrorBanner
          message="Failed to load obligations"
          detail={error}
          onRetry={() => void fetchObligations()}
        />
      )}

      {/* Two-column layout: list (2/3) + activity feed (1/3) */}
      <div className="flex flex-col lg:flex-row gap-6">
        {/* Obligations list */}
        <div ref={listRef} className="flex-1 lg:w-2/3 min-w-0">
          {loading ? (
            <div className="space-y-2">
              {Array.from({ length: 5 }).map((_, i) => (
                <div
                  key={i}
                  className="h-28 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
                />
              ))}
            </div>
          ) : tab === "open" ? (
            <div className="space-y-8">
              {/* Nova */}
              <section>
                <SectionHeading
                  label="Nova"
                  count={nova.length}
                  initial="N"
                  colorClass="bg-ds-gray-700/30 text-ds-gray-1000"
                />
                {nova.length === 0 ? (
                  <p className="text-copy-14 text-ds-gray-900 py-4 pl-2">
                    No obligations assigned to Nova
                  </p>
                ) : (
                  <div className="space-y-3">
                    {nova.map((o, idx) => (
                      <div
                        key={o.id}
                        className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
                      >
                        <ObligationCard obligation={o} onRefresh={fetchObligations} />
                      </div>
                    ))}
                  </div>
                )}
              </section>

              {/* Leo */}
              <section>
                <SectionHeading
                  label="Leo"
                  count={leo.length}
                  initial="L"
                  colorClass="bg-red-700/30 text-red-700"
                />
                {leo.length === 0 ? (
                  <p className="text-copy-14 text-ds-gray-900 py-4 pl-2">
                    No obligations assigned to Leo
                  </p>
                ) : (
                  <div className="space-y-3">
                    {leo.map((o, idx) => (
                      <div
                        key={o.id}
                        className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
                      >
                        <ObligationCard obligation={o} onRefresh={fetchObligations} />
                      </div>
                    ))}
                  </div>
                )}
              </section>

              {/* Other */}
              {other.length > 0 && (
                <section>
                  <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide mb-3">
                    Other
                  </h2>
                  <div className="space-y-3">
                    {other.map((o) => (
                      <ObligationCard
                        key={o.id}
                        obligation={o}
                        onRefresh={fetchObligations}
                      />
                    ))}
                  </div>
                </section>
              )}

              {open.length === 0 && (
                <EmptyState
                  title="No active obligations"
                  description="All clear. New obligations will appear here when detected."
                  icon={<CheckSquare size={40} aria-hidden="true" />}
                />
              )}
            </div>
          ) : (
            // History tab
            <div className="space-y-3">
              {history.length === 0 ? (
                <EmptyState
                  title="No history yet"
                  description="Completed and dismissed obligations will appear here."
                  icon={<Clock size={40} aria-hidden="true" />}
                />
              ) : (
                sortByPriority(history).map((o, idx) => (
                  <div
                    key={o.id}
                    className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
                  >
                    <ObligationCard obligation={o} onRefresh={fetchObligations} />
                  </div>
                ))
              )}
            </div>
          )}
        </div>

        {/* Activity feed sidebar */}
        <div className="w-full lg:w-1/3 shrink-0">
          <ActivityFeed onObligationClick={scrollToObligation} />
        </div>
      </div>
    </div>
  );
}
