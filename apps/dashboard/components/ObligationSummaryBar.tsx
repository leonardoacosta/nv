"use client";

import type { DaemonObligation } from "@/types/api";

const STATUS_CONFIG: {
  key: string;
  label: string;
  dotClass: string;
  badgeClass: string;
}[] = [
  {
    key: "open",
    label: "Open",
    dotClass: "bg-ds-gray-1000",
    badgeClass: "bg-ds-gray-alpha-200 text-ds-gray-1000",
  },
  {
    key: "in_progress",
    label: "In Progress",
    dotClass: "bg-amber-500",
    badgeClass: "bg-amber-500/20 text-amber-500",
  },
  {
    key: "proposed_done",
    label: "Proposed Done",
    dotClass: "bg-blue-500",
    badgeClass: "bg-blue-500/20 text-blue-400",
  },
  {
    key: "done",
    label: "Done",
    dotClass: "bg-green-700",
    badgeClass: "bg-green-700/20 text-green-600",
  },
  {
    key: "dismissed",
    label: "Dismissed",
    dotClass: "bg-ds-gray-700",
    badgeClass: "bg-ds-gray-alpha-100 text-ds-gray-700",
  },
];

interface ObligationSummaryBarProps {
  obligations: DaemonObligation[];
}

export default function ObligationSummaryBar({
  obligations,
}: ObligationSummaryBarProps) {
  const counts: Record<string, number> = {};
  for (const o of obligations) {
    counts[o.status] = (counts[o.status] ?? 0) + 1;
  }

  return (
    <div className="flex items-center gap-3 flex-wrap px-4 py-2.5 rounded-lg bg-ds-gray-100 border border-ds-gray-400">
      <span className="text-label-12 text-ds-gray-700 shrink-0">
        {obligations.length} Total
      </span>
      <div
        className="w-px h-4 bg-ds-gray-400 shrink-0"
        aria-hidden="true"
      />
      {STATUS_CONFIG.map(({ key, label, dotClass, badgeClass }) => {
        const count = counts[key] ?? 0;
        return (
          <div key={key} className="flex items-center gap-1.5">
            <div
              className={`w-1.5 h-1.5 rounded-full ${dotClass}`}
              aria-hidden="true"
            />
            <span className={`text-xs font-mono px-1.5 py-0.5 rounded ${badgeClass}`}>
              {count}
            </span>
            <span className="text-xs text-ds-gray-900">{label}</span>
          </div>
        );
      })}
    </div>
  );
}
