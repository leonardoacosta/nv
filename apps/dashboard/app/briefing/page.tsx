"use client";

import { useEffect, useRef, useState } from "react";
import {
  Sun,
  RefreshCw,
  Zap,
  Loader2,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import { parseBriefingSections } from "@/lib/briefing";
import ErrorBanner from "@/components/layout/ErrorBanner";
import ErrorBoundary from "@/components/layout/ErrorBoundary";
import type {
  BriefingEntry,
  BriefingGetResponse,
  BriefingHistoryGetResponse,
} from "@/types/api";
import { apiFetch } from "@/lib/api-client";

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
      <h3 className="text-label-12 text-ds-gray-700">
        {title}
      </h3>
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

// ── Main page ─────────────────────────────────────────────────────────────────

export default function BriefingPage() {
  // 1. State
  const [entry, setEntry] = useState<BriefingEntry | null>(null);
  const [history, setHistory] = useState<BriefingEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [updateBanner, setUpdateBanner] = useState(false);
  const [generating, setGenerating] = useState(false);

  // Ref for cleanup
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const bannerTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Derived: the displayed entry is selectedId from history, or the latest
  const displayEntry =
    selectedId !== null
      ? (history.find((e) => e.id === selectedId) ?? entry)
      : entry;

  // 2. Fetch functions
  const fetchLatest = async (): Promise<BriefingEntry | null> => {
    const res = await apiFetch("/api/briefing");
    if (res.status === 404) return null;
    if (!res.ok) throw new Error(`GET /api/briefing: HTTP ${res.status}`);
    const data = (await res.json()) as BriefingGetResponse;
    return data.entry;
  };

  const fetchHistory = async (): Promise<BriefingEntry[]> => {
    const res = await apiFetch("/api/briefing/history?limit=10");
    if (!res.ok) return [];
    const data = (await res.json()) as BriefingHistoryGetResponse;
    return data.entries;
  };

  // 3. Initial load
  const loadAll = async () => {
    setLoading(true);
    setError(null);
    try {
      const [latestResult, histResult] = await Promise.allSettled([
        fetchLatest(),
        fetchHistory(),
      ]);

      if (latestResult.status === "fulfilled") {
        setEntry(latestResult.value);
      } else {
        setError(
          latestResult.reason instanceof Error
            ? latestResult.reason.message
            : "Failed to load briefing",
        );
      }

      if (histResult.status === "fulfilled") {
        setHistory(histResult.value);
      }
    } finally {
      setLoading(false);
    }
  };

  // 4. Effects — initial load
  useEffect(() => {
    void loadAll();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // 5. Effects — 60s polling (auto-refresh)
  useEffect(() => {
    intervalRef.current = setInterval(() => {
      void (async () => {
        try {
          const latest = await fetchLatest();
          if (latest && latest.generated_at !== entry?.generated_at) {
            setEntry(latest);
            // Also refresh history so the rail stays current
            const hist = await fetchHistory();
            setHistory(hist);
            // Show update banner
            setUpdateBanner(true);
            if (bannerTimerRef.current) clearTimeout(bannerTimerRef.current);
            bannerTimerRef.current = setTimeout(
              () => setUpdateBanner(false),
              4000,
            );
          }
        } catch {
          // Silent — don't surface auto-refresh errors
        }
      })();
    }, 60_000);

    // 6. Cleanup on unmount
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
      if (bannerTimerRef.current) clearTimeout(bannerTimerRef.current);
    };
  }, [entry?.generated_at]); // eslint-disable-line react-hooks/exhaustive-deps

  // 7. Handlers
  const handleRefresh = () => {
    setSelectedId(null);
    void loadAll();
  };

  const handleGenerate = async () => {
    setGenerating(true);
    setError(null);
    try {
      const res = await apiFetch("/api/briefing/generate", { method: "POST" });
      if (!res.ok) {
        const body = (await res.json().catch(() => ({}))) as { error?: string };
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      setSelectedId(null);
      await loadAll();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to generate briefing",
      );
    } finally {
      setGenerating(false);
    }
  };

  const handleSelectHistory = (id: string) => {
    setSelectedId(id === selectedId ? null : id);
  };

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

      {/* Update banner */}
      {updateBanner && (
        <div className="flex items-center gap-3 p-3 rounded-xl bg-ds-gray-alpha-100 border border-ds-gray-1000/30 text-ds-gray-1000 text-copy-13">
          <Sun size={14} className="text-ds-gray-1000 shrink-0" />
          Briefing updated
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
            ) : !displayEntry ? (
              /* Empty state */
              <p className="text-copy-13 text-ds-gray-900 py-3">No briefing yet today</p>
            ) : (
              <>
                {/* Section cards */}
                <div className="space-y-4">
                  {(displayEntry.content ? parseBriefingSections(displayEntry.content) : []).map(
                    (section, idx) => (
                      <BriefingSectionCard
                        key={idx}
                        title={section.title}
                        body={section.body}
                      />
                    ),
                  )}
                </div>

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
