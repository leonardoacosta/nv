"use client";

import { useState, useCallback } from "react";
import {
  BookOpen,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  Clock,
  Hash,
  Radio,
} from "lucide-react";
import DiaryEntryCard from "@/components/DiaryEntry";
import ErrorBanner from "@/components/layout/ErrorBanner";
import StatCard from "@/components/layout/StatCard";
import type { DiaryGetResponse } from "@/types/api";
import { useQuery } from "@tanstack/react-query";
import { useTRPC } from "@/lib/trpc/react";

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

function isYesterday(dateStr: string): boolean {
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  return dateStr === toDateString(yesterday);
}

function relativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffMs = now - then;
  if (diffMs < 0) return "just now";
  const diffSec = Math.floor(diffMs / 1000);
  if (diffSec < 60) return `${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  const diffDay = Math.floor(diffHr / 24);
  return `${diffDay}d ago`;
}

/** Contextual day label: "Today", "Yesterday", or formatted date. */
function dayLabel(dateStr: string): string {
  if (isToday(dateStr)) return "Today";
  if (isYesterday(dateStr)) return "Yesterday";
  return formatDisplayDate(dateStr);
}

// ── Page ─────────────────────────────────────────────────────────────────────

export default function DiaryPage() {
  const trpc = useTRPC();
  const [dateStr, setDateStr] = useState<string>(toDateString(new Date()));

  const diaryQuery = useQuery(
    trpc.diary.list.queryOptions({ date: dateStr, limit: 100 }),
  );
  const data = (diaryQuery.data as DiaryGetResponse | undefined) ?? null;
  const loading = diaryQuery.isLoading;
  const error = diaryQuery.error?.message ?? null;

  // Refetch triggers -- date navigation re-queries automatically via queryOptions input change
  const fetchDiary = useCallback(() => {
    void diaryQuery.refetch();
  }, [diaryQuery]);

  const goBack = () => setDateStr((d) => addDays(d, -1));
  const goForward = () => {
    if (!isToday(dateStr)) setDateStr((d) => addDays(d, 1));
  };

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
              Activity Log
            </h1>
            <p className="text-copy-13 text-ds-gray-900 mt-0.5">
              Nova&apos;s interaction history
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={() => void fetchDiary()}
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
          onRetry={() => void fetchDiary()}
        />
      )}

      {/* Summary bar */}
      {!loading && !error && data && (
        <div className="flex flex-wrap border-b border-ds-gray-400">
          <StatCard
            icon={<Hash size={14} />}
            label="Entries"
            value={data.total}
            inline
          />
          <StatCard
            icon={<Radio size={14} />}
            label="Channels"
            value={data.distinct_channels}
            inline
          />
          <StatCard
            icon={<Clock size={14} />}
            label="Last Activity"
            value={
              data.last_interaction_at
                ? relativeTime(data.last_interaction_at)
                : "\u2014"
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
        <div>
          {/* Day header */}
          <div className="text-label-12 text-ds-gray-900 uppercase tracking-wider py-2 border-b border-ds-gray-400">
            {dayLabel(dateStr)}
          </div>

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
