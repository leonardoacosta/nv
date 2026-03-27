"use client";

import { useEffect, useState, useCallback } from "react";
import {
  BookOpen,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  Clock,
  Zap,
  Hash,
} from "lucide-react";
import DiaryEntryCard from "@/components/DiaryEntry";
import ErrorBanner from "@/components/layout/ErrorBanner";
import StatCard from "@/components/layout/StatCard";
import type { DiaryGetResponse, DiaryEntryItem } from "@/types/api";
import { apiFetch } from "@/lib/api-client";

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
      const res = await apiFetch(`/api/diary?date=${date}&limit=100`);
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
    <div className="p-4 space-y-3 w-full animate-fade-in-up">
      {/* Header */}
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-ds-gray-alpha-200 border border-ds-gray-1000/30 shrink-0">
            <BookOpen size={18} className="text-ds-gray-1000" />
          </div>
          <div>
            <h1 className="text-heading-20 text-ds-gray-1000">
              Interaction Diary
            </h1>
            <p className="text-copy-13 text-ds-gray-900 mt-0.5">
              A log of every interaction Nova handled
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={() => void fetchDiary(dateStr)}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50 shrink-0"
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
          className="flex items-center justify-center w-9 h-9 rounded-lg border border-ds-gray-400 text-ds-gray-900 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors"
        >
          <ChevronLeft size={16} />
        </button>

        <div className="flex-1 text-center">
          <p className="text-label-14 text-ds-gray-1000">
            {formatDisplayDate(dateStr)}
          </p>
          {isToday(dateStr) && (
            <span className="text-label-13 text-ds-gray-1000">Today</span>
          )}
        </div>

        <button
          type="button"
          onClick={goForward}
          disabled={isToday(dateStr)}
          aria-label="Next day"
          className="flex items-center justify-center w-9 h-9 rounded-lg border border-ds-gray-400 text-ds-gray-900 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
        >
          <ChevronRight size={16} />
        </button>
      </div>

      {/* Error state */}
      {error && (
        <ErrorBanner
          message="Failed to load diary"
          detail={error}
          onRetry={() => void fetchDiary(dateStr)}
        />
      )}

      {/* Summary bar */}
      {!loading && !error && stats && (
        <div className="flex flex-wrap border-b border-ds-gray-400">
          <StatCard
            icon={<Hash size={14} />}
            label="Entries"
            value={stats.total}
            inline
          />
          <StatCard
            icon={<Zap size={14} />}
            label="Tokens"
            value={stats.totalTokens.toLocaleString()}
            inline
          />
          <StatCard
            icon={<Clock size={14} />}
            label="Avg Latency"
            value={
              stats.avgLatencyMs > 0
                ? stats.avgLatencyMs >= 1000
                  ? `${(stats.avgLatencyMs / 1000).toFixed(1)}s`
                  : `${stats.avgLatencyMs}ms`
                : "—"
            }
            inline
          />
        </div>
      )}

      {/* Loading skeleton */}
      {loading && (
        <div className="space-y-3">
          {Array.from({ length: 4 }).map((_, i) => (
            <div
              key={i}
              className="h-28 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
            />
          ))}
        </div>
      )}

      {/* Empty state */}
      {!loading && !error && data && data.entries.length === 0 && (
        <p className="text-copy-13 text-ds-gray-900 py-3">No entries for this day</p>
      )}

      {/* Diary entries — reverse-chronological (API already returns newest first) */}
      {!loading && !error && data && data.entries.length > 0 && (
        <div className="space-y-3">
          {data.entries.map((entry, idx) => (
            <div
              key={`${entry.time}-${idx}`}
              className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
            >
              <DiaryEntryCard entry={entry} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
