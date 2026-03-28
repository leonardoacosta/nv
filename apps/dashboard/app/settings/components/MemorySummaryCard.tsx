"use client";

import Link from "next/link";

interface MemorySummaryData {
  count: number;
  topics: string[];
  lastWriteAt: string | null;
  totalSizeBytes: number;
}

interface MemorySummaryCardProps {
  data: MemorySummaryData;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function formatRelative(iso: string | null): string {
  if (!iso) return "never";
  const d = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  const diffDay = Math.floor(diffHr / 24);
  return `${diffDay}d ago`;
}

export default function MemorySummaryCard({ data }: MemorySummaryCardProps) {
  const topicsToShow = data.topics.slice(0, 12);
  const hasMore = data.topics.length > 12;

  return (
    <div className="p-4 space-y-3" data-testid="memory-summary-card">
      {/* Stats row */}
      <div className="flex items-center gap-6 text-copy-13">
        <div className="space-y-0.5">
          <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest font-semibold">
            Entries
          </p>
          <p className="text-label-13 text-ds-gray-1000 font-mono" data-testid="memory-entry-count">
            {data.count.toLocaleString()}
          </p>
        </div>
        <div className="space-y-0.5">
          <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest font-semibold">
            Last write
          </p>
          <p className="text-label-13 text-ds-gray-1000 font-mono" suppressHydrationWarning>
            {formatRelative(data.lastWriteAt)}
          </p>
        </div>
        <div className="space-y-0.5">
          <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest font-semibold">
            Total size
          </p>
          <p className="text-label-13 text-ds-gray-1000 font-mono">
            {formatBytes(data.totalSizeBytes)}
          </p>
        </div>
      </div>

      {/* Topics */}
      {data.topics.length > 0 && (
        <div className="space-y-1.5">
          <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest font-semibold">
            Topics
          </p>
          <div className="flex flex-wrap gap-1.5">
            {topicsToShow.map((topic) => (
              <Link
                key={topic}
                href={`/memory?topic=${encodeURIComponent(topic)}`}
                data-testid="memory-topic-chip"
                className="inline-flex px-2 py-0.5 rounded-full text-[11px] font-mono bg-ds-gray-alpha-200 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors"
              >
                {topic}
              </Link>
            ))}
            {hasMore && (
              <Link
                href="/memory"
                className="inline-flex px-2 py-0.5 rounded-full text-[11px] font-mono text-ds-gray-700 hover:text-ds-gray-900 transition-colors"
              >
                +{data.topics.length - 12} more
              </Link>
            )}
          </div>
        </div>
      )}

      {/* View all link */}
      <Link
        href="/memory"
        data-testid="memory-view-all-link"
        className="text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000 underline decoration-ds-gray-400 hover:decoration-ds-gray-700 transition-colors"
      >
        View all topics
      </Link>
    </div>
  );
}
