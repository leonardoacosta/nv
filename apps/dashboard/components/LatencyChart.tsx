"use client";

import { useEffect, useState } from "react";
import { Activity, RefreshCw, AlertCircle } from "lucide-react";
import { trpcClient } from "@/lib/trpc/client";

// ── API types ─────────────────────────────────────────────────────────────────

interface StageLatency {
  stage: string;
  p50_ms: number | null;
  p95_ms: number | null;
  window: string;
}

interface LatencyResponse {
  stages: StageLatency[];
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function msLabel(ms: number | null): string {
  if (ms === null || ms === undefined) return "—";
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.round(ms)}ms`;
}

/** Normalise stage names for display. */
function stageLabel(stage: string): string {
  const labels: Record<string, string> = {
    receive: "Receive",
    context_build: "Context Build",
    api_call: "API Call",
    tool_loop: "Tool Loop",
    delivery: "Delivery",
  };
  return labels[stage] ?? stage;
}

/**
 * Horizontal bar chart row for a single stage.
 * Renders two overlapping bars: P50 (filled) and P95 (outlined).
 */
function StageRow({
  entry,
  maxMs,
}: {
  entry: StageLatency;
  maxMs: number;
}) {
  const p50 = entry.p50_ms ?? 0;
  const p95 = entry.p95_ms ?? 0;
  const p50Pct = maxMs > 0 ? Math.min((p50 / maxMs) * 100, 100) : 0;
  const p95Pct = maxMs > 0 ? Math.min((p95 / maxMs) * 100, 100) : 0;

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between text-copy-13">
        <span className="font-medium text-ds-gray-1000">{stageLabel(entry.stage)}</span>
        <span className="text-ds-gray-900 tabular-nums">
          P50 {msLabel(entry.p50_ms)} / P95 {msLabel(entry.p95_ms)}
        </span>
      </div>
      <div className="relative h-4 rounded overflow-hidden bg-ds-gray-alpha-200">
        {/* P95 outlined bar (wider, lighter) */}
        {p95 > 0 && (
          <div
            className="absolute inset-y-0 left-0 rounded border border-blue-700 bg-blue-700/20"
            style={{ width: `${p95Pct}%` }}
          />
        )}
        {/* P50 filled bar (narrower, solid) */}
        {p50 > 0 && (
          <div
            className="absolute inset-y-0 left-0 rounded bg-blue-700"
            style={{ width: `${p50Pct}%` }}
          />
        )}
        {/* No-data state */}
        {p50 === 0 && p95 === 0 && (
          <div className="absolute inset-0 flex items-center px-2">
            <span className="text-copy-13 text-ds-gray-900">no data</span>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

interface LatencyChartProps {
  /** Which time window to display: "24h" or "7d". Defaults to "24h". */
  window?: "24h" | "7d";
}

export default function LatencyChart({ window = "24h" }: LatencyChartProps) {
  const [data, setData] = useState<LatencyResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async () => {
    try {
      setLoading(true);
      setError(null);
      const json = (await trpcClient.system.latency.query()) as LatencyResponse;
      setData(json);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
    // Refresh every 30 seconds
    const id = setInterval(fetchData, 30_000);
    return () => clearInterval(id);
  }, []);

  // Filter to the selected window
  const windowStages = data?.stages.filter((s) => s.window === window) ?? [];

  // Find the max P95 value across all stages for proportional bar scaling
  const maxMs = Math.max(
    1,
    ...windowStages.map((s) => s.p95_ms ?? s.p50_ms ?? 0),
  );

  return (
    <div className="rounded-lg border border-ds-gray-400 bg-ds-gray-100 p-4 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Activity className="h-4 w-4 text-ds-gray-700" />
          <h3 className="text-copy-14 font-semibold text-ds-gray-1000">Pipeline Latency</h3>
          <span className="rounded-full bg-ds-gray-alpha-200 px-2 py-0.5 text-label-12 text-ds-gray-900">
            {window}
          </span>
        </div>
        <button
          type="button"
          onClick={fetchData}
          className="rounded p-1 text-ds-gray-700 hover:text-ds-gray-1000 hover:bg-ds-gray-alpha-200 transition-colors"
          aria-label="Refresh latency data"
        >
          <RefreshCw className="h-3.5 w-3.5" />
        </button>
      </div>

      {/* Legend */}
      <div className="flex items-center gap-4 text-copy-13 text-ds-gray-900">
        <span className="flex items-center gap-1.5">
          <span className="inline-block h-2.5 w-4 rounded bg-blue-700" />
          P50
        </span>
        <span className="flex items-center gap-1.5">
          <span className="inline-block h-2.5 w-4 rounded border border-blue-700 bg-blue-700/20" />
          P95
        </span>
      </div>

      {/* Content */}
      {loading && (
        <div className="flex items-center justify-center py-4 text-copy-13 text-ds-gray-900">
          Loading...
        </div>
      )}

      {!loading && error && (
        <div className="flex items-center gap-2 rounded-md bg-red-700/10 px-3 py-2 text-copy-13 text-red-700">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {!loading && !error && windowStages.length === 0 && (
        <div className="py-3 text-center text-copy-13 text-ds-gray-900">
          No latency data yet. Data appears after the first message.
        </div>
      )}

      {!loading && !error && windowStages.length > 0 && (
        <div className="space-y-3">
          {windowStages.map((entry) => (
            <StageRow key={entry.stage} entry={entry} maxMs={maxMs} />
          ))}
        </div>
      )}
    </div>
  );
}
