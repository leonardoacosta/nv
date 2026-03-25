"use client";

import { useEffect, useState } from "react";

interface StatsPoint {
  tokens: number;
  ts: number;
}

interface StatsResponse {
  hourly?: StatsPoint[];
  tokens_today?: number;
  cost_today_usd?: number;
}

function buildPath(points: number[], width: number, height: number): string {
  if (points.length < 2) return "";

  const min = Math.min(...points);
  const max = Math.max(...points);
  const range = max - min || 1;

  const xs = points.map((_, i) => (i / (points.length - 1)) * width);
  const ys = points.map((v) => height - ((v - min) / range) * height);

  return xs
    .map((x, i) => `${i === 0 ? "M" : "L"} ${x.toFixed(1)} ${ys[i]!.toFixed(1)}`)
    .join(" ");
}

export default function UsageSparkline() {
  const [points, setPoints] = useState<number[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      try {
        const res = await fetch("/api/stats");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data = (await res.json()) as StatsResponse;

        if (cancelled) return;

        if (data.hourly && data.hourly.length > 0) {
          // Use last 24 hourly buckets
          const raw = data.hourly.slice(-24).map((p) => p.tokens);
          setPoints(raw);
        } else if (typeof data.tokens_today === "number") {
          // Fallback: synthesize a flat line from today's token count
          setPoints(Array.from({ length: 12 }, () => data.tokens_today ?? 0));
        }
      } catch {
        // Silently fail — sparkline is non-critical
        setPoints([]);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  const W = 120;
  const H = 24;
  const path = buildPath(points, W, H);

  if (loading) {
    return (
      <div
        style={{ width: W, height: H }}
        className="animate-pulse rounded bg-cosmic-border/40"
      />
    );
  }

  if (points.length < 2) return null;

  return (
    <svg
      width={W}
      height={H}
      viewBox={`0 0 ${W} ${H}`}
      aria-label="24h token usage sparkline"
      role="img"
      className="overflow-visible"
    >
      {/* Area fill */}
      <defs>
        <linearGradient id="sl-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#7c3aed" stopOpacity="0.18" />
          <stop offset="100%" stopColor="#7c3aed" stopOpacity="0" />
        </linearGradient>
      </defs>
      <path
        d={`${path} L ${W} ${H} L 0 ${H} Z`}
        fill="url(#sl-fill)"
      />
      {/* Line */}
      <path
        d={path}
        fill="none"
        stroke="#7c3aed"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
