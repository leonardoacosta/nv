import type { ReactNode } from "react";

export interface PageShellProps {
  title: string;
  subtitle?: string;
  action?: ReactNode;
  children: ReactNode;
}

/**
 * PageShell — standard page wrapper with header (title, subtitle, action slot)
 * and an edge-to-edge content container. Used by every dashboard page.
 * Geist type scale: text-heading-20 for title, text-copy-13 for subtitle.
 */
export default function PageShell({
  title,
  subtitle,
  action,
  children,
}: PageShellProps) {
  return (
    <div className="flex flex-col h-full">
      {/* Page header */}
      <header
        className="flex items-start justify-between gap-4 px-6 py-3 shrink-0"
        style={{ borderBottom: "1px solid var(--ds-gray-alpha-200)" }}
      >
        <div className="min-w-0">
          <h1 className="text-heading-20 text-ds-gray-1000 leading-tight truncate">
            {title}
          </h1>
          {subtitle && (
            <p className="mt-0.5 text-copy-13 text-ds-gray-900 truncate">
              {subtitle}
            </p>
          )}
        </div>

        {action && (
          <div className="shrink-0 flex items-center gap-2">{action}</div>
        )}
      </header>

      {/* Page content */}
      <div className="flex-1 overflow-auto px-6 py-4">
        <div className="w-full animate-fade-in-up">
          {children}
        </div>
      </div>
    </div>
  );
}
