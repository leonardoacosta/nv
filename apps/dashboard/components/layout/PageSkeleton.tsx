export type PageSkeletonVariant = "stat-grid" | "list" | "detail";

interface PageSkeletonProps {
  variant?: PageSkeletonVariant;
}

/**
 * PageSkeleton — shimmer skeleton matching target content shape.
 * Variants: stat-grid (overview pages), list (message/obligations),
 * detail (session/memory item pages).
 */
export default function PageSkeleton({ variant = "stat-grid" }: PageSkeletonProps) {
  return (
    <div className="flex flex-col h-full">
      {/* Header skeleton */}
      <div className="flex items-start justify-between gap-4 px-6 py-5 border-b border-ds-gray-400 shrink-0">
        <div className="space-y-2">
          <div className="h-5 w-40 animate-shimmer rounded" />
          <div className="h-3.5 w-64 animate-shimmer rounded" />
        </div>
        <div className="h-9 w-28 animate-shimmer rounded-md shrink-0" />
      </div>

      {/* Content skeleton */}
      <div className="flex-1 overflow-hidden px-6 py-6">
        <div className="max-w-6xl mx-auto w-full space-y-4">

          {variant === "stat-grid" && (
            <>
              {/* Stat cards row */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                {Array.from({ length: 4 }).map((_, i) => (
                  <div
                    key={i}
                    className="h-28 animate-shimmer rounded-xl border border-ds-gray-alpha-200"
                  />
                ))}
              </div>

              {/* Section label */}
              <div className="h-3 w-24 animate-shimmer rounded mt-6" />

              {/* Content rows */}
              {Array.from({ length: 5 }).map((_, i) => (
                <div
                  key={i}
                  className="h-16 animate-shimmer rounded-xl border border-ds-gray-alpha-200"
                  style={{ opacity: 1 - i * 0.12 }}
                />
              ))}
            </>
          )}

          {variant === "list" && (
            <>
              {/* Filter/search bar */}
              <div className="h-10 w-full animate-shimmer rounded-md" />

              {/* List rows */}
              {Array.from({ length: 8 }).map((_, i) => (
                <div
                  key={i}
                  className="h-14 animate-shimmer rounded-lg border border-ds-gray-alpha-200"
                  style={{ opacity: 1 - i * 0.08 }}
                />
              ))}
            </>
          )}

          {variant === "detail" && (
            <>
              {/* Detail card */}
              <div className="h-32 animate-shimmer rounded-xl border border-ds-gray-alpha-200" />

              {/* Sub-section label */}
              <div className="h-3 w-20 animate-shimmer rounded mt-4" />

              {/* Detail rows */}
              {Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="h-10 animate-shimmer rounded-md border border-ds-gray-alpha-200"
                  style={{ opacity: 1 - i * 0.1 }}
                />
              ))}
            </>
          )}

        </div>
      </div>
    </div>
  );
}
