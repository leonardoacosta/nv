/**
 * PageSkeleton — animated pulse skeleton that fills the page area.
 * Mimics the PageShell structure: a header bar and a grid of content tiles.
 */
export default function PageSkeleton() {
  return (
    <div className="flex flex-col h-full">
      {/* Header skeleton */}
      <div className="flex items-start justify-between gap-4 px-6 py-5 border-b border-cosmic-border shrink-0">
        <div className="space-y-2">
          <div className="h-5 w-40 animate-pulse rounded bg-cosmic-border" />
          <div className="h-3.5 w-64 animate-pulse rounded bg-cosmic-border" />
        </div>
        <div className="h-9 w-28 animate-pulse rounded-lg bg-cosmic-border shrink-0" />
      </div>

      {/* Content skeleton */}
      <div className="flex-1 overflow-hidden px-6 py-6">
        <div className="max-w-6xl mx-auto w-full space-y-4">
          {/* Stat cards row */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            {Array.from({ length: 4 }).map((_, i) => (
              <div
                key={i}
                className="h-24 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
              />
            ))}
          </div>

          {/* Main content rows */}
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-16 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
              style={{ opacity: 1 - i * 0.12 }}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
