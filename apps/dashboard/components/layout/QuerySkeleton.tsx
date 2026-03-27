"use client";

/**
 * QuerySkeleton — reusable loading placeholder for query-based data.
 * Renders pulse-animated rows to match the loading state pattern.
 */

interface QuerySkeletonProps {
  /** Number of skeleton rows to render. Default: 5 */
  rows?: number;
  /** Height class for each row. Default: "h-16" */
  height?: string;
}

export default function QuerySkeleton({
  rows = 5,
  height = "h-16",
}: QuerySkeletonProps) {
  return (
    <div className="flex flex-col gap-2">
      {Array.from({ length: rows }).map((_, i) => (
        <div
          key={i}
          className={`${height} animate-shimmer rounded-xl border border-ds-gray-alpha-200`}
          style={{ opacity: 1 - i * 0.1 }}
        />
      ))}
    </div>
  );
}
