"use client";

import {
  Clock,
  ChevronRight,
  FileText,
  GitPullRequest,
  Terminal,
  HelpCircle,
} from "lucide-react";
import type { Approval, ApprovalActionType } from "./types";

// ---------------------------------------------------------------------------
// Helpers
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
  { label: string; dot: string; text: string; border: string }
> = {
  critical: {
    label: "Critical",
    dot: "bg-[#EF4444]",
    text: "text-[#EF4444]",
    border: "border-l-[3px] border-l-red-700",
  },
  high: {
    label: "High",
    dot: "bg-[#F97316]",
    text: "text-[#F97316]",
    border: "border-l-[3px] border-l-red-700",
  },
  medium: {
    label: "Medium",
    dot: "bg-amber-400",
    text: "text-amber-400",
    border: "border-l-[3px] border-l-amber-700",
  },
  low: {
    label: "Low",
    dot: "bg-ds-gray-600",
    text: "text-ds-gray-900",
    border: "border-l-[3px] border-l-transparent",
  },
};

function relativeTime(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffH = Math.floor(diffMin / 60);
  if (diffH < 24) return `${diffH}h ago`;
  return `${Math.floor(diffH / 24)}d ago`;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface ApprovalQueueItemProps {
  approval: Approval;
  /** Whether this item is the currently focused/viewed item. */
  selected: boolean;
  /** Whether the batch-selection checkbox is checked. */
  checked: boolean;
  /** Called when the user clicks the item row (focus it in detail panel). */
  onSelect: () => void;
  /** Called when the checkbox is toggled. */
  onToggleCheck: (id: string) => void;
}

export default function ApprovalQueueItem({
  approval,
  selected,
  checked,
  onSelect,
  onToggleCheck,
}: ApprovalQueueItemProps) {
  const ActionIcon = ACTION_ICON[approval.action_type] ?? HelpCircle;
  const urg = URGENCY_CONFIG[approval.urgency];

  return (
    <div
      className={[
        "group relative w-full flex items-start gap-3 px-4 py-3.5 min-h-11 transition-colors",
        "border-b border-ds-gray-400 last:border-b-0",
        urg.border,
        selected ? "bg-ds-gray-700/15" : "hover:bg-ds-gray-100/60",
      ].join(" ")}
    >
      {/* Batch checkbox */}
      <input
        type="checkbox"
        checked={checked}
        onChange={() => onToggleCheck(approval.id)}
        aria-label={`Select "${approval.title}" for batch action`}
        className="mt-2 h-4 w-4 shrink-0 rounded border-ds-gray-400 accent-emerald-600 cursor-pointer"
      />

      {/* Clickable row area */}
      <button
        type="button"
        onClick={onSelect}
        className="flex items-start gap-3 flex-1 min-w-0 text-left"
      >
        {/* Action type icon */}
        <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-ds-gray-100 border border-ds-gray-400 shrink-0 mt-0.5">
          <ActionIcon size={14} className="text-ds-gray-900" />
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-sm font-medium text-ds-gray-1000 truncate">
              {approval.title}
            </span>
            {/* Urgency dot */}
            <span
              className={`inline-block w-2 h-2 rounded-full shrink-0 ${urg.dot}`}
              aria-label={`Urgency: ${urg.label}`}
              title={`Urgency: ${urg.label}`}
            />
          </div>

          <div className="flex items-center gap-2 mt-0.5 flex-wrap">
            {approval.project && (
              <span className="text-xs font-mono text-ds-gray-900 truncate">
                {approval.project}
              </span>
            )}
            <span className="text-xs text-ds-gray-900 flex items-center gap-1">
              <Clock size={10} />
              <span suppressHydrationWarning>{relativeTime(approval.created_at)}</span>
            </span>
          </div>
        </div>

        <ChevronRight
          size={14}
          className={`shrink-0 mt-1 transition-colors ${
            selected ? "text-ds-gray-1000" : "text-ds-gray-900/40"
          }`}
        />
      </button>

      {/* Shortcut hints on hover */}
      <div className="absolute right-10 top-1/2 -translate-y-1/2 hidden group-hover:flex items-center gap-1 pointer-events-none">
        {selected && (
          <>
            <kbd className="px-1.5 py-0.5 text-[10px] font-mono font-medium rounded bg-ds-gray-100 border border-ds-gray-400 text-ds-gray-900">
              A
            </kbd>
            <kbd className="px-1.5 py-0.5 text-[10px] font-mono font-medium rounded bg-ds-gray-100 border border-ds-gray-400 text-ds-gray-900">
              D
            </kbd>
          </>
        )}
      </div>
    </div>
  );
}
