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
      <header className="flex items-start justify-between gap-4 px-6 py-5 border-b border-cosmic-border shrink-0">
        <div className="min-w-0">
          <h1 className="text-lg font-semibold text-cosmic-bright leading-tight truncate">
            {title}
          </h1>
          {subtitle && (
            <p className="mt-0.5 text-sm text-cosmic-muted truncate">
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
        <div className="max-w-6xl mx-auto w-full">{children}</div>
      </div>
    </div>
  );
}
