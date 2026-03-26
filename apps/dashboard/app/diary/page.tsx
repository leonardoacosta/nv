"use client";

import { useEffect, useState, useCallback } from "react";
import {
  BookOpen,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  AlertCircle,
  Clock,
  Zap,
} from "lucide-react";
import DiaryEntryCard from "@/components/DiaryEntry";
import type { DiaryGetResponse, DiaryEntryItem } from "@/types/api";

// ── Helpers ──────────────────────────────────────────────────────────────────

function toDateString(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function formatDisplayDate(dateStr: string): string {
  const [y, m, d] = dateStr.split("-").map(Number);
  const date = new Date(y, (m as number) - 1, d as number);
  return date.toLocaleDateString(undefined, {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

function addDays(dateStr: string, delta: number): string {
  const [y, m, d] = dateStr.split("-").map(Number);
  const date = new Date(y, (m as number) - 1, d as number);
  date.setDate(date.getDate() + delta);
  return toDateString(date);
}

function isToday(dateStr: string): boolean {
  return dateStr === toDateString(new Date());
}

// ── Summary stats ────────────────────────────────────────────────────────────

function computeStats(entries: DiaryEntryItem[]) {
  const total = entries.length;
  const totalTokens = entries.reduce(
    (sum, e) => sum + e.tokens_in + e.tokens_out,
    0,
  );
  const avgLatencyMs =
    total === 0
      ? 0
      : Math.round(
          entries.reduce((sum, e) => sum + e.response_latency_ms, 0) / total,
        );
  return { total, totalTokens, avgLatencyMs };
}

// ── Page ─────────────────────────────────────────────────────────────────────

export default function DiaryPage() {
  const [dateStr, setDateStr] = useState<string>(toDateString(new Date()));
  const [data, setData] = useState<DiaryGetResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchDiary = useCallback(async (date: string) => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(`/api/diary?date=${date}&limit=100`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = (await res.json()) as DiaryGetResponse;
      setData(json);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load diary");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchDiary(dateStr);
  }, [dateStr, fetchDiary]);

  const goBack = () => setDateStr((d) => addDays(d, -1));
  const goForward = () => {
    if (!isToday(dateStr)) setDateStr((d) => addDays(d, 1));
  };

  const stats = data ? computeStats(data.entries) : null;

  return (
    <div className="p-6 sm:p-8 space-y-6 max-w-3xl">
      {/* Header */}
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-cosmic-purple/20 border border-cosmic-purple/30 shrink-0">
            <BookOpen size={18} className="text-cosmic-purple" />
          </div>
          <div>
            <h1 className="text-2xl font-semibold text-cosmic-bright">
              Interaction Diary
            </h1>
            <p className="text-sm text-cosmic-muted mt-0.5">
              A log of every interaction Nova handled
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={() => void fetchDiary(dateStr)}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50 shrink-0"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Date navigation */}
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={goBack}
          aria-label="Previous day"
          className="flex items-center justify-center w-9 h-9 rounded-lg border border-cosmic-border text-cosmic-muted hover:text-cosmic-text hover:border-cosmic-purple/50 transition-colors"
        >
          <ChevronLeft size={16} />
        </button>

        <div className="flex-1 text-center">
          <p className="text-sm font-medium text-cosmic-text">
            {formatDisplayDate(dateStr)}
          </p>
          {isToday(dateStr) && (
            <span className="text-xs text-cosmic-purple font-medium">Today</span>
          )}
        </div>

        <button
          type="button"
          onClick={goForward}
          disabled={isToday(dateStr)}
          aria-label="Next day"
          className="flex items-center justify-center w-9 h-9 rounded-lg border border-cosmic-border text-cosmic-muted hover:text-cosmic-text hover:border-cosmic-purple/50 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
        >
          <ChevronRight size={16} />
        </button>
      </div>

      {/* Error state */}
      {error && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} className="shrink-0" />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {/* Summary bar */}
      {!loading && !error && stats && (
        <div className="grid grid-cols-3 gap-3">
          <div className="p-3 rounded-cosmic border border-cosmic-border bg-cosmic-surface">
            <p className="text-xs text-cosmic-muted uppercase tracking-wide mb-1">
              Entries
            </p>
            <p className="text-xl font-mono font-semibold text-cosmic-bright">
              {stats.total}
            </p>
          </div>
          <div className="p-3 rounded-cosmic border border-cosmic-border bg-cosmic-surface">
            <div className="flex items-center gap-1.5 text-xs text-cosmic-muted uppercase tracking-wide mb-1">
              <Zap size={11} />
              Tokens
            </div>
            <p className="text-xl font-mono font-semibold text-cosmic-bright">
              {stats.totalTokens.toLocaleString()}
            </p>
          </div>
          <div className="p-3 rounded-cosmic border border-cosmic-border bg-cosmic-surface">
            <div className="flex items-center gap-1.5 text-xs text-cosmic-muted uppercase tracking-wide mb-1">
              <Clock size={11} />
              Avg Latency
            </div>
            <p className="text-xl font-mono font-semibold text-cosmic-bright">
              {stats.avgLatencyMs > 0
                ? stats.avgLatencyMs >= 1000
                  ? `${(stats.avgLatencyMs / 1000).toFixed(1)}s`
                  : `${stats.avgLatencyMs}ms`
                : "—"}
            </p>
          </div>
        </div>
      )}

      {/* Loading skeleton */}
      {loading && (
        <div className="space-y-3">
          {Array.from({ length: 4 }).map((_, i) => (
            <div
              key={i}
              className="h-28 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
            />
          ))}
        </div>
      )}

      {/* Empty state */}
      {!loading && !error && data && data.entries.length === 0 && (
        <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted">
          <BookOpen size={36} className="opacity-40" />
          <p className="text-sm">No diary entries for this day.</p>
        </div>
      )}

      {/* Diary entries — reverse-chronological (API already returns newest first) */}
      {!loading && !error && data && data.entries.length > 0 && (
        <div className="space-y-3">
          {data.entries.map((entry, idx) => (
            <DiaryEntryCard key={`${entry.time}-${idx}`} entry={entry} />
          ))}
        </div>
      )}
    </div>
  );
}
