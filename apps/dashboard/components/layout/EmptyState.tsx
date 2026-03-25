import type { ReactNode } from "react";
import { Inbox } from "lucide-react";

export interface EmptyStateProps {
  title?: string;
  description?: string;
  icon?: ReactNode;
  action?: ReactNode;
}

/**
 * EmptyState — centered icon + message + optional CTA.
 * Consistent across all empty pages in the dashboard.
 */
export default function EmptyState({
  title = "Nothing here yet",
  description = "Data will appear here once it becomes available.",
  icon,
  action,
}: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center gap-4 py-16 px-6 text-center">
      <div className="flex items-center justify-center w-14 h-14 rounded-full bg-cosmic-surface border border-cosmic-border text-cosmic-muted">
        {icon ?? <Inbox size={24} aria-hidden="true" />}
      </div>

      <div className="space-y-1">
        <h3 className="text-sm font-semibold text-cosmic-bright">{title}</h3>
        <p className="text-sm text-cosmic-muted max-w-xs">{description}</p>
      </div>

      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
