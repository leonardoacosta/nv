"use client";

import { useCallback, useState } from "react";
import type { DaemonObligation } from "@/types/api";
import { apiFetch } from "@/lib/api-client";
import KanbanColumn from "@/components/obligations/KanbanColumn";
import EmptyState from "@/components/layout/EmptyState";
import { LayoutGrid } from "lucide-react";

export interface KanbanBoardProps {
  obligations: DaemonObligation[];
  onRefresh: () => void;
  approachingDeadlineHours?: number;
}

export default function KanbanBoard({
  obligations,
  onRefresh,
  approachingDeadlineHours = 24,
}: KanbanBoardProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [forceCreateOwner, setForceCreateOwner] = useState<"nova" | "leo" | null>(null);

  // Sort by priority ascending
  const sortByPriority = (items: DaemonObligation[]) =>
    [...items].sort((a, b) => a.priority - b.priority);

  const novaObligations = sortByPriority(obligations.filter((o) => o.owner === "nova"));
  const leoObligations = sortByPriority(obligations.filter((o) => o.owner === "leo"));
  const otherObligations = sortByPriority(
    obligations.filter((o) => o.owner !== "nova" && o.owner !== "leo"),
  );

  const handleSelect = useCallback((id: string) => {
    setExpandedId((prev) => (prev === id ? null : id));
  }, []);

  // Handle status change within same column
  const handleStatusChange = useCallback(
    async (obligationId: string, newStatus: string) => {
      // Find current obligation to check if status actually changed
      const current = obligations.find((o) => o.id === obligationId);
      if (!current || current.status === newStatus) return;

      // Optimistic update: trigger refresh after patch
      try {
        const res = await apiFetch(`/api/obligations/${obligationId}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ status: newStatus }),
        });
        if (res.ok) onRefresh();
      } catch {
        // Refresh anyway to reconcile state
        onRefresh();
      }
    },
    [obligations, onRefresh],
  );

  // Cross-column drop: nova column receives a leo card (owner change)
  const handleOwnerChange = useCallback(
    async (obligationId: string, newOwner: "nova" | "leo") => {
      const current = obligations.find((o) => o.id === obligationId);
      if (!current || current.owner === newOwner) return;

      try {
        const res = await apiFetch(`/api/obligations/${obligationId}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ owner: newOwner }),
        });
        if (res.ok) onRefresh();
      } catch {
        onRefresh();
      }
    },
    [obligations, onRefresh],
  );

  // Wrap status change to also handle cross-column drops
  // The KanbanColumn's onStatusChange is used for within-column lane drops.
  // Cross-column drops are handled at the board level by each column's drag events.

  const handleNovaDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      const id = e.dataTransfer.getData("obligationId");
      const droppedObligation = obligations.find((o) => o.id === id);
      if (!id || !droppedObligation) return;

      // If card belongs to Leo, change owner to Nova
      if (droppedObligation.owner === "leo") {
        await handleOwnerChange(id, "nova");
      }
      setDraggingId(null);
    },
    [obligations, handleOwnerChange],
  );

  const handleLeoDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      const id = e.dataTransfer.getData("obligationId");
      const droppedObligation = obligations.find((o) => o.id === id);
      if (!id || !droppedObligation) return;

      // If card belongs to Nova, change owner to Leo
      if (droppedObligation.owner === "nova") {
        await handleOwnerChange(id, "leo");
      }
      setDraggingId(null);
    },
    [obligations, handleOwnerChange],
  );

  const handleDragStart = useCallback((id: string) => {
    setDraggingId(id);
  }, []);

  const handleDragEnd = useCallback(() => {
    setDraggingId(null);
  }, []);

  // Empty board state — both nova and leo have zero obligations
  const isEmpty = novaObligations.length === 0 && leoObligations.length === 0 && otherObligations.length === 0;

  if (isEmpty) {
    return (
      <div className="flex flex-col items-center justify-center py-12 gap-4">
        <EmptyState
          title="No active obligations"
          description="Create an obligation to get started."
          icon={<LayoutGrid size={20} />}
        />
        <div className="flex gap-3">
          <button
            type="button"
            onClick={() => setForceCreateOwner("nova")}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium border border-ds-gray-400 text-ds-gray-1000 hover:bg-ds-gray-alpha-100 transition-colors"
          >
            + Assign to Nova
          </button>
          <button
            type="button"
            onClick={() => setForceCreateOwner("leo")}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium border border-ds-gray-400 text-ds-gray-900 hover:bg-ds-gray-alpha-100 transition-colors"
          >
            + Assign to Leo
          </button>
        </div>
        {/* Hidden columns to receive inline create when triggered from empty state */}
        {forceCreateOwner && (
          <div className="w-full max-w-sm">
            <KanbanColumn
              owner={forceCreateOwner}
              obligations={[]}
              expandedId={null}
              onSelect={handleSelect}
              onRefresh={onRefresh}
              onStatusChange={handleStatusChange}
              approachingDeadlineHours={approachingDeadlineHours}
              draggingId={draggingId}
              forceShowCreate={true}
              onForceCreateShown={() => setForceCreateOwner(null)}
            />
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      onDragEnd={handleDragEnd}
      className="grid grid-cols-1 lg:grid-cols-2 gap-4"
    >
      {/* Nova column */}
      <div
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => void handleNovaDrop(e)}
      >
        <KanbanColumn
          owner="nova"
          obligations={novaObligations}
          expandedId={expandedId}
          onSelect={handleSelect}
          onRefresh={onRefresh}
          onStatusChange={handleStatusChange}
          approachingDeadlineHours={approachingDeadlineHours}
          draggingId={draggingId}
          forceShowCreate={forceCreateOwner === "nova"}
          onForceCreateShown={() => setForceCreateOwner(null)}
        />
      </div>

      {/* Leo column */}
      <div
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => void handleLeoDrop(e)}
      >
        <KanbanColumn
          owner="leo"
          obligations={leoObligations}
          expandedId={expandedId}
          onSelect={handleSelect}
          onRefresh={onRefresh}
          onStatusChange={handleStatusChange}
          approachingDeadlineHours={approachingDeadlineHours}
          draggingId={draggingId}
          forceShowCreate={forceCreateOwner === "leo"}
          onForceCreateShown={() => setForceCreateOwner(null)}
        />
      </div>

      {/* Other owner obligations (if any) */}
      {otherObligations.length > 0 && (
        <div className="lg:col-span-2">
          <details className="group">
            <summary className="flex items-center gap-2 cursor-pointer text-label-12 text-ds-gray-700 py-1 list-none select-none">
              <span>Other ({otherObligations.length})</span>
            </summary>
            <div className="mt-2 grid grid-cols-1 lg:grid-cols-2 gap-3">
              {otherObligations.map((o) => (
                <div key={o.id} className="surface-card pl-4 pr-4 pt-3 pb-2 text-copy-13 text-ds-gray-900">
                  {o.owner}: {o.detected_action}
                </div>
              ))}
            </div>
          </details>
        </div>
      )}
    </div>
  );
}

// Re-export drag start wrapper used by KanbanCard via KanbanLane
// (Not needed here — KanbanLane handles this internally)
