"use client";

import { useEffect, useRef, useState } from "react";
import {
  Sun,
  RefreshCw,
  Zap,
  Loader2,
  AlertTriangle,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import { parseBriefingSections } from "@/lib/briefing";
import ErrorBanner from "@/components/layout/ErrorBanner";
import ErrorBoundary from "@/components/layout/ErrorBoundary";
import type {
  BriefingEntry,
  BriefingGetResponse,
} from "@/types/api";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTRPC } from "@/lib/trpc/react";
import { BriefingRenderer } from "@/components/blocks/BlockRegistry";
import type { BriefingBlock } from "@nova/db";

// ── Helpers ──────────────────────────────────────────────────────────────────

function formatGeneratedAt(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const isToday =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();

  const time = date.toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });

  if (isToday) return `Today, ${time}`;

  return (
    date.toLocaleDateString([], {
      weekday: "short",
      month: "short",
      day: "numeric",
    }) +
    ", " +
    time
  );
}

function formatHistoryDate(iso: string): string {
  const date = new Date(iso);
  return (
    date.toLocaleDateString([], {
      weekday: "short",
      month: "short",
      day: "numeric",
    }) +
    ", " +
    date.toLocaleTimeString([], {
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    })
  );
}

function sourceStatusColor(status: string): string {
  if (status === "ok") return "bg-green-700";
  if (status === "unavailable") return "bg-red-700";
  return "bg-ds-gray-500";
}

function getNextBriefingTime(): string {
  const now = new Date();
  if (now.getHours() < 7) return "Today at 7:00 AM";
  return "Tomorrow at 7:00 AM";
}

// ── Sub-components ────────────────────────────────────────────────────────────

function BriefingSectionCard({
  title,
  body,
}: {
  title: string;
  body: string;
}) {
  return (
    <div className="surface-card p-5 space-y-2">
      <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
      <div className="prose prose-sm prose-invert max-w-none text-copy-14 text-ds-gray-1000 leading-relaxed">
        <ReactMarkdown>{body}</ReactMarkdown>
      </div>
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div className="space-y-4">
      {Array.from({ length: 4 }).map((_, i) => (
        <div
          key={i}
          className="h-28 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
        />
      ))}
    </div>
  );
}

function StreamingSkeleton({ count }: { count: number }) {
  return (
    <div className="space-y-4">
      {Array.from({ length: count }).map((_, i) => (
        <div
          key={i}
          className="h-24 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400 opacity-60"
        />
      ))}
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

export default function BriefingPage() {
  const trpc = useTRPC();

  // 1. State
  const [entry, setEntry] = useState<BriefingEntry | null>(null);
  const [history, setHistory] = useState<BriefingEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [updateBanner, setUpdateBanner] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [missedToday, setMissedToday] = useState(false);
  const [missedDismissed, setMissedDismissed] = useState(false);

  // Streaming state
  const [streamingBlocks, setStreamingBlocks] = useState<BriefingBlock[]>([]);
  const [streamingError, setStreamingError] = useState<string | null>(null);
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamExpectedCount, setStreamExpectedCount] = useState(4);

  // Refs for cleanup
  const bannerTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const eventSourceRef = useRef<EventSource | null>(null);

  // Derived: the displayed entry is selectedId from history, or the latest
  const displayEntry =
    selectedId !== null
      ? (history.find((e) => e.id === selectedId) ?? entry)
      : entry;

  const queryClient = useQueryClient();

  // 2. Queries
  const latestQuery = useQuery(
    trpc.briefing.latest.queryOptions(undefined, { refetchInterval: 60_000 }),
  );
  const historyQuery = useQuery(
    trpc.briefing.history.queryOptions({ limit: 10 }),
  );

  const generateMutation = useMutation(
    trpc.briefing.generate.mutationOptions({
      onSuccess: () => {
        void queryClient.invalidateQueries({ queryKey: trpc.briefing.latest.queryKey() });
        void queryClient.invalidateQueries({ queryKey: trpc.briefing.history.queryKey() });
      },
    }),
  );

  // 3. Sync query data to local state
  useEffect(() => {
    if (latestQuery.data) {
      const data = latestQuery.data as BriefingGetResponse;
      if (data.entry && data.entry.generated_at !== entry?.generated_at) {
        setEntry(data.entry);
        setUpdateBanner(true);
        if (bannerTimerRef.current) clearTimeout(bannerTimerRef.current);
        bannerTimerRef.current = setTimeout(() => setUpdateBanner(false), 4000);
      } else if (data.entry) {
        setEntry(data.entry);
      }
      setMissedToday(data.missedToday ?? false);
      setLoading(false);
    }
    if (latestQuery.error) {
      setError(latestQuery.error.message);
      setLoading(false);
    }
  }, [latestQuery.data, latestQuery.error]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (historyQuery.data) {
      setHistory(historyQuery.data.entries as unknown as BriefingEntry[]);
    }
  }, [historyQuery.data]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (bannerTimerRef.current) clearTimeout(bannerTimerRef.current);
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
        eventSourceRef.current = null;
      }
    };
  }, []);

  // 7. Handlers
  const handleRefresh = () => {
    setSelectedId(null);
    void queryClient.invalidateQueries({ queryKey: trpc.briefing.latest.queryKey() });
    void queryClient.invalidateQueries({ queryKey: trpc.briefing.history.queryKey() });
  };

  const handleGenerate = async () => {
    // Close any existing SSE connection
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }

    setGenerating(true);
    setError(null);
    setStreamingError(null);
    setStreamingBlocks([]);
    setStreamExpectedCount(4);

    // Attempt SSE stream from daemon
    const DAEMON_URL =
      typeof window !== "undefined"
        ? (process.env.NEXT_PUBLIC_DAEMON_URL ?? "http://localhost:7700")
        : "http://localhost:7700";

    try {
      const es = new EventSource(`${DAEMON_URL}/api/briefing/stream`);
      eventSourceRef.current = es;
      setIsStreaming(true);
      setSelectedId(null);

      es.onmessage = (event: MessageEvent<string>) => {
        try {
          const parsed = JSON.parse(event.data) as
            | { type: "block"; index: number; block: BriefingBlock }
            | { type: "done"; blocks: BriefingBlock[] }
            | { type: "error"; message: string };

          if (parsed.type === "block") {
            setStreamingBlocks((prev) => [...prev, parsed.block]);
          } else if (parsed.type === "done") {
            setStreamingBlocks(parsed.blocks);
            setIsStreaming(false);
            es.close();
            eventSourceRef.current = null;
            setGenerating(false);
            setMissedDismissed(true);
            // Refresh from DB so page shows persisted briefing
            void queryClient.invalidateQueries({ queryKey: trpc.briefing.latest.queryKey() });
            void queryClient.invalidateQueries({ queryKey: trpc.briefing.history.queryKey() });
          } else if (parsed.type === "error") {
            setStreamingError(parsed.message);
            setIsStreaming(false);
            es.close();
            eventSourceRef.current = null;
            setGenerating(false);
            // Fall back to mutation-based generate
            setStreamingBlocks([]);
          }
        } catch {
          // Malformed SSE data — ignore individual bad events
        }
      };

      es.onerror = () => {
        // SSE failed — fall back to tRPC mutation
        es.close();
        eventSourceRef.current = null;
        setIsStreaming(false);
        setStreamingBlocks([]);

        generateMutation.mutate(undefined, {
          onSuccess: () => {
            setGenerating(false);
            setSelectedId(null);
            setMissedDismissed(true);
          },
          onError: (err) => {
            setError(err.message ?? "Failed to generate briefing");
            setGenerating(false);
          },
        });
      };
    } catch {
      // EventSource not available or DAEMON_URL unreachable — fall back to mutation
      setIsStreaming(false);
      try {
        await generateMutation.mutateAsync();
        setSelectedId(null);
        setMissedDismissed(true);
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Failed to generate briefing",
        );
      } finally {
        setGenerating(false);
      }
    }
  };

  const handleSelectHistory = (id: string) => {
    setSelectedId(id === selectedId ? null : id);
    // Clear streaming state when navigating history
    setStreamingBlocks([]);
    setIsStreaming(false);
  };

  // Determine what content to render in the main panel
  const showStreamingBlocks = isStreaming || (generating && streamingBlocks.length > 0);
  const hasStreamedBlocks = !isStreaming && streamingBlocks.length > 0 && selectedId === null;

  // 8. Render
  return (
    <div className="p-4 space-y-3 w-full animate-fade-in-up">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-heading-20 text-ds-gray-1000">
            Morning Briefing
          </h1>
          <p className="mt-0.5 text-copy-13 text-ds-gray-900">
            {displayEntry
              ? formatGeneratedAt(displayEntry.generated_at)
              : "Nova generates a briefing each morning at 7am"}
          </p>
          <p className="mt-0.5 text-copy-13 text-ds-gray-700">
            Next briefing: {getNextBriefingTime()}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void handleGenerate()}
            disabled={generating || loading}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
          >
            {generating ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <Zap size={14} />
            )}
            Generate Now
          </button>
          <button
            type="button"
            onClick={handleRefresh}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
            Refresh
          </button>
        </div>
      </div>

      {/* Missed briefing banner */}
      {missedToday && !missedDismissed && !generating && (
        <div className="flex items-center justify-between gap-3 p-3 rounded-xl bg-ds-amber-700/10 border border-ds-amber-700/30 text-ds-amber-700 text-copy-13">
          <div className="flex items-center gap-2">
            <AlertTriangle size={14} className="shrink-0" />
            No briefing generated today. Generate one now?
          </div>
          <button
            type="button"
            onClick={() => void handleGenerate()}
            disabled={generating}
            className="px-2.5 py-1 rounded-md text-label-12 bg-ds-amber-700/20 hover:bg-ds-amber-700/30 border border-ds-amber-700/40 transition-colors shrink-0 disabled:opacity-50"
          >
            Generate
          </button>
        </div>
      )}

      {/* Update banner */}
      {updateBanner && (
        <div className="flex items-center gap-3 p-3 rounded-xl bg-ds-gray-alpha-100 border border-ds-gray-1000/30 text-ds-gray-1000 text-copy-13">
          <Sun size={14} className="text-ds-gray-1000 shrink-0" />
          Briefing updated
        </div>
      )}

      {/* Streaming error banner */}
      {streamingError && (
        <div className="flex items-center gap-3 p-3 rounded-xl bg-ds-red-700/10 border border-ds-red-700/30 text-ds-red-700 text-copy-13">
          <AlertTriangle size={14} className="shrink-0" />
          Stream error: {streamingError}
        </div>
      )}

      {/* Error banner */}
      {error && (
        <ErrorBanner
          message="Failed to load briefing"
          detail={error}
          onRetry={handleRefresh}
        />
      )}

      {/* Sources status */}
      {displayEntry?.sources_status &&
        Object.keys(displayEntry.sources_status).length > 0 && (
          <div className="flex flex-wrap gap-2">
            {Object.entries(displayEntry.sources_status).map(
              ([source, status]) => (
                <span
                  key={source}
                  className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-900"
                >
                  <span
                    className={`w-2 h-2 rounded-full shrink-0 ${sourceStatusColor(status)}`}
                  />
                  {source}
                </span>
              ),
            )}
          </div>
        )}

      {/* Main grid: content + history rail */}
      <div className="flex gap-4 items-start">
        {/* Content panel */}
        <ErrorBoundary onReset={handleRefresh}>
          <div className="flex-1 min-w-0 space-y-4">
            {loading ? (
              <LoadingSkeleton />
            ) : showStreamingBlocks ? (
              /* Progressive streaming render */
              <>
                {streamingBlocks.length > 0 && (
                  <BriefingRenderer blocks={streamingBlocks} />
                )}
                {isStreaming && (
                  <StreamingSkeleton
                    count={Math.max(1, streamExpectedCount - streamingBlocks.length)}
                  />
                )}
              </>
            ) : hasStreamedBlocks ? (
              /* Streamed blocks before DB refresh completes */
              <BriefingRenderer blocks={streamingBlocks} />
            ) : !displayEntry ? (
              /* Empty state */
              <p className="text-copy-13 text-ds-gray-900 py-3">No briefing yet today</p>
            ) : (
              <>
                {/* Block-based rendering (generative UI) or markdown fallback */}
                {displayEntry.blocks && displayEntry.blocks.length > 0 ? (
                  <BriefingRenderer blocks={displayEntry.blocks} />
                ) : (
                  <div className="space-y-4">
                    {(displayEntry.content
                      ? parseBriefingSections(displayEntry.content)
                      : []
                    ).map((section, idx) => (
                      <BriefingSectionCard
                        key={idx}
                        title={section.title}
                        body={section.body}
                      />
                    ))}
                  </div>
                )}

                {/* Suggested actions chips */}
                {(displayEntry.suggested_actions?.length ?? 0) > 0 && (
                  <div className="space-y-2">
                    <p className="text-label-12 text-ds-gray-700">
                      Suggested Actions
                    </p>
                    <div className="flex flex-wrap gap-2">
                      {displayEntry.suggested_actions.map((action) => {
                        const chipClass =
                          action.status === "completed"
                            ? "bg-green-700/10 border-green-700/30 text-green-700"
                            : action.status === "dismissed"
                              ? "bg-ds-gray-100 border-ds-gray-400 text-ds-gray-900 line-through"
                              : "bg-ds-gray-alpha-100 border-ds-gray-1000/30 text-ds-gray-1000";
                        return (
                          <span
                            key={action.id}
                            className={`px-3 py-1.5 rounded-full border text-label-13 ${chipClass}`}
                          >
                            {action.label}
                          </span>
                        );
                      })}
                    </div>
                  </div>
                )}
              </>
            )}
          </div>
        </ErrorBoundary>

        {/* History rail */}
        {history.length > 0 && (
          <div className="w-52 shrink-0 space-y-1">
            <p className="text-label-12 text-ds-gray-700 mb-2 px-1">
              History
            </p>
            {history.map((h) => {
              const isSelected =
                selectedId === h.id ||
                (selectedId === null && h.id === entry?.id);
              return (
                <button
                  type="button"
                  key={h.id}
                  onClick={() => handleSelectHistory(h.id)}
                  className={[
                    "w-full text-left px-3 py-2 rounded-lg text-copy-13 transition-colors",
                    isSelected
                      ? "bg-ds-gray-alpha-200 text-ds-gray-1000 border border-ds-gray-1000/30"
                      : "text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-100 border border-transparent",
                  ].join(" ")}
                >
                  {formatHistoryDate(h.generated_at)}
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
