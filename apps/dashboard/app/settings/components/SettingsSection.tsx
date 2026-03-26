"use client";

import { useEffect, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

interface SettingsSectionProps {
  id: string;
  title: string;
  icon: React.ElementType;
  description: string;
  itemCount: number;
  children: React.ReactNode;
  /** Default expanded state on first visit (before localStorage). Defaults to true. */
  defaultExpanded?: boolean;
}

const STORAGE_PREFIX = "nv-settings-section-";

export default function SettingsSection({
  id,
  title,
  icon: Icon,
  description,
  itemCount,
  children,
  defaultExpanded = true,
}: SettingsSectionProps) {
  const [open, setOpen] = useState<boolean>(() => {
    if (typeof window === "undefined") return defaultExpanded;
    const stored = localStorage.getItem(`${STORAGE_PREFIX}${id}`);
    return stored !== null ? stored === "true" : defaultExpanded;
  });

  useEffect(() => {
    localStorage.setItem(`${STORAGE_PREFIX}${id}`, String(open));
  }, [id, open]);

  if (itemCount === 0) {
    return (
      <div className="surface-card overflow-hidden opacity-60">
        <div className="w-full flex items-center gap-3 px-4 py-3.5 min-h-11 text-left">
          <Icon size={15} className="text-ds-gray-700 shrink-0" />
          <div className="flex-1 min-w-0">
            <h2 className="text-label-16 text-ds-gray-1000">{title}</h2>
            <p className="text-copy-13 text-ds-gray-900 mt-0.5">{description}</p>
          </div>
          <span className="text-label-13-mono text-ds-gray-900">0</span>
        </div>
        <div
          className="px-4 py-3"
          style={{ borderTop: "1px solid var(--ds-gray-alpha-200)" }}
        >
          <p className="text-copy-13 text-ds-gray-900 italic">No fields configured.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="surface-card overflow-hidden">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center gap-3 px-4 py-3.5 min-h-11 hover:bg-ds-gray-alpha-100 transition-colors text-left"
      >
        <Icon size={15} className="text-ds-gray-700 shrink-0" />
        <div className="flex-1 min-w-0">
          <h2 className="text-label-16 text-ds-gray-1000">{title}</h2>
        </div>
        <span className="text-label-13-mono text-ds-gray-900">{itemCount}</span>
        <div className="text-ds-gray-700 shrink-0">
          {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </div>
      </button>

      <div className={`height-reveal ${open ? "open" : ""}`}>
        <div>
          <div
            style={{ borderTop: "1px solid var(--ds-gray-alpha-200)" }}
            className="divide-y divide-ds-gray-alpha-200"
          >
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}
