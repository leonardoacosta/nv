"use client";

import { useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import type { DaemonObligation } from "@/types/api";
import KanbanCard from "@/components/obligations/KanbanCard";

const LANE_STATUS_CONFIG: Record<string, { label: string; dotClass: string }> = {
  in_progress: { label: "In Progress", dotClass: "bg-amber-500" },
  open: { label: "Pending", dotClass: "bg-ds-gray-1000" },
  proposed_done: { label: "Proposed Done", dotClass: "bg-blue-500" },
};

export interface KanbanLaneProps {
  status: string;
  obligations: DaemonObligation[];
  expandedId: string | null;
  onSelect: (id: string) => void;
  onRefresh: () => void;
  onDrop: (obligationId: string, targetStatus: string) => void;
  approachingDeadlineHours?: number;
  draggingId?: string | null;
}

export default function KanbanLane({
  status,
  obligations,
  expandedId,
  onSelect,
  onRefresh,
  onDrop,
  approachingDeadlineHours = 24,
  draggingId = null,
}: KanbanLaneProps) {
  const [collapsed, setCollapsed] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);

  const config = LANE_STATUS_CONFIG[status] ?? { label: status, dotClass: "bg-ds-gray-700" };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setIsDragOver(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    // Only clear if leaving the lane entirely (not just entering a child)
    if (!e.currentTarget.contains(e.relatedTarget as Node)) {
      setIsDragOver(false);
    }
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
    const id = e.dataTransfer.getData("obligationId");
    if (id) {
      onDrop(id, status);
    }
  };

  const handleDragStart = (e: React.DragEvent, id: string) => {
    e.dataTransfer.setData("obligationId", id);
    e.dataTransfer.effectAllowed = "move";
  };

  // Sort by priority ascending
  const sorted = [...obligations].sort((a, b) => a.priority - b.priority);

  return (
    <div
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      className={[
        "rounded-lg transition-all duration-150",
        isDragOver
          ? "border-2 border-dashed border-ds-gray-700 bg-ds-gray-alpha-100"
          : "border border-ds-gray-400",
      ].join(" ")}
    >
      {/* Lane header */}
      <div className="flex items-center gap-2 px-2.5 py-1.5">
        <div className={`w-1.5 h-1.5 rounded-full shrink-0 ${config.dotClass}`} aria-hidden="true" />
        <span className="text-label-12 text-ds-gray-700 flex-1 min-w-0">
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
          onClick={() => setCollapsed((v) => !v)}
          className="flex items-center justify-center w-5 h-5 rounded hover:bg-ds-gray-alpha-200 transition-colors text-ds-gray-700"
          aria-label={collapsed ? "Expand lane" : "Collapse lane"}
        >
          {collapsed ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
        </button>
      </div>

      {/* Lane body */}
      {!collapsed && (
        <div className="px-1.5 pb-1.5 space-y-1.5 min-h-[2rem]">
          {sorted.length === 0 ? (
            <p className="text-copy-13 text-ds-gray-700 py-2 pl-1 opacity-50">No items</p>
          ) : (
            sorted.map((o) => (
              <KanbanCard
                key={o.id}
                obligation={o}
                isExpanded={expandedId === o.id}
                onSelect={onSelect}
                onRefresh={onRefresh}
                approachingDeadlineHours={approachingDeadlineHours}
                isDragging={draggingId === o.id}
                onDragStart={handleDragStart}
              />
            ))
          )}
        </div>
      )}

      {/* Collapsed drag target — still accepts drops when collapsed */}
      {collapsed && (
        <div className="px-2.5 pb-1.5">
          <p className="text-[10px] text-ds-gray-700 opacity-50 font-mono">
            {obligations.length} item{obligations.length !== 1 ? "s" : ""} hidden
          </p>
        </div>
      )}
    </div>
  );
}
