"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Activity, AlertCircle, Database, RefreshCw, Server, Radio } from "lucide-react";
import ChannelRow from "@/components/ChannelRow";
import ServiceRow from "@/components/ServiceRow";
import { apiFetch } from "@/lib/api-client";
import type {
  FleetHealthResponse,
  ServerHealthGetResponse,
} from "@/types/api";

/** Auto-refresh interval in milliseconds (30s). */
const POLL_INTERVAL_MS = 30_000;

export default function StatusPage() {
  const [fleetData, setFleetData] = useState<FleetHealthResponse | null>(null);
  const [infraData, setInfraData] = useState<ServerHealthGetResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastChecked, setLastChecked] = useState<Date | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchStatus = useCallback(async () => {
    setError(null);
    try {
      const [fleetRes, healthRes] = await Promise.all([
        apiFetch("/api/fleet-status"),
        apiFetch("/api/server-health"),
      ]);

      if (!fleetRes.ok) throw new Error(`Fleet status: HTTP ${fleetRes.status}`);
      if (!healthRes.ok) throw new Error(`Server health: HTTP ${healthRes.status}`);

      const fleet = (await fleetRes.json()) as FleetHealthResponse;
      const health = (await healthRes.json()) as ServerHealthGetResponse;

      setFleetData(fleet);
      setInfraData(health);
      setLastChecked(new Date());
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load status");
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial fetch + auto-refresh with cleanup
  useEffect(() => {
    void fetchStatus();

    intervalRef.current = setInterval(() => {
      void fetchStatus();
    }, POLL_INTERVAL_MS);

    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchStatus]);

  const dbStatus = infraData?.status ?? "unknown";

  return (
    <div className="p-4 space-y-3 w-full animate-fade-in-up">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-heading-20 text-ds-gray-1000">Status</h1>
          <p className="mt-0.5 text-copy-13 text-ds-gray-900">
            Service health and channel connectivity
          </p>
        </div>
        <div className="flex items-center gap-3">
          {lastChecked && (
            <span className="text-label-12 text-ds-gray-700 font-mono">
              <LastCheckedLabel date={lastChecked} />
            </span>
          )}
          <button
            type="button"
            onClick={() => {
              setLoading(true);
              void fetchStatus();
            }}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
          >
            <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
            Refresh
          </button>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div
          className="flex items-start gap-3 p-4 rounded-md"
          style={{
            background: "rgba(229, 72, 77, 0.08)",
            borderLeft: "3px solid var(--ds-red-700)",
          }}
        >
          <AlertCircle size={16} className="text-red-700 shrink-0 mt-0.5" />
          <span className="text-copy-14 text-red-700">{error}</span>
        </div>
      )}

      {/* Loading skeleton */}
      {loading && !fleetData ? (
        <div className="space-y-6">
          {Array.from({ length: 3 }).map((_, g) => (
            <div key={g} className="space-y-2">
              <div className="h-3 w-24 animate-pulse rounded bg-ds-gray-300" />
              {Array.from({ length: 3 }).map((_, i) => (
                <div
                  key={i}
                  className="h-9 animate-pulse rounded-md bg-ds-gray-100 border border-ds-gray-alpha-400"
                />
              ))}
            </div>
          ))}
        </div>
      ) : (
        <div className="space-y-4">
          {/* ── Channels ─────────────────────────────────────── */}
          <section>
            <div className="flex items-center gap-2 mb-2">
              <Radio size={12} className="text-ds-gray-700" />
              <h2 className="text-label-12 text-ds-gray-700">Channels</h2>
              <span className="px-1.5 py-0.5 rounded-full bg-ds-gray-alpha-200 text-label-12 text-ds-gray-900 font-mono">
                {fleetData?.channels.length ?? 0}
              </span>
            </div>
            {fleetData?.channels.length === 0 ? (
              <p className="text-copy-13 text-ds-gray-900 py-2 pl-1 italic">
                No channels configured.
              </p>
            ) : (
              <div className="space-y-0.5">
                {fleetData?.channels.map((ch) => (
                  <ChannelRow key={ch.name} channel={ch} />
                ))}
              </div>
            )}
          </section>

          {/* ── Fleet Services ────────────────────────────────── */}
          <section>
            <div className="flex items-center gap-2 mb-2">
              <Server size={12} className="text-ds-gray-700" />
              <h2 className="text-label-12 text-ds-gray-700">Fleet Services</h2>
              <span className="px-1.5 py-0.5 rounded-full bg-ds-gray-alpha-200 text-label-12 text-ds-gray-900 font-mono">
                {fleetData?.fleet.total_count ?? 0}
              </span>
            </div>
            {/* Aggregate status line */}
            {fleetData && (
              <div className="flex items-center gap-2 px-3 py-1.5 mb-1">
                <span
                  className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${
                    fleetData.fleet.status === "healthy"
                      ? "bg-green-700"
                      : fleetData.fleet.status === "unknown"
                        ? "bg-ds-gray-500"
                        : "bg-red-700"
                  }`}
                />
                <span className="text-copy-13 text-ds-gray-900 font-mono">
                  {fleetData.fleet.status === "unknown"
                    ? `${fleetData.fleet.total_count} services configured (status unknown -- host network only)`
                    : fleetData.fleet.healthy_count === fleetData.fleet.total_count
                      ? `${fleetData.fleet.healthy_count}/${fleetData.fleet.total_count} healthy`
                      : `${fleetData.fleet.healthy_count}/${fleetData.fleet.total_count} healthy (${fleetData.fleet.total_count - fleetData.fleet.healthy_count} unreachable)`}
                </span>
              </div>
            )}
            <div className="space-y-0.5">
              {fleetData?.fleet.services.map((svc) => (
                <ServiceRow
                  key={svc.name}
                  service={svc}
                  lastChecked={lastChecked}
                />
              ))}
            </div>
          </section>

          {/* ── Infrastructure ────────────────────────────────── */}
          <section>
            <div className="flex items-center gap-2 mb-2">
              <Database size={12} className="text-ds-gray-700" />
              <h2 className="text-label-12 text-ds-gray-700">Infrastructure</h2>
            </div>
            <div className="space-y-0.5">
              {/* Postgres */}
              <div className="flex items-center gap-3 px-3 py-2 min-h-9 rounded-md hover:bg-ds-gray-alpha-100 transition-colors">
                <span
                  className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${
                    dbStatus === "healthy"
                      ? "bg-green-700"
                      : dbStatus === "degraded"
                        ? "bg-amber-700"
                        : dbStatus === "critical"
                          ? "bg-red-700"
                          : "bg-ds-gray-500"
                  }`}
                />
                <span className="text-label-14 text-ds-gray-1000 flex-1">
                  Postgres
                </span>
                <span className="text-label-12 text-ds-gray-700 font-mono">
                  {dbStatus}
                </span>
              </div>

              {/* Daemon */}
              <div className="flex items-center gap-3 px-3 py-2 min-h-9 rounded-md hover:bg-ds-gray-alpha-100 transition-colors">
                <span
                  className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${
                    infraData?.latest?.uptime_seconds != null
                      ? "bg-green-700"
                      : "bg-ds-gray-500"
                  }`}
                />
                <span className="text-label-14 text-ds-gray-1000 flex-1">
                  Daemon
                </span>
                {infraData?.latest?.uptime_seconds != null && (
                  <span className="text-label-12 text-ds-gray-900 font-mono">
                    uptime {formatUptime(infraData.latest.uptime_seconds)}
                  </span>
                )}
                <span className="text-label-12 text-ds-gray-700 font-mono">
                  {infraData?.latest?.uptime_seconds != null
                    ? "healthy"
                    : "no data"}
                </span>
              </div>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

// ── Helpers ──────────────────────────────────────────────────────────────

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** Self-updating "Last checked: Xs ago" label. */
function LastCheckedLabel({ date }: { date: Date }) {
  const [, setTick] = useState(0);

  useEffect(() => {
    const timer = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(timer);
  }, []);

  const diffSecs = Math.floor((Date.now() - date.getTime()) / 1000);
  if (diffSecs < 60) return <>Last checked: {diffSecs}s ago</>;
  const mins = Math.floor(diffSecs / 60);
  return <>Last checked: {mins}m ago</>;
}
