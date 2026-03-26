"use client";

import { ShieldCheck } from "lucide-react";

/**
 * Renders a brief "All clear" celebration when the queue empties.
 * Uses the global animate-fade-in-up utility with a longer 800ms duration
 * applied via inline style override on the animation-duration.
 */
export default function QueueClearCelebration() {
  return (
    <div
      className="animate-fade-in-up flex flex-col items-center justify-center gap-4 py-16 text-center"
      style={{ animationDuration: "800ms" }}
    >
      <div className="flex items-center justify-center w-16 h-16 rounded-2xl bg-emerald-600/10 border border-emerald-600/20">
        <ShieldCheck size={32} className="text-emerald-600" />
      </div>
      <div>
        <h3 className="text-lg font-semibold text-ds-gray-1000">All clear</h3>
        <p className="text-sm text-ds-gray-900 mt-1">
          Every approval has been actioned. Nice work.
        </p>
      </div>
    </div>
  );
}
