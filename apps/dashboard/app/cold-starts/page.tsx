"use client";

import { useEffect, useState, useCallback } from "react";
import {
  Activity,
  RefreshCw,
  AlertCircle,
  Zap,
} from "lucide-react";

// ── API types ────────────────────────────────────────────────────────────────

interface ColdStartEvent {
  session_id: string;
  started_at: string;
  context_build_ms: number;
  first_response_ms: number;
  total_ms: number;
  tool_count: number;
  tokens_in: number;
  tokens_out: number;
  trigger_type: string;
}

interface ColdStartPercentiles {
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
  sample_count: number;
}

interface ColdStartsResponse {
  events: ColdStartEvent[];
  percentiles: ColdStartPercentiles;
}

// ── Helpers ──────────────────────────────────────────────────────────────────

function msToSeconds(ms: number): string {
  return (ms / 1000).toFixed(1) + "s";
}

function avg(values: number[]): number {
  if (!values.length) return 0;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

/** Compute 20-event rolling average of total_ms (centered on each point) */
function rollingAverage(events: ColdStartEvent[], window = 20): number[] {
  return events.map((_, idx) => {
    const half = Math.floor(window / 2);
    const start = Math.max(0, idx - half);
    const end = Math.min(events.length, idx + half + 1);
    const slice = events.slice(start, end);
    return avg(slice.map((e) => e.total_ms));
  });
}

// ── Chart ────────────────────────────────────────────────────────────────────

interface LatencyChartProps {
  events: ColdStartEvent[];
}

function LatencyChart({ events }: LatencyChartProps) {
  const visible = events.slice(0, 100).reverse(); // oldest first for left-to-right

  if (!visible.length) return null;

  const allValues = [
    ...visible.map((e) => e.total_ms),
    ...visible.map((e) => e.first_response_ms),
  ];
  const maxVal = Math.max(...allValues, 1);
  const minVal = 0;
  const range = maxVal - minVal;

  const W = 800;
  const H = 200;
  const PAD_L = 52;
  const PAD_R = 16;
  const PAD_T = 16;
  const PAD_B = 32;
  const chartW = W - PAD_L - PAD_R;
  const chartH = H - PAD_T - PAD_B;

  const n = visible.length;
  const xOf = (i: number) => PAD_L + (i / Math.max(n - 1, 1)) * chartW;
  const yOf = (ms: number) =>
    PAD_T + chartH - ((ms - minVal) / range) * chartH;

  const toPolyline = (vals: number[]) =>
    vals.map((v, i) => `${xOf(i)},${yOf(v)}`).join(" ");

  const totalPoints = visible.map((e) => e.total_ms);
  const firstPoints = visible.map((e) => e.first_response_ms);

  // Rolling average computed on chronological order (visible = oldest-first already)
  const rolling = rollingAverage(visible);

  // Y-axis tick labels
  const TICKS = 4;
  const yTicks = Array.from({ length: TICKS + 1 }, (_, i) =>
    minVal + (range * i) / TICKS,
  );

  // X-axis: show ~5 timestamp labels
  const xLabelCount = Math.min(5, n);
  const xLabelIndices =
    n <= 1
      ? [0]
      : Array.from({ length: xLabelCount }, (_, i) =>
          Math.round((i / (xLabelCount - 1)) * (n - 1)),
        );

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      className="w-full"
      style={{ height: "200px" }}
      aria-label="Cold start latency chart"
    >
      {/* Grid lines */}
      {yTicks.map((tick) => (
        <line
          key={tick}
          x1={PAD_L}
          y1={yOf(tick)}
          x2={W - PAD_R}
          y2={yOf(tick)}
          stroke="rgba(255,255,255,0.06)"
          strokeWidth="1"
        />
      ))}

      {/* Y-axis labels */}
      {yTicks.map((tick) => (
        <text
          key={tick}
          x={PAD_L - 6}
          y={yOf(tick) + 4}
          textAnchor="end"
          fontSize="10"
          fill="rgba(255,255,255,0.35)"
        >
          {tick >= 1000
            ? `${(tick / 1000).toFixed(1)}s`
            : `${Math.round(tick)}`}
        </text>
      ))}

      {/* X-axis labels */}
      {xLabelIndices.map((idx) => {
        const label = new Date(visible[idx].started_at).toLocaleTimeString(
          [],
          { hour: "2-digit", minute: "2-digit" },
        );
        return (
          <text
            key={idx}
            x={xOf(idx)}
            y={H - PAD_B + 14}
            textAnchor="middle"
            fontSize="9"
            fill="rgba(255,255,255,0.30)"
          >
            {label}
          </text>
        );
      })}

      {/* first_response_ms series — dimmer */}
      <polyline
        points={toPolyline(firstPoints)}
        fill="none"
        stroke="#8B5CF6"
        strokeWidth="1.5"
        strokeOpacity="0.5"
      />

      {/* total_ms series */}
      <polyline
        points={toPolyline(totalPoints)}
        fill="none"
        stroke="#8B5CF6"
        strokeWidth="2"
      />

      {/* Rolling average trend line */}
      <polyline
        points={toPolyline(rolling)}
        fill="none"
        stroke="#F59E0B"
        strokeWidth="1.5"
        strokeDasharray="4 3"
        strokeOpacity="0.85"
      />

      {/* Data point dots for total_ms */}
      {visible.map((e, i) => (
        <circle
          key={e.session_id + i}
          cx={xOf(i)}
          cy={yOf(e.total_ms)}
          r="2.5"
          fill="#8B5CF6"
          fillOpacity="0.8"
        />
      ))}
    </svg>
  );
}

// ── Legend ────────────────────────────────────────────────────────────────────

function ChartLegend() {
  return (
    <div className="flex items-center gap-5 mt-2">
      <div className="flex items-center gap-1.5">
        <span
          className="inline-block w-5 h-0.5 rounded"
          style={{ background: "#8B5CF6" }}
        />
        <span className="text-xs text-cosmic-muted">total_ms</span>
      </div>
      <div className="flex items-center gap-1.5">
        <span
          className="inline-block w-5 h-0.5 rounded"
          style={{ background: "#8B5CF6", opacity: 0.5 }}
        />
        <span className="text-xs text-cosmic-muted">first_response_ms</span>
      </div>
      <div className="flex items-center gap-1.5">
        <svg width="20" height="4">
          <line
            x1="0"
            y1="2"
            x2="20"
            y2="2"
            stroke="#F59E0B"
            strokeWidth="1.5"
            strokeDasharray="4 3"
          />
        </svg>
        <span className="text-xs text-cosmic-muted">20-event avg</span>
      </div>
    </div>
  );
}

// ── Percentile card ───────────────────────────────────────────────────────────

function PercentileCard({
  label,
  ms,
  accent,
}: {
  label: string;
  ms: number;
  accent?: boolean;
}) {
  return (
    <div
      className={`p-4 rounded-cosmic border ${
        accent
          ? "border-cosmic-purple/40 bg-cosmic-purple/10"
          : "border-cosmic-border bg-cosmic-surface"
      }`}
    >
      <p className="text-xs text-cosmic-muted uppercase tracking-wide">{label}</p>
      <p
        className={`text-2xl font-mono font-semibold mt-1 ${
          accent ? "text-cosmic-bright" : "text-cosmic-text"
        }`}
      >
        {msToSeconds(ms)}
      </p>
    </div>
  );
}

// ── Stat card ─────────────────────────────────────────────────────────────────

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="p-3 rounded-cosmic border border-cosmic-border bg-cosmic-surface">
      <p className="text-xs text-cosmic-muted uppercase tracking-wide">{label}</p>
      <p className="text-lg font-mono font-semibold text-cosmic-bright mt-0.5">
        {value}
      </p>
    </div>
  );
}

// ── Page ──────────────────────────────────────────────────────────────────────

export default function ColdStartsPage() {
  const [data, setData] = useState<ColdStartsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/cold-starts?limit=200");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = (await res.json()) as ColdStartsResponse;
      setData(json);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load cold-start data",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchData();
  }, [fetchData]);

  // Compute client-side stats from first 100 events (chart window)
  const visibleEvents = data?.events.slice(0, 100) ?? [];
  const avgToolCount = avg(visibleEvents.map((e) => e.tool_count));
  const avgTokensIn = avg(visibleEvents.map((e) => e.tokens_in));
  const avgTokensOut = avg(visibleEvents.map((e) => e.tokens_out));

  return (
    <div className="p-8 space-y-8 max-w-5xl">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">
            Cold Starts
          </h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            Session latency, percentiles, and token usage
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchData()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {/* Percentile cards */}
      <div>
        <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide mb-3">
          Percentiles (24h)
        </h2>
        {loading ? (
          <div className="grid grid-cols-3 gap-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-20 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
              />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <PercentileCard
              label="P50"
              ms={data?.percentiles.p50_ms ?? 0}
              accent
            />
            <PercentileCard
              label="P95"
              ms={data?.percentiles.p95_ms ?? 0}
            />
            <PercentileCard
              label="P99"
              ms={data?.percentiles.p99_ms ?? 0}
            />
          </div>
        )}
        {!loading && data && (
          <p className="mt-2 text-xs text-cosmic-muted">
            {data.percentiles.sample_count} events in window
          </p>
        )}
      </div>

      {/* Latency chart */}
      <div>
        <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide mb-3">
          Latency (last 100 events)
        </h2>
        {loading ? (
          <div className="h-52 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border" />
        ) : !visibleEvents.length ? (
          <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted rounded-cosmic border border-cosmic-border bg-cosmic-surface">
            <Activity size={32} />
            <p className="text-sm">No cold-start events recorded yet</p>
          </div>
        ) : (
          <div className="rounded-cosmic border border-cosmic-border bg-cosmic-surface p-4">
            <LatencyChart events={data?.events ?? []} />
            <ChartLegend />
          </div>
        )}
      </div>

      {/* Stats row */}
      <div>
        <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide mb-3">
          Averages (visible window)
        </h2>
        {loading ? (
          <div className="grid grid-cols-3 gap-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-16 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
              />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <StatCard
              label="Avg Tool Count"
              value={avgToolCount.toFixed(1)}
            />
            <StatCard
              label="Avg Tokens In"
              value={Math.round(avgTokensIn).toLocaleString()}
            />
            <StatCard
              label="Avg Tokens Out"
              value={Math.round(avgTokensOut).toLocaleString()}
            />
          </div>
        )}
      </div>
    </div>
  );
}
