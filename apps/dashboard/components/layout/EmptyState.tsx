import type { ReactNode } from "react";
import { Inbox } from "lucide-react";

export interface EmptyStateProps {
  title?: string;
  description?: string;
  icon?: ReactNode;
  action?: ReactNode;
  /** Render as a single inline line with just the title text and no icon */
  inline?: boolean;
}

/**
 * EmptyState — compact icon + message + optional CTA.
 * Consistent across all empty pages in the dashboard.
 * Uses Geist neutral palette — no purple.
 *
 * When `inline` is true, renders as a single muted text line (no icon, no description).
 */
export default function EmptyState({
  title = "Nothing here yet",
  description = "Data will appear here once it becomes available.",
  icon,
  action,
  inline = false,
}: EmptyStateProps) {
  if (inline) {
    return (
      <p className="text-copy-13 text-ds-gray-900 py-3">{title}</p>
    );
  }

  return (
    <div className="flex flex-col items-center justify-center gap-3 py-4 px-4 text-center animate-fade-in-up">
      <div
        className="flex items-center justify-center w-6 h-6 text-ds-gray-600"
        aria-hidden="true"
      >
        {icon ?? <Inbox size={20} aria-hidden="true" />}
      </div>

      <div className="space-y-0.5">
        <h3 className="text-copy-14 font-medium text-ds-gray-1000">{title}</h3>
        <p className="text-copy-13 text-ds-gray-900 max-w-xs">{description}</p>
      </div>

      {action && (
        <div className="mt-1">
          {action}
        </div>
      )}
    </div>
  );
}
