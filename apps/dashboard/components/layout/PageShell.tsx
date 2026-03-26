import type { ReactNode } from "react";

export interface PageShellProps {
  title: string;
  subtitle?: string;
  action?: ReactNode;
  children: ReactNode;
}

/**
 * PageShell — standard page wrapper with header (title, subtitle, action slot)
 * and a max-width content container. Used by every dashboard page.
 * Geist type scale: text-heading-24 for title, text-copy-14 for subtitle.
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
        className="flex items-start justify-between gap-4 px-6 py-5 shrink-0"
        style={{ borderBottom: "1px solid var(--ds-gray-alpha-200)" }}
      >
        <div className="min-w-0">
          <h1 className="text-heading-24 text-ds-gray-1000 leading-tight truncate">
            {title}
          </h1>
          {subtitle && (
            <p className="mt-1 text-copy-14 text-ds-gray-900 truncate">
              {subtitle}
            </p>
          )}
        </div>

        {action && (
          <div className="shrink-0 flex items-center gap-2">{action}</div>
        )}
      </header>

      {/* Page content */}
      <div className="flex-1 overflow-auto px-6 py-6">
        <div className="max-w-6xl mx-auto w-full animate-fade-in-up">
          {children}
        </div>
      </div>
    </div>
  );
}
