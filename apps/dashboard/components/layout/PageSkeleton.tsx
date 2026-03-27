import { Skeleton } from "@nova/ui";

export type PageSkeletonVariant = "stat-grid" | "list" | "detail";

interface PageSkeletonProps {
  variant?: PageSkeletonVariant;
}

/**
 * PageSkeleton — shimmer skeleton matching target content shape.
 * Variants: stat-grid (overview pages), list (message/obligations),
 * detail (session/memory item pages).
 *
 * Uses the shadcn Skeleton component from @nova/ui which leverages
 * the existing animate-shimmer gradient.
 */
export default function PageSkeleton({ variant = "stat-grid" }: PageSkeletonProps) {
  return (
    <div className="flex flex-col h-full">
      {/* Header skeleton */}
      <div className="flex items-start justify-between gap-4 px-6 py-5 border-b border-ds-gray-400 shrink-0">
        <div className="flex flex-col gap-2">
          <Skeleton className="h-5 w-40" />
          <Skeleton className="h-3.5 w-64" />
        </div>
        <Skeleton className="h-9 w-28 rounded-md shrink-0" />
      </div>

      {/* Content skeleton */}
      <div className="flex-1 overflow-hidden px-6 py-4">
        <div className="max-w-6xl mx-auto w-full flex flex-col gap-4">

          {variant === "stat-grid" && (
            <>
              {/* Stat cards row */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                {Array.from({ length: 4 }).map((_, i) => (
                  <Skeleton
                    key={i}
                    className="h-28 rounded-xl border border-ds-gray-alpha-200"
                  />
                ))}
              </div>

              {/* Section label */}
              <Skeleton className="h-3 w-24 mt-6" />

              {/* Content rows */}
              {Array.from({ length: 5 }).map((_, i) => (
                <Skeleton
                  key={i}
                  className="h-16 rounded-xl border border-ds-gray-alpha-200"
                  style={{ opacity: 1 - i * 0.12 }}
                />
              ))}
            </>
          )}

          {variant === "list" && (
            <>
              {/* Filter/search bar */}
              <Skeleton className="h-10 w-full rounded-md" />

              {/* List rows */}
              {Array.from({ length: 8 }).map((_, i) => (
                <Skeleton
                  key={i}
                  className="h-14 rounded-lg border border-ds-gray-alpha-200"
                  style={{ opacity: 1 - i * 0.08 }}
                />
              ))}
            </>
          )}

          {variant === "detail" && (
            <>
              {/* Detail card */}
              <Skeleton className="h-32 rounded-xl border border-ds-gray-alpha-200" />

              {/* Sub-section label */}
              <Skeleton className="h-3 w-20 mt-4" />

              {/* Detail rows */}
              {Array.from({ length: 6 }).map((_, i) => (
                <Skeleton
                  key={i}
                  className="h-10 rounded-md border border-ds-gray-alpha-200"
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
