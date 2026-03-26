"use client";

import { Save, RotateCcw, X } from "lucide-react";

interface SaveRestartBarProps {
  dirtyCount: number;
  saving: boolean;
  onSaveRestart: () => void;
  onDiscard: () => void;
}

export default function SaveRestartBar({
  dirtyCount,
  saving,
  onSaveRestart,
  onDiscard,
}: SaveRestartBarProps) {
  if (dirtyCount === 0) return null;

  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-40 surface-raised flex items-center gap-4 px-5 py-3 shadow-lg animate-fade-in-up">
      <div className="flex items-center gap-2">
        <RotateCcw size={14} className="text-amber-700" />
        <span className="text-copy-14 text-ds-gray-1000">
          Restart required
        </span>
        <span className="inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 rounded-full bg-amber-700/20 text-amber-700 text-xs font-medium font-mono">
          {dirtyCount}
        </span>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <button
          type="button"
          onClick={onDiscard}
          disabled={saving}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <X size={12} />
          Discard
        </button>
        <button
          type="button"
          onClick={onSaveRestart}
          disabled={saving}
          className="flex items-center gap-2 px-4 py-1.5 rounded-lg text-button-14 font-medium bg-amber-700 text-white hover:bg-amber-700/80 transition-colors disabled:opacity-50"
        >
          <Save size={14} />
          {saving ? "Saving..." : "Save & Restart"}
        </button>
      </div>
    </div>
  );
}
