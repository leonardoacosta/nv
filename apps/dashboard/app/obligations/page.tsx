"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  CheckSquare,
  RefreshCw,
  Clock,
  Check,
  X,
  RotateCcw,
  ChevronDown,
  ChevronUp,
  Play,
  Radio,
  FolderOpen,
} from "lucide-react";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import ObligationSummaryBar from "@/components/ObligationSummaryBar";
import ActivityFeed from "@/components/ActivityFeed";
import type {
  DaemonObligation,
  ObligationNote,
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
// Deadline proximity helpers
// ---------------------------------------------------------------------------

/** Default threshold in hours; overridden by daemon config if available */
const DEFAULT_APPROACHING_DEADLINE_HOURS = 24;

type DeadlineProximity = "overdue" | "approaching" | "normal";

function getDeadlineProximity(
  deadline: string | null,
  thresholdHours: number,
): DeadlineProximity {
  if (!deadline) return "normal";
  const now = Date.now();
  const deadlineMs = new Date(deadline).getTime();
  if (deadlineMs <= now) return "overdue";
  const hoursUntil = (deadlineMs - now) / (1000 * 60 * 60);
  if (hoursUntil <= thresholdHours) return "approaching";
  return "normal";
}

const DEADLINE_RING: Record<DeadlineProximity, string> = {
  overdue: "ring-2 ring-red-700/60 border-red-700/40",
  approaching: "ring-2 ring-amber-500/40 border-amber-500/30",
  normal: "",
};

// ---------------------------------------------------------------------------
// Rich obligation card
// ---------------------------------------------------------------------------

interface ObligationCardProps {
  obligation: DaemonObligation;
  onRefresh: () => void;
  approachingDeadlineHours?: number;
  /** Whether details section is expanded */
  isExpanded: boolean;
  /** Toggle expand callback */
  onToggleExpand: () => void;
}

function ObligationCard({
  obligation,
  onRefresh,
  approachingDeadlineHours = DEFAULT_APPROACHING_DEADLINE_HOURS,
  isExpanded,
  onToggleExpand,
}: ObligationCardProps) {
  const [notesExpanded, setNotesExpanded] = useState(false);
  const [actionPending, setActionPending] = useState(false);

  const priorityBar = PRIORITY_BAR[obligation.priority] ?? PRIORITY_BAR[2];
  const priorityText = PRIORITY_TEXT[obligation.priority] ?? PRIORITY_TEXT[2];
  const statusBadge = STATUS_BADGE[obligation.status] ?? STATUS_BADGE["open"];
  const statusLabel = STATUS_LABEL[obligation.status] ?? obligation.status;

  const deadlineProximity = getDeadlineProximity(
    obligation.deadline,
    approachingDeadlineHours,
  );
  const deadlineRing = DEADLINE_RING[deadlineProximity];

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
      className={`surface-card relative overflow-hidden scroll-mt-4 ${deadlineRing}`}
    >
      {/* Priority bar */}
      <div className={`absolute left-0 top-0 bottom-0 w-1 ${priorityBar}`} aria-hidden="true" />

      <div className="pl-4 pr-4 pt-4 pb-3 space-y-3">
        {/* Header row — always visible, clickable to toggle details */}
        <button
          type="button"
          onClick={onToggleExpand}
          className="flex items-start gap-2 flex-wrap w-full text-left"
        >
          <span className={`text-xs font-mono font-bold ${priorityText} shrink-0`}>
            P{obligation.priority}
          </span>
          <span className="text-sm font-semibold text-ds-gray-1000 flex-1 min-w-0">
            {obligation.detected_action}
          </span>
          <div className="flex items-center gap-2 shrink-0 flex-wrap">
            {/* Deadline proximity indicator */}
            {deadlineProximity === "overdue" && (
              <span className="text-[10px] font-mono font-bold text-red-500 uppercase px-1.5 py-0.5 rounded bg-red-700/20">
                Overdue
              </span>
            )}
            {deadlineProximity === "approaching" && (
              <span className="text-[10px] font-mono font-bold text-amber-500 uppercase px-1.5 py-0.5 rounded bg-amber-500/20">
                Due Soon
              </span>
            )}
            <span className={`text-xs px-2 py-0.5 rounded font-mono ${statusBadge}`}>
              {statusLabel}
            </span>
            <OwnerBadge owner={obligation.owner} />
            <ChevronDown
              size={14}
              className={`text-ds-gray-700 transition-transform duration-200 ${isExpanded ? "rotate-180" : ""}`}
            />
          </div>
        </button>

        {/* Compact meta — always visible */}
        <div className="flex items-center gap-3 flex-wrap text-xs text-ds-gray-900">
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
          {obligation.deadline && (
            <span
              className={`flex items-center gap-1 font-mono ${
                deadlineProximity === "overdue"
                  ? "text-red-500"
                  : deadlineProximity === "approaching"
                    ? "text-amber-500"
                    : ""
              }`}
              suppressHydrationWarning
            >
              <Clock size={11} />
              due {new Date(obligation.deadline).toLocaleDateString()}
            </span>
          )}
          {obligation.attempt_count > 0 && (
            <span className="flex items-center gap-1 font-mono text-ds-gray-700">
              {obligation.attempt_count} attempt{obligation.attempt_count !== 1 ? "s" : ""}
            </span>
          )}
          {/* Inline action buttons in header area */}
          <div className="ml-auto">
            <ActionButtons
              status={obligation.status}
              pending={actionPending}
              onStart={handleStart}
              onPatch={patchStatus}
            />
          </div>
        </div>

        {/* Collapsible details section — uses height-reveal CSS animation */}
        <div className={`height-reveal ${isExpanded ? "open" : ""}`}>
          <div className="space-y-3">
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
                      {notesExpanded &&
                        olderNotes.map((n) => (
                          <NoteRow key={n.id} note={n} expanded={false} />
                        ))}
                      <button
                        type="button"
                        onClick={() => setNotesExpanded((v) => !v)}
                        className="flex items-center gap-1 text-xs text-ds-gray-700 hover:text-ds-gray-1000 transition-colors"
                      >
                        {notesExpanded ? (
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
          </div>
        </div>
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
// Tooltip wrapper
// ---------------------------------------------------------------------------

function Tooltip({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="relative group/tooltip">
      {children}
      <span className="pointer-events-none absolute -top-8 left-1/2 -translate-x-1/2 whitespace-nowrap rounded bg-ds-gray-200 border border-ds-gray-400 px-2 py-1 text-[11px] text-ds-gray-1000 opacity-0 transition-opacity group-hover/tooltip:opacity-100">
        {label}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Action buttons — contextual per obligation status
// ---------------------------------------------------------------------------

interface ActionDef {
  icon: React.ReactNode;
  tooltip: string;
  onClick: () => void;
  variant: "default" | "success" | "danger";
}

function getActionsForStatus(
  status: string,
  pending: boolean,
  onStart: () => void,
  onPatch: (status: string) => void,
): ActionDef[] {
  switch (status) {
    case "open":
      return [
        {
          icon: <Play size={13} />,
          tooltip: "Start",
          onClick: onStart,
          variant: "default",
        },
        {
          icon: <Check size={13} />,
          tooltip: "Mark Done",
          onClick: () => onPatch("done"),
          variant: "success",
        },
        {
          icon: <X size={13} />,
          tooltip: "Cancel",
          onClick: () => onPatch("dismissed"),
          variant: "danger",
        },
      ];
    case "in_progress":
      return [
        {
          icon: <Check size={13} />,
          tooltip: "Mark Done",
          onClick: () => onPatch("done"),
          variant: "success",
        },
        {
          icon: <X size={13} />,
          tooltip: "Cancel",
          onClick: () => onPatch("dismissed"),
          variant: "danger",
        },
      ];
    case "proposed_done":
      return [
        {
          icon: <Check size={13} />,
          tooltip: "Confirm Done",
          onClick: () => onPatch("done"),
          variant: "success",
        },
        {
          icon: <RotateCcw size={13} />,
          tooltip: "Reopen",
          onClick: () => onPatch("open"),
          variant: "default",
        },
      ];
    case "done":
      return [
        {
          icon: <RotateCcw size={13} />,
          tooltip: "Reopen",
          onClick: () => onPatch("open"),
          variant: "default",
        },
      ];
    default:
      return [];
  }
}

const ACTION_VARIANT_CLASSES: Record<string, string> = {
  default:
    "bg-ds-gray-alpha-200 text-ds-gray-1000 hover:bg-ds-gray-alpha-400 border-ds-gray-400",
  success:
    "bg-green-700/20 text-green-600 hover:bg-green-700/30 border-green-700/30",
  danger:
    "bg-ds-gray-alpha-100 text-ds-gray-900 hover:bg-ds-gray-alpha-200 border-ds-gray-400",
};

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
  const actions = getActionsForStatus(status, pending, onStart, onPatch);
  if (actions.length === 0) return null;

  return (
    <div className="flex gap-1.5">
      {actions.map((action) => (
        <Tooltip key={action.tooltip} label={action.tooltip}>
          <button
            type="button"
            onClick={action.onClick}
            disabled={pending}
            className={`flex items-center justify-center w-7 h-7 rounded-lg border transition-colors disabled:opacity-50 ${ACTION_VARIANT_CLASSES[action.variant]}`}
            aria-label={action.tooltip}
          >
            {action.icon}
          </button>
        </Tooltip>
      ))}
    </div>
  );
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
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<TabKey>("open");
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [initialExpandDone, setInitialExpandDone] = useState(false);
  const [approachingDeadlineHours, setApproachingDeadlineHours] = useState(
    DEFAULT_APPROACHING_DEADLINE_HOURS,
  );
  const listRef = useRef<HTMLDivElement>(null);

  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const fetchObligations = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [oblRes, configRes] = await Promise.all([
        fetch("/api/obligations"),
        fetch("/api/config"),
      ]);
      if (!oblRes.ok) throw new Error(`HTTP ${oblRes.status}`);
      const data = (await oblRes.json()) as ObligationsGetResponse;
      setObligations(data.obligations ?? []);
      if (configRes.ok) {
        const configData = (await configRes.json()) as Record<string, unknown>;
        const hours = configData.approaching_deadline_hours;
        if (typeof hours === "number" && hours > 0) {
          setApproachingDeadlineHours(hours);
        }
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

  // Expand first item by default on initial load
  useEffect(() => {
    if (!initialExpandDone && obligations.length > 0) {
      setExpandedIds(new Set([obligations[0]!.id]));
      setInitialExpandDone(true);
    }
  }, [obligations, initialExpandDone]);

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

      {/* Compact summary bar — replaces the 5 stat cards */}
      {obligations.length > 0 && (
        <div className="section-stagger-1">
          <ObligationSummaryBar obligations={obligations} />
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400 w-fit section-stagger-2">
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
      <div className="flex flex-col lg:flex-row gap-6 section-stagger-3">
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
            <div key="open" className="animate-crossfade-in space-y-8">
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
                        <ObligationCard
                          obligation={o}
                          onRefresh={fetchObligations}
                          approachingDeadlineHours={approachingDeadlineHours}
                          isExpanded={expandedIds.has(o.id)}
                          onToggleExpand={() => toggleExpand(o.id)}
                        />
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
                        <ObligationCard
                          obligation={o}
                          onRefresh={fetchObligations}
                          approachingDeadlineHours={approachingDeadlineHours}
                          isExpanded={expandedIds.has(o.id)}
                          onToggleExpand={() => toggleExpand(o.id)}
                        />
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
                        approachingDeadlineHours={approachingDeadlineHours}
                        isExpanded={expandedIds.has(o.id)}
                        onToggleExpand={() => toggleExpand(o.id)}
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
            <div key="history" className="animate-crossfade-in space-y-3">
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
                    <ObligationCard
                      obligation={o}
                      onRefresh={fetchObligations}
                      approachingDeadlineHours={approachingDeadlineHours}
                      isExpanded={expandedIds.has(o.id)}
                      onToggleExpand={() => toggleExpand(o.id)}
                    />
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
