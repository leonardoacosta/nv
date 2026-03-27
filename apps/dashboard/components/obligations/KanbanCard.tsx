"use client";

import { useState, useEffect, useCallback } from "react";
import {
  Check,
  X,
  ArrowLeftRight,
  ChevronDown,
  ChevronUp,
  Clock,
  FolderOpen,
  Radio,
  Play,
  RotateCcw,
} from "lucide-react";
import type { DaemonObligation, ObligationActivity } from "@/types/api";
import { apiFetch } from "@/lib/api-client";

// ── Shared constants (replicate from obligations page to keep components independent) ──

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

// ── Tooltip ──────────────────────────────────────────────────────────────────

function Tooltip({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="relative group/tooltip">
      {children}
      <span className="pointer-events-none absolute -top-8 left-1/2 -translate-x-1/2 whitespace-nowrap rounded bg-ds-gray-200 border border-ds-gray-400 px-2 py-1 text-[11px] text-ds-gray-1000 opacity-0 transition-opacity group-hover/tooltip:opacity-100 z-50">
        {label}
      </span>
    </div>
  );
}

// ── Activity section ──────────────────────────────────────────────────────────

function CardActivity({ obligationId }: { obligationId: string }) {
  const [events, setEvents] = useState<ObligationActivity[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const res = await apiFetch(`/api/obligations/${obligationId}/activity`);
        if (!cancelled && res.ok) {
          const data = (await res.json()) as { events: ObligationActivity[] };
          setEvents(data.events ?? []);
        }
      } catch {
        // silently ignore
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [obligationId]);

  if (loading) {
    return (
      <div className="space-y-1.5">
        {Array.from({ length: 2 }).map((_, i) => (
          <div key={i} className="h-4 animate-pulse rounded bg-ds-gray-100" />
        ))}
      </div>
    );
  }

  if (events.length === 0) {
    return <p className="text-xs text-ds-gray-700 italic">No activity yet</p>;
  }

  return (
    <div className="space-y-1.5">
      {events.slice(0, 5).map((ev) => (
        <div key={ev.id} className="flex gap-2 text-xs">
          <span className="font-mono text-ds-gray-700 shrink-0 whitespace-nowrap" suppressHydrationWarning>
            {relativeTime(ev.timestamp)}
          </span>
          <span className="text-ds-gray-900 font-mono uppercase text-[10px] shrink-0">
            {ev.event_type}
          </span>
          <span className="text-ds-gray-1000 truncate">{ev.description}</span>
        </div>
      ))}
    </div>
  );
}

// ── KanbanCard ────────────────────────────────────────────────────────────────

export interface KanbanCardProps {
  obligation: DaemonObligation;
  isExpanded: boolean;
  onSelect: (id: string) => void;
  onRefresh: () => void;
  approachingDeadlineHours?: number;
  isDragging?: boolean;
  onDragStart?: (e: React.DragEvent, id: string) => void;
}

export default function KanbanCard({
  obligation,
  isExpanded,
  onSelect,
  onRefresh,
  approachingDeadlineHours = 24,
  isDragging = false,
  onDragStart,
}: KanbanCardProps) {
  const [actionPending, setActionPending] = useState(false);
  const [isHovered, setIsHovered] = useState(false);
  const [notesExpanded, setNotesExpanded] = useState(false);

  const priorityBar = PRIORITY_BAR[obligation.priority] ?? PRIORITY_BAR[2]!;
  const priorityText = PRIORITY_TEXT[obligation.priority] ?? PRIORITY_TEXT[2]!;
  const statusBadge = STATUS_BADGE[obligation.status] ?? STATUS_BADGE["open"]!;
  const statusLabel = STATUS_LABEL[obligation.status] ?? obligation.status;

  const deadlineProximity = getDeadlineProximity(obligation.deadline, approachingDeadlineHours);
  const deadlineRing = DEADLINE_RING[deadlineProximity];

  const notes = obligation.notes ?? [];
  const mostRecentNote = notes[0];
  const olderNotes = notes.slice(1);

  const patchStatus = useCallback(async (status: string) => {
    setActionPending(true);
    try {
      const res = await apiFetch(`/api/obligations/${obligation.id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ status }),
      });
      if (res.ok) onRefresh();
    } catch {
      // ignore
    } finally {
      setActionPending(false);
    }
  }, [obligation.id, onRefresh]);

  const handleStart = useCallback(async () => {
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
  }, [obligation.id, onRefresh]);

  const handleReassign = useCallback(async () => {
    const newOwner = obligation.owner === "nova" ? "leo" : "nova";
    setActionPending(true);
    try {
      const res = await apiFetch(`/api/obligations/${obligation.id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ owner: newOwner }),
      });
      if (res.ok) onRefresh();
    } catch {
      // ignore
    } finally {
      setActionPending(false);
    }
  }, [obligation.id, obligation.owner, onRefresh]);

  // Hover actions based on status
  const hoverActions = (() => {
    const base: Array<{ icon: React.ReactNode; label: string; onClick: () => void; cls: string }> = [];
    if (obligation.status === "open") {
      base.push(
        { icon: <Play size={11} />, label: "Start", onClick: () => void handleStart(), cls: "text-ds-gray-900 hover:bg-ds-gray-alpha-200" },
        { icon: <Check size={11} />, label: "Done", onClick: () => void patchStatus("done"), cls: "text-green-600 hover:bg-green-700/20" },
        { icon: <X size={11} />, label: "Dismiss", onClick: () => void patchStatus("dismissed"), cls: "text-ds-gray-900 hover:bg-ds-gray-alpha-200" },
      );
    } else if (obligation.status === "in_progress") {
      base.push(
        { icon: <Check size={11} />, label: "Done", onClick: () => void patchStatus("done"), cls: "text-green-600 hover:bg-green-700/20" },
        { icon: <X size={11} />, label: "Dismiss", onClick: () => void patchStatus("dismissed"), cls: "text-ds-gray-900 hover:bg-ds-gray-alpha-200" },
      );
    } else if (obligation.status === "proposed_done") {
      base.push(
        { icon: <Check size={11} />, label: "Confirm Done", onClick: () => void patchStatus("done"), cls: "text-green-600 hover:bg-green-700/20" },
        { icon: <RotateCcw size={11} />, label: "Reopen", onClick: () => void patchStatus("open"), cls: "text-ds-gray-900 hover:bg-ds-gray-alpha-200" },
      );
    }
    base.push({
      icon: <ArrowLeftRight size={11} />,
      label: `Reassign to ${obligation.owner === "nova" ? "Leo" : "Nova"}`,
      onClick: () => void handleReassign(),
      cls: "text-ds-gray-700 hover:bg-ds-gray-alpha-200",
    });
    return base;
  })();

  return (
    <div
      id={`kanban-card-${obligation.id}`}
      draggable
      onDragStart={(e) => onDragStart?.(e, obligation.id)}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      className={[
        "relative surface-card overflow-hidden transition-all duration-150",
        "cursor-grab active:cursor-grabbing",
        deadlineRing,
        isDragging ? "opacity-50 scale-95" : "opacity-100 scale-100",
      ].join(" ")}
    >
      {/* Priority bar */}
      <div className={`absolute left-0 top-0 bottom-0 w-1 ${priorityBar}`} aria-hidden="true" />

      {/* Hover action bar */}
      {isHovered && !isExpanded && !actionPending && (
        <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-0.5 bg-ds-gray-200 border border-ds-gray-400 rounded-lg px-1 py-0.5 z-10 shadow-sm">
          {hoverActions.map((action) => (
            <Tooltip key={action.label} label={action.label}>
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); action.onClick(); }}
                disabled={actionPending}
                className={`flex items-center justify-center w-6 h-6 rounded transition-colors disabled:opacity-50 ${action.cls}`}
                aria-label={action.label}
              >
                {action.icon}
              </button>
            </Tooltip>
          ))}
        </div>
      )}

      {/* Card header — click to expand */}
      <button
        type="button"
        onClick={() => onSelect(obligation.id)}
        className="w-full text-left pl-4 pr-10 pt-2.5 pb-2"
      >
        <div className="flex items-center gap-2 mb-1">
          <span className={`text-[10px] font-mono font-bold shrink-0 ${priorityText}`}>
            P{obligation.priority}
          </span>
          <span className="text-copy-13 font-medium text-ds-gray-1000 truncate flex-1 min-w-0">
            {obligation.detected_action}
          </span>
        </div>

        <div className="flex items-center gap-2 flex-wrap">
          {deadlineProximity === "overdue" && (
            <span className="text-[10px] font-mono font-bold text-red-500 uppercase px-1 py-0.5 rounded bg-red-700/20">
              Overdue
            </span>
          )}
          {deadlineProximity === "approaching" && (
            <span className="text-[10px] font-mono font-bold text-amber-500 uppercase px-1 py-0.5 rounded bg-amber-500/20">
              Due Soon
            </span>
          )}
          <span className={`text-[10px] px-1.5 py-0.5 rounded font-mono ${statusBadge}`}>
            {statusLabel}
          </span>

          {obligation.deadline && (
            <span
              className={`flex items-center gap-0.5 text-[10px] font-mono ${
                deadlineProximity === "overdue"
                  ? "text-red-500"
                  : deadlineProximity === "approaching"
                    ? "text-amber-500"
                    : "text-ds-gray-700"
              }`}
              suppressHydrationWarning
            >
              <Clock size={9} />
              {new Date(obligation.deadline).toLocaleDateString()}
            </span>
          )}

          <span className="flex items-center gap-0.5 text-[10px] font-mono text-ds-gray-700" suppressHydrationWarning>
            <Clock size={9} />
            {relativeTime(obligation.created_at)}
          </span>

          {obligation.project_code && (
            <span className="flex items-center gap-0.5 text-[10px] font-mono text-ds-gray-700">
              <FolderOpen size={9} />
              {obligation.project_code}
            </span>
          )}
        </div>
      </button>

      {/* Expanded detail section */}
      {isExpanded && (
        <div className="pl-4 pr-4 pb-3 border-t border-ds-gray-400 mt-0 space-y-3 pt-2">
          {/* Source context */}
          {(obligation.source_channel || obligation.source_message) && (
            <div className="flex gap-2 text-xs text-ds-gray-900 bg-ds-gray-alpha-100 rounded-lg px-3 py-2">
              <Radio size={12} className="shrink-0 mt-0.5 text-ds-gray-700" />
              <div className="flex-1 min-w-0">
                <span className="font-mono text-ds-gray-700 uppercase text-[10px]">
                  {obligation.source_channel}
                </span>
                {obligation.source_message && (
                  <p className="mt-0.5 text-ds-gray-1000 leading-snug text-xs line-clamp-3">
                    {obligation.source_message}
                  </p>
                )}
              </div>
            </div>
          )}

          {/* Execution history */}
          {notes.length > 0 && (
            <div className="space-y-1.5">
              <span className="text-label-12 text-ds-gray-900 uppercase tracking-wide">
                Execution History
              </span>
              <div className="space-y-1.5 pl-1">
                {mostRecentNote && (
                  <div key={mostRecentNote.id} className="flex gap-2 text-xs">
                    <div className="w-1 bg-ds-gray-400 rounded-full shrink-0 self-stretch mt-1" />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-ds-gray-700" suppressHydrationWarning>
                          {relativeTime(mostRecentNote.created_at)}
                        </span>
                        <span className="text-ds-gray-900 font-mono uppercase text-[10px]">
                          {mostRecentNote.note_type}
                        </span>
                      </div>
                      <p className="mt-0.5 text-ds-gray-900 leading-snug text-xs">
                        {mostRecentNote.content}
                      </p>
                    </div>
                  </div>
                )}
                {olderNotes.length > 0 && (
                  <>
                    {notesExpanded && olderNotes.map((n) => (
                      <div key={n.id} className="flex gap-2 text-xs">
                        <div className="w-1 bg-ds-gray-400 rounded-full shrink-0 self-stretch mt-1" />
                        <div className="flex-1 min-w-0">
                          <span className="font-mono text-ds-gray-700" suppressHydrationWarning>
                            {relativeTime(n.created_at)}
                          </span>
                          <p className="mt-0.5 text-ds-gray-900 text-xs line-clamp-1">{n.content}</p>
                        </div>
                      </div>
                    ))}
                    <button
                      type="button"
                      onClick={() => setNotesExpanded((v) => !v)}
                      className="flex items-center gap-1 text-xs text-ds-gray-700 hover:text-ds-gray-1000 transition-colors"
                    >
                      {notesExpanded ? (
                        <><ChevronUp size={11} /> Hide {olderNotes.length} older</>
                      ) : (
                        <><ChevronDown size={11} /> Show {olderNotes.length} older</>
                      )}
                    </button>
                  </>
                )}
              </div>
            </div>
          )}

          {/* Activity */}
          <div className="space-y-1.5">
            <span className="text-label-12 text-ds-gray-900 uppercase tracking-wide">
              Activity
            </span>
            <CardActivity obligationId={obligation.id} />
          </div>

          {/* Inline action buttons */}
          <div className="flex gap-1.5 flex-wrap pt-1">
            {hoverActions.map((action) => (
              <button
                key={action.label}
                type="button"
                onClick={action.onClick}
                disabled={actionPending}
                className={`flex items-center gap-1.5 px-2.5 py-1 rounded text-xs border border-ds-gray-400 transition-colors disabled:opacity-50 ${action.cls}`}
              >
                {action.icon}
                {action.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
