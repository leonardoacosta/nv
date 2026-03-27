"use client";

import { CheckCircle, XCircle } from "lucide-react";

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
        <button
          type="button"
          onClick={onClearSelection}
          disabled={busy}
          className="px-3 py-2 min-h-9 rounded-lg text-xs font-medium text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          Clear
        </button>
        <button
          type="button"
          onClick={onDismissAll}
          disabled={busy}
          className="flex items-center gap-1.5 px-3 py-2 min-h-9 rounded-lg text-xs font-semibold bg-ds-gray-100 hover:bg-red-700/20 text-ds-gray-900 hover:text-red-700 border border-ds-gray-400 hover:border-red-700/40 transition-colors disabled:opacity-50"
        >
          <XCircle size={14} />
          Dismiss All Selected
        </button>
        <button
          type="button"
          onClick={onApproveAll}
          disabled={busy}
          className="flex items-center gap-1.5 px-3 py-2 min-h-9 rounded-lg text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white transition-colors disabled:opacity-50"
        >
          <CheckCircle size={14} />
          Approve All Selected
        </button>
      </div>
    </div>
  );
}
