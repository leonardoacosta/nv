"use client";

import { useState } from "react";
import { Plus } from "lucide-react";
import type { DaemonObligation } from "@/types/api";
import KanbanLane from "@/components/obligations/KanbanLane";
import InlineCreate from "@/components/obligations/InlineCreate";

const LANE_STATUSES = ["in_progress", "open", "proposed_done"] as const;

const OWNER_CONFIG = {
  nova: {
    label: "Nova",
    initial: "N",
    badgeClass: "bg-ds-gray-700/30 text-ds-gray-1000",
    headerClass: "text-ds-gray-1000",
  },
  leo: {
    label: "Leo",
    initial: "L",
    badgeClass: "bg-red-700/20 text-red-500",
    headerClass: "text-red-400",
  },
} as const;

export interface KanbanColumnProps {
  owner: "nova" | "leo";
  obligations: DaemonObligation[];
  expandedId: string | null;
  onSelect: (id: string) => void;
  onRefresh: () => void;
  onStatusChange: (obligationId: string, newStatus: string) => void;
  approachingDeadlineHours?: number;
  draggingId?: string | null;
  forceShowCreate?: boolean;
  onForceCreateShown?: () => void;
}

export default function KanbanColumn({
  owner,
  obligations,
  expandedId,
  onSelect,
  onRefresh,
  onStatusChange,
  approachingDeadlineHours = 24,
  draggingId = null,
  forceShowCreate = false,
  onForceCreateShown,
}: KanbanColumnProps) {
  const [showCreate, setShowCreate] = useState(false);

  const config = OWNER_CONFIG[owner];

  const isCreateVisible = showCreate || forceShowCreate;

  const handleCreated = () => {
    setShowCreate(false);
    onForceCreateShown?.();
    onRefresh();
  };

  const handleCancel = () => {
    setShowCreate(false);
    onForceCreateShown?.();
  };

  return (
    <div className="flex flex-col gap-2">
      {/* Column header */}
      <div className="flex items-center gap-2.5 px-1">
        <div
          className={`w-6 h-6 rounded flex items-center justify-center shrink-0 ${config.badgeClass}`}
        >
          <span className="text-xs font-bold font-mono">{config.initial}</span>
        </div>
        <span className={`text-sm font-semibold uppercase tracking-wide ${config.headerClass}`}>
          {config.label}
        </span>
        <span
          className="inline-flex items-center justify-center px-1.5 py-0.5 min-w-[1.25rem] rounded text-xs font-mono font-medium text-ds-gray-900"
          style={{ background: "var(--ds-gray-alpha-200)" }}
        >
          {obligations.length}
        </span>
        <button
          type="button"
          onClick={() => setShowCreate(true)}
          className="ml-auto flex items-center justify-center w-6 h-6 rounded border border-ds-gray-400 text-ds-gray-700 hover:text-ds-gray-1000 hover:border-ds-gray-500 hover:bg-ds-gray-alpha-100 transition-colors"
          aria-label={`Create obligation for ${config.label}`}
        >
          <Plus size={13} />
        </button>
      </div>

      {/* Lanes */}
      {LANE_STATUSES.map((laneStatus) => (
        <div key={laneStatus}>
          {/* InlineCreate appears at top of Pending (open) lane */}
          {laneStatus === "open" && isCreateVisible && (
            <div className="mb-1.5">
              <InlineCreate
                owner={owner}
                onCreated={handleCreated}
                onCancel={handleCancel}
              />
            </div>
          )}
          <KanbanLane
            status={laneStatus}
            obligations={obligations.filter((o) => o.status === laneStatus)}
            expandedId={expandedId}
            onSelect={onSelect}
            onRefresh={onRefresh}
            onDrop={onStatusChange}
            approachingDeadlineHours={approachingDeadlineHours}
            draggingId={draggingId}
          />
        </div>
      ))}
    </div>
  );
}
