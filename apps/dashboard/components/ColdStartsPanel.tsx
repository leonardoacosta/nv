"use client";

import { useEffect, useState, useCallback } from "react";
import {
  Activity,
  RefreshCw,
  Timer,
  Zap,
} from "lucide-react";
import StatCard from "@/components/layout/StatCard";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import SectionHeader from "@/components/layout/SectionHeader";
import PipelineLatencyChart from "@/components/LatencyChart";
import { apiFetch } from "@/lib/api-client";

// -- API types ----------------------------------------------------------------

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

// -- Helpers ------------------------------------------------------------------

function msToSeconds(ms: number): string {
  return (ms / 1000).toFixed(1) + "s";
}

function avg(values: number[]): number {
  if (!values.length) return 0;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

function rollingAverage(events: ColdStartEvent[], window = 20): number[] {
  return events.map((_, idx) => {
    const half = Math.floor(window / 2);
    const start = Math.max(0, idx - half);
    const end = Math.min(events.length, idx + half + 1);
    const slice = events.slice(start, end);
    return avg(slice.map((e) => e.total_ms));
  });
}

// -- Chart --------------------------------------------------------------------

interface LatencyChartProps {
  events: ColdStartEvent[];
}

function LatencyChart({ events }: LatencyChartProps) {
  const visible = events.slice(0, 100).reverse();

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
  const rolling = rollingAverage(visible);

  const TICKS = 4;
  const yTicks = Array.from({ length: TICKS + 1 }, (_, i) =>
    minVal + (range * i) / TICKS,
  );

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

      {xLabelIndices.map((idx) => {
        const label = new Date(visible[idx]!.started_at).toLocaleTimeString(
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

      {/* first_response_ms series */}
      <polyline
        points={toPolyline(firstPoints)}
        fill="none"
        stroke="var(--ds-gray-600)"
        strokeWidth="1.5"
        strokeOpacity="0.5"
      />

      {/* total_ms series */}
      <polyline
        points={toPolyline(totalPoints)}
        fill="none"
        stroke="var(--ds-gray-800)"
        strokeWidth="2"
      />

      {/* Rolling average trend line */}
      <polyline
        points={toPolyline(rolling)}
        fill="none"
        stroke="var(--ds-amber-700)"
        strokeWidth="1.5"
        strokeDasharray="4 3"
        strokeOpacity="0.85"
      />

      {visible.map((e, i) => (
        <circle
          key={e.session_id + i}
          cx={xOf(i)}
          cy={yOf(e.total_ms)}
          r="2.5"
          fill="var(--ds-gray-700)"
          fillOpacity="0.8"
        />
      ))}
    </svg>
  );
}

function ChartLegend() {
  return (
    <div className="flex items-center gap-5 mt-3">
      <div className="flex items-center gap-1.5">
        <span
          className="inline-block w-5 h-0.5 rounded bg-ds-gray-800"
        />
        <span className="text-label-13 text-ds-gray-900">total_ms</span>
      </div>
      <div className="flex items-center gap-1.5">
        <span
          className="inline-block w-5 h-0.5 rounded bg-ds-gray-600 opacity-50"
        />
        <span className="text-label-13 text-ds-gray-900">first_response_ms</span>
      </div>
      <div className="flex items-center gap-1.5">
        <svg width="20" height="4">
          <line
            x1="0"
            y1="2"
            x2="20"
            y2="2"
            stroke="var(--ds-amber-700)"
            strokeWidth="1.5"
            strokeDasharray="4 3"
          />
        </svg>
        <span className="text-label-13 text-ds-gray-900">20-event avg</span>
      </div>
    </div>
  );
}

// -- ColdStartsPanel ----------------------------------------------------------

export default function ColdStartsPanel() {
  const [data, setData] = useState<ColdStartsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch("/api/cold-starts?limit=200");
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

  const visibleEvents = data?.events.slice(0, 100) ?? [];
  const avgToolCount = avg(visibleEvents.map((e) => e.tool_count));
  const avgTokensIn = avg(visibleEvents.map((e) => e.tokens_in));
  const avgTokensOut = avg(visibleEvents.map((e) => e.tokens_out));

  return (
    <div className="space-y-4 animate-fade-in-up">
      {/* Refresh button */}
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => void fetchData()}
          disabled={loading}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {error && (
        <ErrorBanner
          message="Failed to load cold-start data"
          detail={error}
          onRetry={() => void fetchData()}
        />
      )}

      {/* Percentile StatCards */}
      <div>
        <div className="mb-3">
          <SectionHeader label="Percentiles (24h)" />
        </div>
        {loading ? (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-32 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400"
              />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <StatCard
              icon={<Timer size={20} />}
              label="P50 Latency"
              value={msToSeconds(data?.percentiles.p50_ms ?? 0)}
              sublabel="median"
              variant="success"
            />
            <StatCard
              icon={<Timer size={20} />}
              label="P95 Latency"
              value={msToSeconds(data?.percentiles.p95_ms ?? 0)}
              sublabel="95th percentile"
              variant="warning"
            />
            <StatCard
              icon={<Timer size={20} />}
              label="P99 Latency"
              value={msToSeconds(data?.percentiles.p99_ms ?? 0)}
              sublabel="99th percentile"
              variant="error"
            />
          </div>
        )}
        {!loading && data && (
          <p className="mt-2 text-label-13 text-ds-gray-900">
            {data.percentiles.sample_count} events in window
          </p>
        )}
      </div>

      {/* Latency chart */}
      <div>
        <div className="mb-3">
          <SectionHeader label="Latency (last 100 events)" />
        </div>
        {loading ? (
          <div className="h-52 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400" />
        ) : !visibleEvents.length ? (
          <EmptyState
            title="No cold-start events"
            description="Cold start events will appear here once sessions are recorded."
            icon={<Activity size={24} aria-hidden="true" />}
          />
        ) : (
          <div className="surface-card p-4">
            <div className="surface-inset p-4">
              <LatencyChart events={data?.events ?? []} />
            </div>
            <ChartLegend />
          </div>
        )}
      </div>

      {/* Averages row */}
      <div>
        <div className="mb-3">
          <SectionHeader label="Averages (visible window)" />
        </div>
        {loading ? (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-28 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400"
              />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <StatCard
              icon={<Zap size={20} />}
              label="Avg Tool Count"
              value={avgToolCount.toFixed(1)}
              sublabel="per session"
              variant="default"
            />
            <StatCard
              icon={<Timer size={20} />}
              label="Avg Tokens In"
              value={Math.round(avgTokensIn).toLocaleString()}
              sublabel="input tokens"
              variant="default"
            />
            <StatCard
              icon={<Timer size={20} />}
              label="Avg Tokens Out"
              value={Math.round(avgTokensOut).toLocaleString()}
              sublabel="output tokens"
              variant="default"
            />
          </div>
        )}
      </div>

      {/* Pipeline Latency Chart */}
      <div className="mt-3">
        <PipelineLatencyChart />
      </div>
    </div>
  );
}
