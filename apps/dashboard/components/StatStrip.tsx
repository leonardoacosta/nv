"use client";

import type { ReactNode } from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface StatCell {
  icon: ReactNode;
  label: string;
  value: string;
  sublabel?: string;
}

interface StatStripProps {
  cells: StatCell[];
}

// ---------------------------------------------------------------------------
// StatStrip — horizontal flex row of stat cells separated by border-r
// ---------------------------------------------------------------------------

export default function StatStrip({ cells }: StatStripProps) {
  return (
    <div className="flex flex-wrap border border-ds-gray-400 rounded-lg overflow-hidden">
      {cells.map((cell, i) => (
        <div
          key={i}
          className="flex items-center gap-2.5 flex-1 min-w-[140px] px-4 py-2 border-r border-ds-gray-400 last:border-r-0"
        >
          <div className="shrink-0 text-ds-gray-700" aria-hidden="true">
            {cell.icon}
          </div>
          <div className="min-w-0">
            <div className="text-label-12 text-ds-gray-700">{cell.label}</div>
            <div className="text-heading-16 text-ds-gray-1000 font-mono tabular-nums leading-tight">
              {cell.value}
            </div>
            {cell.sublabel && (
              <div className="text-label-12 text-ds-gray-700 truncate">{cell.sublabel}</div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
