"use client";

import { useEffect, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

interface SubGroupProps {
  id: string;
  title: string;
  children: React.ReactNode;
  defaultExpanded?: boolean;
}

const STORAGE_PREFIX = "nv-settings-subgroup-";

export default function SubGroup({
  id,
  title,
  children,
  defaultExpanded = true,
}: SubGroupProps) {
  const [open, setOpen] = useState<boolean>(() => {
    if (typeof window === "undefined") return defaultExpanded;
    const stored = localStorage.getItem(`${STORAGE_PREFIX}${id}`);
    return stored !== null ? stored === "true" : defaultExpanded;
  });

  useEffect(() => {
    localStorage.setItem(`${STORAGE_PREFIX}${id}`, String(open));
  }, [id, open]);

  return (
    <div className="pl-2">
      {/* Hairline separator + collapsible header */}
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center gap-2 px-2 py-2 text-left hover:bg-ds-gray-alpha-100 transition-colors rounded"
        style={{ borderTop: "1px solid var(--ds-gray-alpha-200)" }}
      >
        <div className="text-ds-gray-700 shrink-0">
          {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </div>
        <span className="text-label-13 text-ds-gray-900 font-medium">{title}</span>
      </button>

      {open && (
        <div className="divide-y divide-ds-gray-alpha-200">
          {children}
        </div>
      )}
    </div>
  );
}
