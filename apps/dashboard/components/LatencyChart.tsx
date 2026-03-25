"use client";

import { useEffect, useState } from "react";
import { Activity, RefreshCw, AlertCircle } from "lucide-react";

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
      <div className="flex items-center justify-between text-xs">
        <span className="font-medium text-foreground">{stageLabel(entry.stage)}</span>
        <span className="text-muted-foreground tabular-nums">
          P50 {msLabel(entry.p50_ms)} / P95 {msLabel(entry.p95_ms)}
        </span>
      </div>
      <div className="relative h-4 rounded overflow-hidden bg-muted">
        {/* P95 outlined bar (wider, lighter) */}
        {p95 > 0 && (
          <div
            className="absolute inset-y-0 left-0 rounded border border-blue-400 bg-blue-400/20"
            style={{ width: `${p95Pct}%` }}
          />
        )}
        {/* P50 filled bar (narrower, solid) */}
        {p50 > 0 && (
          <div
            className="absolute inset-y-0 left-0 rounded bg-blue-500"
            style={{ width: `${p50Pct}%` }}
          />
        )}
        {/* No-data state */}
        {p50 === 0 && p95 === 0 && (
          <div className="absolute inset-0 flex items-center px-2">
            <span className="text-xs text-muted-foreground">no data</span>
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
      const res = await fetch("/api/latency");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json: LatencyResponse = await res.json();
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
    <div className="rounded-lg border bg-card p-4 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Activity className="h-4 w-4 text-muted-foreground" />
          <h3 className="text-sm font-semibold">Pipeline Latency</h3>
          <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
            {window}
          </span>
        </div>
        <button
          onClick={fetchData}
          className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          aria-label="Refresh latency data"
        >
          <RefreshCw className="h-3.5 w-3.5" />
        </button>
      </div>

      {/* Legend */}
      <div className="flex items-center gap-4 text-xs text-muted-foreground">
        <span className="flex items-center gap-1.5">
          <span className="inline-block h-2.5 w-4 rounded bg-blue-500" />
          P50
        </span>
        <span className="flex items-center gap-1.5">
          <span className="inline-block h-2.5 w-4 rounded border border-blue-400 bg-blue-400/20" />
          P95
        </span>
      </div>

      {/* Content */}
      {loading && (
        <div className="flex items-center justify-center py-6 text-sm text-muted-foreground">
          Loading...
        </div>
      )}

      {!loading && error && (
        <div className="flex items-center gap-2 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {!loading && !error && windowStages.length === 0 && (
        <div className="py-4 text-center text-sm text-muted-foreground">
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
