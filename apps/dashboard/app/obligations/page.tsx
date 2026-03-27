"use client";

import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useSearchParams } from "next/navigation";
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
  ShieldAlert,
  AlertTriangle,
  CheckCircle,
  XCircle,
  ChevronRight,
  FileText,
  GitPullRequest,
  Terminal,
  HelpCircle,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import QuerySkeleton from "@/components/layout/QuerySkeleton";
import ObligationSummaryBar from "@/components/ObligationSummaryBar";
// ActivityFeed retained in codebase for other pages; removed from Active tab layout
import ApprovalQueueItem from "@/components/approvals/ApprovalQueueItem";
import BatchActionBar from "@/components/approvals/BatchActionBar";
import QueueClearCelebration from "@/components/approvals/QueueClearCelebration";
import { useApprovalKeyboard } from "@/components/approvals/useApprovalKeyboard";
import type { Approval, ApprovalActionType } from "@/components/approvals/types";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";
import type {
  DaemonObligation,
  ObligationNote,
  ObligationsGetResponse,
} from "@/types/api";
import { apiFetch } from "@/lib/api-client";
import { useApiQuery, useApiMutation } from "@/lib/hooks/use-api-query";
import { queryKeys } from "@/lib/query-keys";
import KanbanBoard from "@/components/obligations/KanbanBoard";
import { useKanbanKeyboard } from "@/hooks/useKanbanKeyboard";

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
  return text.slice(0, max) + "\u2026";
}

// ---------------------------------------------------------------------------
// Status badge
// ---------------------------------------------------------------------------

const STATUS_BADGE: Record<string, string> = {
  open: "bg-ds-gray-alpha-200 text-ds-gray-1000",
  in_progress: "bg-amber-700/20 text-amber-700",
  proposed_done: "bg-blue-700/20 text-blue-700",
  done: "bg-green-700/20 text-green-700",
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
  0: "bg-red-700",
  1: "bg-amber-700",
  2: "bg-ds-gray-700",
  3: "bg-ds-gray-600",
  4: "bg-ds-gray-500",
};

const PRIORITY_TEXT: Record<number, string> = {
  0: "text-red-700",
  1: "text-amber-700",
  2: "text-ds-gray-1000",
  3: "text-ds-gray-700",
  4: "text-ds-gray-600",
};

// ---------------------------------------------------------------------------
// Owner badge
// ---------------------------------------------------------------------------

function OwnerBadge({ owner }: { owner: string }) {
  if (owner === "nova") {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-copy-13 font-mono bg-ds-gray-700/30 text-ds-gray-1000">
        <span className="font-bold">N</span> Nova
      </span>
    );
  }
  if (owner === "leo") {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-copy-13 font-mono bg-red-700/20 text-red-700">
        <span className="font-bold">L</span> Leo
      </span>
    );
  }
  return (
    <span className="inline-flex items-center px-2 py-0.5 rounded text-copy-13 font-mono bg-ds-gray-alpha-100 text-ds-gray-900">
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
    <div className="flex gap-2 text-copy-13">
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
      const res = await apiFetch(`/api/obligations/${obligation.id}`, {
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
      const res = await apiFetch(`/api/obligations/${obligation.id}/execute`, {
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
          <span className={`text-label-12 font-mono font-bold ${priorityText} shrink-0`}>
            P{obligation.priority}
          </span>
          <span className="text-copy-14 font-semibold text-ds-gray-1000 flex-1 min-w-0">
            {obligation.detected_action}
          </span>
          <div className="flex items-center gap-2 shrink-0 flex-wrap">
            {/* Deadline proximity indicator */}
            {deadlineProximity === "overdue" && (
              <span className="text-label-12 font-mono font-bold text-red-700 uppercase px-1.5 py-0.5 rounded bg-red-700/20">
                Overdue
              </span>
            )}
            {deadlineProximity === "approaching" && (
              <span className="text-label-12 font-mono font-bold text-amber-700 uppercase px-1.5 py-0.5 rounded bg-amber-700/20">
                Due Soon
              </span>
            )}
            <span className={`text-copy-13 px-2 py-0.5 rounded font-mono ${statusBadge}`}>
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
        <div className="flex items-center gap-3 flex-wrap text-copy-13 text-ds-gray-900">
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
                  ? "text-red-700"
                  : deadlineProximity === "approaching"
                    ? "text-amber-700"
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
                        className="flex items-center gap-1 text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 transition-colors"
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
    <div className="flex gap-2 text-copy-13 text-ds-gray-900 bg-ds-gray-alpha-100 rounded-lg px-3 py-2">
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
    "bg-green-700/20 text-green-700 hover:bg-green-700/30 border-green-700/30",
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
    <div className="flex items-center gap-2 mb-2">
      <div
        className={`w-6 h-6 rounded flex items-center justify-center ${colorClass}`}
      >
        <span className="text-label-12 font-bold font-mono">{initial}</span>
      </div>
      <h2 className="text-heading-16 text-ds-gray-1000 uppercase tracking-wide">
        {label}
      </h2>
      <span className="text-copy-13 font-mono text-ds-gray-900">{count}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Approvals helpers (merged from approvals page)
// ---------------------------------------------------------------------------

const ACTION_ICON: Record<ApprovalActionType, React.ElementType> = {
  file_write: FileText,
  file_delete: FileText,
  shell_exec: Terminal,
  git_push: GitPullRequest,
  api_call: GitPullRequest,
  other: HelpCircle,
};

const URGENCY_CONFIG: Record<
  Approval["urgency"],
  { label: string; dot: string; text: string }
> = {
  critical: {
    label: "Critical",
    dot: "bg-red-700",
    text: "text-red-700",
  },
  high: { label: "High", dot: "bg-amber-700", text: "text-amber-700" },
  medium: {
    label: "Medium",
    dot: "bg-amber-700",
    text: "text-amber-700",
  },
  low: { label: "Low", dot: "bg-ds-gray-600", text: "text-ds-gray-900" },
};

function priorityToUrgency(priority: number): Approval["urgency"] {
  if (priority === 0) return "critical";
  if (priority === 1) return "high";
  if (priority === 2) return "medium";
  return "low";
}

function mapObligationToApproval(o: DaemonObligation): Approval {
  const status: Approval["status"] =
    o.status === "done" ? "approved" : o.status === "dismissed" ? "dismissed" : "pending";
  return {
    id: o.id,
    title: o.detected_action,
    description: o.source_message ?? undefined,
    action_type: "other",
    project: o.project_code ?? undefined,
    proposed_changes: undefined,
    context: undefined,
    urgency: priorityToUrgency(o.priority),
    status,
    created_at: o.created_at,
  };
}

function approvalRelativeTime(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffH = Math.floor(diffMin / 60);
  if (diffH < 24) return `${diffH}h ago`;
  return `${Math.floor(diffH / 24)}d ago`;
}

// ---------------------------------------------------------------------------
// Approval Detail Panel
// ---------------------------------------------------------------------------

interface DetailPanelProps {
  approval: Approval;
  onApprove: (id: string) => Promise<void>;
  onDismiss: (id: string) => Promise<void>;
  approving: boolean;
  dismissing: boolean;
}

function DetailPanel({
  approval,
  onApprove,
  onDismiss,
  approving,
  dismissing,
}: DetailPanelProps) {
  const ActionIcon = ACTION_ICON[approval.action_type] ?? HelpCircle;
  const urg = URGENCY_CONFIG[approval.urgency];

  return (
    <div className="flex flex-col h-full">
      {/* Detail header */}
      <div className="px-4 py-3 border-b border-ds-gray-400 shrink-0">
        <div className="flex items-start gap-3">
          <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-ds-gray-alpha-100 border border-ds-gray-1000/30 shrink-0">
            <ActionIcon size={16} className="text-ds-gray-1000" />
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-heading-16 text-ds-gray-1000 leading-tight">
              {approval.title}
            </h2>
            <div className="flex items-center gap-3 mt-0.5 flex-wrap">
              {approval.project && (
                <span className="text-copy-13 font-mono text-ds-gray-900">
                  {approval.project}
                </span>
              )}
              <span className={`text-label-13 ${urg.text}`}>
                {urg.label} urgency
              </span>
              <span
                className="text-copy-13 text-ds-gray-900 flex items-center gap-1"
                suppressHydrationWarning
              >
                <Clock size={10} />
                {approvalRelativeTime(approval.created_at)}
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Scrollable body */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-4">
        {approval.description && (
          <section>
            <h3 className="text-label-12 text-ds-gray-900 mb-1">
              Description
            </h3>
            <p className="text-copy-13 text-ds-gray-1000 leading-relaxed">
              {approval.description}
            </p>
          </section>
        )}

        {approval.proposed_changes && (
          <section>
            <h3 className="text-label-12 text-ds-gray-900 mb-1">
              Proposed Changes
            </h3>
            <pre className="text-copy-13 text-ds-gray-1000 font-mono bg-ds-bg-100 border border-ds-gray-400 rounded-xl p-3 overflow-x-auto whitespace-pre-wrap">
              {approval.proposed_changes}
            </pre>
          </section>
        )}

        {approval.context && (
          <section>
            <h3 className="text-label-12 text-ds-gray-900 mb-1">
              Context
            </h3>
            <p className="text-copy-13 text-ds-gray-900 leading-relaxed">
              {approval.context}
            </p>
          </section>
        )}
      </div>

      {/* Action buttons */}
      <div className="px-4 py-3 border-t border-ds-gray-400 shrink-0">
        <div className="flex gap-3">
          <button
            type="button"
            onClick={() => void onApprove(approval.id)}
            disabled={approving || dismissing}
            className="flex-1 flex items-center justify-center gap-2 px-3 py-2 min-h-9 rounded-lg text-button-14 bg-green-700 hover:bg-green-700/80 text-white transition-colors disabled:opacity-50"
          >
            <CheckCircle size={14} />
            {approving ? "Approving\u2026" : "Approve"}
          </button>
          <button
            type="button"
            onClick={() => void onDismiss(approval.id)}
            disabled={approving || dismissing}
            className="flex-1 flex items-center justify-center gap-2 px-3 py-2 min-h-9 rounded-lg text-button-14 bg-ds-gray-100 hover:bg-red-700/20 text-ds-gray-900 hover:text-red-700 border border-ds-gray-400 hover:border-red-700/40 transition-colors disabled:opacity-50"
          >
            <XCircle size={14} />
            {dismissing ? "Dismissing\u2026" : "Dismiss"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Approvals Tab Content
// ---------------------------------------------------------------------------

function ApprovalsTabContent() {
  const [approvals, setApprovals] = useState<Approval[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [approving, setApproving] = useState(false);
  const [dismissing, setDismissing] = useState(false);
  const [mobileDetailOpen, setMobileDetailOpen] = useState(false);
  const [checkedIds, setCheckedIds] = useState<Set<string>>(new Set());
  const [showCelebration, setShowCelebration] = useState(false);
  const [batchBusy, setBatchBusy] = useState(false);

  const pending = approvals.filter((a) => a.status === "pending");
  const selected = pending.find((a) => a.id === selectedId) ?? null;

  const fetchApprovals = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch("/api/obligations?owner=leo&status=open");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as ObligationsGetResponse;
      const mapped = (data.obligations ?? []).map(mapObligationToApproval);
      setApprovals(mapped);
      if (mapped.length > 0 && !selectedId) {
        setSelectedId(mapped[0]!.id);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load approvals");
    } finally {
      setLoading(false);
    }
  }, [selectedId]);

  useEffect(() => {
    void fetchApprovals();
  }, [fetchApprovals]);

  useDaemonEvents(
    useCallback(
      (_ev) => {
        void fetchApprovals();
      },
      [fetchApprovals],
    ),
    "approval",
  );

  const handleApprove = async (id: string) => {
    setApproving(true);
    try {
      const res = await apiFetch(`/api/approvals/${id}/approve`, { method: "POST" });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setApprovals((prev) => {
        const next = prev.filter((a) => a.id !== id);
        if (next.filter((a) => a.status === "pending").length === 0 && prev.filter((a) => a.status === "pending").length > 0) {
          setShowCelebration(true);
        }
        return next;
      });
      setCheckedIds((prev) => { const next = new Set(prev); next.delete(id); return next; });
      setSelectedId(null);
      setMobileDetailOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to approve");
    } finally {
      setApproving(false);
    }
  };

  const handleDismiss = async (id: string) => {
    setDismissing(true);
    try {
      const res = await apiFetch(`/api/obligations/${id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ status: "dismissed" }),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setApprovals((prev) => {
        const next = prev.filter((a) => a.id !== id);
        if (next.filter((a) => a.status === "pending").length === 0 && prev.filter((a) => a.status === "pending").length > 0) {
          setShowCelebration(true);
        }
        return next;
      });
      setCheckedIds((prev) => { const next = new Set(prev); next.delete(id); return next; });
      setSelectedId(null);
      setMobileDetailOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to dismiss");
    } finally {
      setDismissing(false);
    }
  };

  const handleSelect = (id: string) => {
    setSelectedId(id);
    setMobileDetailOpen(true);
  };

  const handleToggleCheck = (id: string) => {
    setCheckedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleBatchApprove = async () => {
    setBatchBusy(true);
    try {
      await Promise.all(
        Array.from(checkedIds).map(async (id) => {
          const res = await apiFetch(`/api/approvals/${id}/approve`, { method: "POST" });
          if (!res.ok) throw new Error(`HTTP ${res.status}`);
        }),
      );
      setApprovals((prev) => {
        const next = prev.filter((a) => !checkedIds.has(a.id));
        if (next.filter((a) => a.status === "pending").length === 0 && prev.filter((a) => a.status === "pending").length > 0) {
          setShowCelebration(true);
        }
        return next;
      });
      setCheckedIds(new Set());
      setSelectedId(null);
      setMobileDetailOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to batch approve");
    } finally {
      setBatchBusy(false);
    }
  };

  const handleBatchDismiss = async () => {
    setBatchBusy(true);
    try {
      await Promise.all(
        Array.from(checkedIds).map(async (id) => {
          const res = await apiFetch(`/api/obligations/${id}`, {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ status: "dismissed" }),
          });
          if (!res.ok) throw new Error(`HTTP ${res.status}`);
        }),
      );
      setApprovals((prev) => {
        const next = prev.filter((a) => !checkedIds.has(a.id));
        if (next.filter((a) => a.status === "pending").length === 0 && prev.filter((a) => a.status === "pending").length > 0) {
          setShowCelebration(true);
        }
        return next;
      });
      setCheckedIds(new Set());
      setSelectedId(null);
      setMobileDetailOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to batch dismiss");
    } finally {
      setBatchBusy(false);
    }
  };

  useApprovalKeyboard({
    pendingIds: pending.map((a) => a.id),
    selectedId,
    onNavigate: (id) => {
      setSelectedId(id);
      setMobileDetailOpen(true);
    },
    onApprove: (id) => void handleApprove(id),
    onDismiss: (id) => void handleDismiss(id),
    busy: approving || dismissing || batchBusy,
  });

  if (error) {
    return <ErrorBanner message={error} onRetry={() => void fetchApprovals()} />;
  }

  if (loading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 5 }).map((_, i) => (
          <div key={i} className="h-14 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400" />
        ))}
      </div>
    );
  }

  if (pending.length === 0) {
    return showCelebration ? (
      <QueueClearCelebration />
    ) : (
      <p className="text-copy-13 text-ds-gray-900 py-3">No pending approvals</p>
    );
  }

  return (
    <div className="flex gap-0 surface-card overflow-hidden min-h-[400px]">
      {/* Queue list */}
      <div
        className={[
          "flex flex-col border-r border-ds-gray-400 bg-ds-bg-100 overflow-y-auto",
          mobileDetailOpen ? "hidden md:flex md:w-64 lg:w-72 shrink-0" : "w-full md:w-64 lg:w-72 shrink-0",
        ].join(" ")}
      >
        <div className="flex items-center gap-2 px-4 py-2.5 border-b border-ds-gray-400 shrink-0">
          <AlertTriangle size={14} className="text-amber-700 shrink-0" />
          <span className="text-label-12 text-ds-gray-900">Queue</span>
          <span className="ml-auto inline-flex items-center justify-center px-1.5 py-0.5 min-w-[1.25rem] rounded text-copy-13 font-mono font-medium bg-ds-gray-400 text-ds-gray-1000">
            {pending.length}
          </span>
        </div>

        {pending.map((a) => (
          <ApprovalQueueItem
            key={a.id}
            approval={a}
            selected={a.id === selectedId}
            checked={checkedIds.has(a.id)}
            onSelect={() => handleSelect(a.id)}
            onToggleCheck={handleToggleCheck}
          />
        ))}

        <BatchActionBar
          selectedCount={checkedIds.size}
          onApproveAll={() => void handleBatchApprove()}
          onDismissAll={() => void handleBatchDismiss()}
          onClearSelection={() => setCheckedIds(new Set())}
          busy={batchBusy}
        />
      </div>

      {/* Detail panel */}
      <div
        className={[
          "flex-1 surface-raised rounded-none",
          mobileDetailOpen ? "flex flex-col w-full md:flex" : "hidden md:flex md:flex-col",
        ].join(" ")}
      >
        <button
          type="button"
          onClick={() => setMobileDetailOpen(false)}
          className="flex md:hidden items-center gap-2 px-4 py-2.5 text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 border-b border-ds-gray-400"
        >
          <ChevronRight size={14} className="rotate-180" />
          Back to queue
        </button>

        {selected ? (
          <DetailPanel
            approval={selected}
            onApprove={handleApprove}
            onDismiss={handleDismiss}
            approving={approving}
            dismissing={dismissing}
          />
        ) : (
          <div className="flex flex-col items-center justify-center h-full gap-3 text-ds-gray-900 py-8">
            <AlertTriangle size={24} />
            <p className="text-copy-13">Select an item to review</p>
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// KanbanBoardWithKeyboard — wraps KanbanBoard with keyboard navigation
// ---------------------------------------------------------------------------

interface KanbanBoardWithKeyboardProps {
  obligations: DaemonObligation[];
  onRefresh: () => void;
  approachingDeadlineHours: number;
  loading: boolean;
}

function KanbanBoardWithKeyboard({
  obligations,
  onRefresh,
  approachingDeadlineHours,
  loading,
}: KanbanBoardWithKeyboardProps) {
  const [keyboardExpandedId, setKeyboardExpandedId] = useState<string | null>(null);

  const handleKeyboardDone = useCallback(
    async (id: string) => {
      try {
        await apiFetch(`/api/obligations/${id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ status: "done" }),
        });
        onRefresh();
      } catch {
        // ignore
      }
    },
    [onRefresh],
  );

  const handleKeyboardDismiss = useCallback(
    async (id: string) => {
      try {
        await apiFetch(`/api/obligations/${id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ status: "dismissed" }),
        });
        onRefresh();
      } catch {
        // ignore
      }
    },
    [onRefresh],
  );

  const handleKeyboardReassign = useCallback(
    async (id: string) => {
      const obl = obligations.find((o) => o.id === id);
      if (!obl) return;
      const newOwner = obl.owner === "nova" ? "leo" : "nova";
      try {
        await apiFetch(`/api/obligations/${id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ owner: newOwner }),
        });
        onRefresh();
      } catch {
        // ignore
      }
    },
    [obligations, onRefresh],
  );

  useKanbanKeyboard({
    obligations,
    expandedId: keyboardExpandedId,
    onExpand: (id) => setKeyboardExpandedId((prev) => (prev === id ? null : id)),
    onDone: (id) => void handleKeyboardDone(id),
    onDismiss: (id) => void handleKeyboardDismiss(id),
    onReassign: (id) => void handleKeyboardReassign(id),
    active: true,
  });

  if (loading) {
    return (
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {[0, 1].map((col) => (
          <div key={col} className="space-y-2">
            <div className="h-6 w-24 animate-pulse rounded bg-ds-gray-100" />
            {Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="h-16 animate-pulse rounded-lg bg-ds-gray-100 border border-ds-gray-400" />
            ))}
          </div>
        ))}
      </div>
    );
  }

  return (
    <KanbanBoard
      obligations={obligations}
      onRefresh={onRefresh}
      approachingDeadlineHours={approachingDeadlineHours}
    />
  );
}

// ---------------------------------------------------------------------------
// Page (with Suspense for useSearchParams)
// ---------------------------------------------------------------------------

export default function ObligationsPageWrapper() {
  return (
    <Suspense>
      <ObligationsPage />
    </Suspense>
  );
}

type TabKey = "open" | "history" | "approvals";

function ObligationsPage() {
  const searchParams = useSearchParams();
  const initialTab = (searchParams.get("tab") as TabKey) ?? "open";
  const queryClient = useQueryClient();

  const [tab, setTab] = useState<TabKey>(
    initialTab === "approvals" ? "approvals" : initialTab === "history" ? "history" : "open",
  );
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [initialExpandDone, setInitialExpandDone] = useState(false);
  const [approachingDeadlineHours, setApproachingDeadlineHours] = useState(
    DEFAULT_APPROACHING_DEADLINE_HOURS,
  );
  const listRef = useRef<HTMLDivElement>(null);

  // Query: obligations list
  const oblQuery = useApiQuery<ObligationsGetResponse>("/api/obligations");
  const obligations = oblQuery.data?.obligations ?? [];
  const loading = oblQuery.isLoading;
  const error = oblQuery.error;

  // Query: config for deadline threshold
  const configQuery = useApiQuery<Record<string, unknown>>("/api/config");
  useEffect(() => {
    if (configQuery.data) {
      const hours = configQuery.data.approaching_deadline_hours;
      if (typeof hours === "number" && hours > 0) {
        setApproachingDeadlineHours(hours);
      }
    }
  }, [configQuery.data]);

  const fetchObligations = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: queryKeys.api("/api/obligations") });
  }, [queryClient]);

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

  // Expand first item by default on initial load
  useEffect(() => {
    if (!initialExpandDone && obligations.length > 0) {
      setExpandedIds(new Set([obligations[0]!.id]));
      setInitialExpandDone(true);
    }
  }, [obligations, initialExpandDone]);

  const sortByPriority = (items: DaemonObligation[]) =>
    [...items].sort((a, b) => a.priority - b.priority);

  const activeStatuses = ["open", "in_progress", "proposed_done"];
  const open = obligations.filter((o) => activeStatuses.includes(o.status));
  const history = obligations.filter(
    (o) => o.status === "done" || o.status === "dismissed",
  );

  return (
    <div className="p-4 space-y-3 w-full animate-fade-in-up">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-heading-20 text-ds-gray-1000">Obligations</h1>
          <p className="mt-0.5 text-copy-13 text-ds-gray-900">
            Active tasks and commitments
          </p>
        </div>
        <button
          type="button"
          onClick={fetchObligations}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={oblQuery.isFetching ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Compact summary bar */}
      {obligations.length > 0 && (
        <div className="section-stagger-1">
          <ObligationSummaryBar obligations={obligations} />
        </div>
      )}

      {/* Tabs — now includes Approvals */}
      <div className="flex gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400 w-fit section-stagger-2">
        {(
          [
            { key: "open" as const, icon: <CheckSquare size={14} />, label: "Active", count: open.length },
            { key: "approvals" as const, icon: <ShieldAlert size={14} />, label: "Approvals", count: null },
            { key: "history" as const, icon: <Clock size={14} />, label: "History", count: history.length },
          ] as const
        ).map((t) => (
          <button
            key={t.key}
            type="button"
            onClick={() => setTab(t.key)}
            className={`flex items-center gap-2 px-4 py-1.5 rounded text-label-13 transition-colors ${
              tab === t.key
                ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                : "text-ds-gray-900 hover:text-ds-gray-1000"
            }`}
          >
            {t.icon}
            <span>{t.label}</span>
            {t.count !== null && (
              <span className="text-copy-13 font-mono opacity-70">{t.count}</span>
            )}
          </button>
        ))}
      </div>

      {error && (
        <ErrorBanner
          message="Failed to load obligations"
          detail={error.message}
          onRetry={fetchObligations}
        />
      )}

      {/* Tab content */}
      {tab === "approvals" ? (
        <ApprovalsTabContent />
      ) : tab === "open" ? (
        <div className="section-stagger-3">
          {/* Kanban board — replaces flat Nova/Leo sections + ActivityFeed sidebar */}
          <KanbanBoardWithKeyboard
            obligations={open}
            onRefresh={fetchObligations}
            approachingDeadlineHours={approachingDeadlineHours}
            loading={loading}
          />
        </div>
      ) : (
        /* History tab */
        <div ref={listRef} className="section-stagger-3">
          {loading ? (
            <div className="space-y-2">
              {Array.from({ length: 5 }).map((_, i) => (
                <div
                  key={i}
                  className="h-28 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
                />
              ))}
            </div>
          ) : (
            <div key="history" className="animate-crossfade-in space-y-3">
              {history.length === 0 ? (
                <p className="text-copy-13 text-ds-gray-900 py-3">No history yet</p>
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
      )}
    </div>
  );
}
