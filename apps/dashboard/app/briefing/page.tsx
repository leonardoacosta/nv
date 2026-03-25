"use client";

import { useEffect, useRef, useState } from "react";
import {
  Sun,
  RefreshCw,
  AlertCircle,
} from "lucide-react";
import { parseBriefingSections } from "@/lib/briefing";
import type {
  BriefingEntry,
  BriefingGetResponse,
  BriefingHistoryGetResponse,
} from "@/types/api";

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
  if (status === "ok") return "bg-emerald-400";
  if (status === "unavailable") return "bg-red-400";
  return "bg-cosmic-muted/50";
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
    <div className="rounded-cosmic bg-cosmic-surface border border-cosmic-border p-5 space-y-2">
      <h3 className="text-sm font-semibold text-cosmic-bright uppercase tracking-wide">
        {title}
      </h3>
      <p className="text-sm text-cosmic-text leading-relaxed whitespace-pre-wrap">
        {body}
      </p>
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div className="space-y-4">
      {Array.from({ length: 4 }).map((_, i) => (
        <div
          key={i}
          className="h-28 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
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
    const res = await fetch("/api/briefing");
    if (res.status === 404) return null;
    if (!res.ok) throw new Error(`GET /api/briefing: HTTP ${res.status}`);
    const data = (await res.json()) as BriefingGetResponse;
    return data.entry;
  };

  const fetchHistory = async (): Promise<BriefingEntry[]> => {
    const res = await fetch("/api/briefing/history?limit=10");
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

  const handleSelectHistory = (id: string) => {
    setSelectedId(id === selectedId ? null : id);
  };

  // 8. Render
  return (
    <div className="p-8 space-y-6 max-w-6xl">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">
            Morning Briefing
          </h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            {displayEntry
              ? formatGeneratedAt(displayEntry.generated_at)
              : "Nova generates a briefing each morning at 7am"}
          </p>
        </div>
        <button
          type="button"
          onClick={handleRefresh}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Update banner */}
      {updateBanner && (
        <div className="flex items-center gap-3 p-3 rounded-cosmic bg-cosmic-purple/10 border border-cosmic-purple/30 text-cosmic-bright text-sm">
          <Sun size={14} className="text-cosmic-purple shrink-0" />
          Briefing updated
        </div>
      )}

      {/* Error banner */}
      {error && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {/* Sources status */}
      {displayEntry &&
        Object.keys(displayEntry.sources_status).length > 0 && (
          <div className="flex flex-wrap gap-2">
            {Object.entries(displayEntry.sources_status).map(
              ([source, status]) => (
                <span
                  key={source}
                  className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-cosmic-surface border border-cosmic-border text-xs text-cosmic-muted"
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
      <div className="flex gap-6 items-start">
        {/* Content panel */}
        <div className="flex-1 min-w-0 space-y-4">
          {loading ? (
            <LoadingSkeleton />
          ) : !displayEntry ? (
            /* Empty state */
            <div className="flex flex-col items-center gap-4 py-20 text-cosmic-muted">
              <div className="flex items-center justify-center w-14 h-14 rounded-full bg-cosmic-surface border border-cosmic-border">
                <Sun size={28} className="text-cosmic-purple/60" />
              </div>
              <div className="text-center space-y-1">
                <p className="text-base font-medium text-cosmic-text">
                  No briefing yet today
                </p>
                <p className="text-sm text-cosmic-muted">
                  Nova generates a briefing each morning at 7am
                </p>
              </div>
            </div>
          ) : (
            <>
              {/* Section cards */}
              <div className="space-y-4">
                {parseBriefingSections(displayEntry.content).map(
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
              {displayEntry.suggested_actions.length > 0 && (
                <div className="space-y-2">
                  <p className="text-xs text-cosmic-muted uppercase tracking-wide font-medium">
                    Suggested Actions
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {displayEntry.suggested_actions.map((action) => {
                      const chipClass =
                        action.status === "completed"
                          ? "bg-emerald-500/10 border-emerald-500/30 text-emerald-400"
                          : action.status === "dismissed"
                            ? "bg-cosmic-surface border-cosmic-border text-cosmic-muted line-through"
                            : "bg-cosmic-purple/10 border-cosmic-purple/30 text-cosmic-bright";
                      return (
                        <span
                          key={action.id}
                          className={`px-3 py-1.5 rounded-full border text-xs font-medium ${chipClass}`}
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

        {/* History rail */}
        {history.length > 0 && (
          <div className="w-52 shrink-0 space-y-1">
            <p className="text-xs text-cosmic-muted uppercase tracking-wide font-medium mb-2 px-1">
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
                    "w-full text-left px-3 py-2 rounded-lg text-xs transition-colors",
                    isSelected
                      ? "bg-cosmic-purple/20 text-cosmic-bright border border-cosmic-purple/30"
                      : "text-cosmic-muted hover:text-cosmic-text hover:bg-cosmic-surface border border-transparent",
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
