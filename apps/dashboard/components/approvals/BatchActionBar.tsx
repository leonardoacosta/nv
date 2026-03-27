"use client";

import { CheckCircle, XCircle } from "lucide-react";
import { Button } from "@nova/ui";

interface BatchActionBarProps {
  /** Number of items currently checked. */
  selectedCount: number;
  /** Approve all checked items. */
  onApproveAll: () => void;
  /** Dismiss all checked items. */
  onDismissAll: () => void;
  /** Clear the batch selection. */
  onClearSelection: () => void;
  /** Whether any batch action is in-flight. */
  busy: boolean;
}

export default function BatchActionBar({
  selectedCount,
  onApproveAll,
  onDismissAll,
  onClearSelection,
  busy,
}: BatchActionBarProps) {
  if (selectedCount === 0) return null;

  return (
    <div className="animate-fade-in-up sticky bottom-4 mx-4 mb-4 flex items-center gap-3 rounded-xl border border-ds-gray-400 bg-ds-bg-200 px-4 py-3 shadow-lg">
      <span className="text-sm font-medium text-ds-gray-1000">
        {selectedCount} selected
      </span>

      <div className="ml-auto flex items-center gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={onClearSelection}
          disabled={busy}
        >
          Clear
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={onDismissAll}
          disabled={busy}
          className="hover:bg-red-700/20 hover:text-red-700 hover:border-red-700/40"
        >
          <XCircle size={14} />
          Dismiss All Selected
        </Button>
        <Button
          type="button"
          size="sm"
          onClick={onApproveAll}
          disabled={busy}
          className="bg-emerald-600 hover:bg-emerald-500 text-white"
        >
          <CheckCircle size={14} />
          Approve All Selected
        </Button>
      </div>
    </div>
  );
}
