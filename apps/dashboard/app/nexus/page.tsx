"use client";

import {
  Cpu,
  MemoryStick,
  HardDrive,
  Activity,
  RefreshCw,
  Clock,
  Wifi,
  WifiOff,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import type { ServerHealthGetResponse } from "@/types/api";
import { useQuery } from "@tanstack/react-query";
import { useTRPC } from "@/lib/trpc/react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TileStatus = "ok" | "warn" | "crit" | "unknown";

interface HealthTile {
  id: string;
  label: string;
  value: string;
  sublabel: string;
  status: TileStatus;
  icon: React.ElementType;
  pct: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatUptime(seconds: number | null): string {
  if (!seconds) return "—";
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function tileStatus(pct: number, warnAt = 70, critAt = 90): TileStatus {
  if (pct >= critAt) return "crit";
  if (pct >= warnAt) return "warn";
  return "ok";
}

const ACCENT_BAR: Record<TileStatus, string> = {
  ok: "bg-green-700",
  warn: "bg-amber-700",
  crit: "bg-red-700",
  unknown: "bg-ds-gray-600",
};

const STATUS_DOT: Record<TileStatus, string> = {
  ok: "bg-green-700",
  warn: "bg-amber-700",
  crit: "bg-red-700",
  unknown: "bg-ds-gray-600",
};

const STATUS_LABEL: Record<TileStatus, string> = {
  ok: "Healthy",
  warn: "Warning",
  crit: "Critical",
  unknown: "Unknown",
};

// ---------------------------------------------------------------------------
// HealthTileCard
// ---------------------------------------------------------------------------

function HealthTileCard({ tile }: { tile: HealthTile }) {
  const TileIcon = tile.icon;
  const accent = ACCENT_BAR[tile.status];

  return (
    <div className="surface-card relative flex flex-col gap-4 p-5 overflow-hidden">
      {/* Left accent bar */}
      <div
        className={`absolute left-0 top-0 bottom-0 w-1 ${accent} rounded-l-xl`}
        aria-hidden="true"
      />

      {/* Header */}
      <div className="flex items-center justify-between pl-2">
        <div className="flex items-center gap-2.5">
          <TileIcon size={16} className="text-ds-gray-700 shrink-0" />
          <span className="text-label-12 text-ds-gray-900">{tile.label}</span>
        </div>
        <span
          className={`inline-flex items-center gap-1.5 text-label-12 ${STATUS_LABEL[tile.status] === "Healthy" ? "text-green-700" : STATUS_LABEL[tile.status] === "Warning" ? "text-amber-700" : STATUS_LABEL[tile.status] === "Critical" ? "text-red-700" : "text-ds-gray-700"}`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full ${STATUS_DOT[tile.status]}`}
          />
          {STATUS_LABEL[tile.status]}
        </span>
      </div>

      {/* Value */}
      <div className="pl-2">
        <div className="text-heading-32 text-ds-gray-1000 leading-none">
          {tile.value}
        </div>
        <div className="mt-1 text-label-13 text-ds-gray-900">{tile.sublabel}</div>
      </div>

      {/* Progress bar */}
      <div className="pl-2">
        <div className="h-1.5 rounded-full bg-ds-bg-100 overflow-hidden">
          <div
            className={`h-full rounded-full transition-all duration-700 ${accent}`}
            style={{ width: `${Math.min(100, tile.pct)}%` }}
          />
        </div>
        <div className="flex justify-between mt-1">
          <span className="text-label-12 text-ds-gray-900">0%</span>
          <span className="text-label-12 text-ds-gray-900">
            {tile.pct.toFixed(0)}%
          </span>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// StatusBadge
// ---------------------------------------------------------------------------

function DaemonStatusBadge({ status }: { status: string }) {
  const isHealthy = status === "healthy" || status === "ok";
  const isDegraded = status === "degraded";

  const icon = isHealthy ? (
    <Wifi size={14} className="text-green-700" />
  ) : isDegraded ? (
    <Wifi size={14} className="text-amber-700" />
  ) : (
    <WifiOff size={14} className="text-red-700" />
  );

  const badgeClass = isHealthy
    ? "bg-green-700/10 text-green-700 border-green-700/20"
    : isDegraded
      ? "bg-amber-700/10 text-amber-700 border-amber-700/20"
      : "bg-red-700/10 text-red-700 border-red-700/20";

  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-label-12 border ${badgeClass}`}
    >
      {icon}
      <span className="capitalize">{status}</span>
    </span>
  );
}

// ---------------------------------------------------------------------------
// NexusPage
// ---------------------------------------------------------------------------

export default function NexusPage() {
  const trpc = useTRPC();

  const healthQuery = useQuery(
    trpc.system.health.queryOptions(undefined, { refetchInterval: 15_000 }),
  );

  const data = (healthQuery.data as ServerHealthGetResponse | undefined) ?? null;
  const loading = healthQuery.isLoading;
  const error = healthQuery.error?.message ?? null;

  const fetchHealth = () => void healthQuery.refetch();

  const latest = data?.latest;

  // Build health tiles from snapshot
  const tiles: HealthTile[] = latest
    ? [
        {
          id: "cpu",
          label: "CPU Usage",
          value: `${(latest.cpu_percent ?? 0).toFixed(1)}%`,
          sublabel: `Load 1m: ${latest.load_avg_1m?.toFixed(2) ?? "—"}`,
          pct: latest.cpu_percent ?? 0,
          status: tileStatus(latest.cpu_percent ?? 0, 70, 90),
          icon: Cpu,
        },
        {
          id: "memory",
          label: "Memory",
          value: `${(latest.memory_used_mb ?? 0).toFixed(0)} MB`,
          sublabel: `of ${(latest.memory_total_mb ?? 0).toFixed(0)} MB total`,
          pct:
            latest.memory_total_mb && latest.memory_total_mb > 0
              ? ((latest.memory_used_mb ?? 0) / latest.memory_total_mb) * 100
              : 0,
          status: tileStatus(
            latest.memory_total_mb && latest.memory_total_mb > 0
              ? ((latest.memory_used_mb ?? 0) / latest.memory_total_mb) * 100
              : 0,
            80,
            90,
          ),
          icon: MemoryStick,
        },
        {
          id: "disk",
          label: "Disk",
          value:
            latest.disk_used_gb != null
              ? `${latest.disk_used_gb.toFixed(1)} GB`
              : "—",
          sublabel:
            latest.disk_total_gb != null
              ? `of ${latest.disk_total_gb.toFixed(1)} GB total`
              : "No disk data",
          pct:
            latest.disk_used_gb != null && latest.disk_total_gb != null
              ? (latest.disk_used_gb / latest.disk_total_gb) * 100
              : 0,
          status: tileStatus(
            latest.disk_used_gb != null && latest.disk_total_gb != null
              ? (latest.disk_used_gb / latest.disk_total_gb) * 100
              : 0,
            80,
            90,
          ),
          icon: HardDrive,
        },
        {
          id: "uptime",
          label: "Uptime",
          value: formatUptime(latest.uptime_seconds),
          sublabel: "Daemon running time",
          pct: 100,
          status: "ok",
          icon: Clock,
        },
      ]
    : [];

  const headerAction = (
    <button
      type="button"
      onClick={() => void fetchHealth()}
      disabled={loading}
      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
    >
      <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
      Refresh
    </button>
  );

  return (
    <PageShell
      title="Nexus"
      subtitle="System health and daemon status"
      action={headerAction}
    >
      <div className="space-y-3 animate-fade-in-up">
        {error && (
          <ErrorBanner
            message="Failed to load health data"
            detail={error}
            onRetry={() => void fetchHealth()}
          />
        )}

        {/* Status overview */}
        {!loading && data && (
          <div className="surface-card p-5 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Activity size={18} className="text-ds-gray-700" />
              <div>
                <p className="text-label-14 text-ds-gray-1000">
                  Daemon Status
                </p>
                <p className="text-copy-13 text-ds-gray-900">
                  Nova system health overview
                </p>
              </div>
            </div>
            <DaemonStatusBadge status={data.status} />
          </div>
        )}

        {/* Health tiles grid */}
        {loading ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            {Array.from({ length: 4 }).map((_, i) => (
              <div
                key={i}
                className="h-44 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
              />
            ))}
          </div>
        ) : tiles.length > 0 ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            {tiles.map((tile, i) => (
              <div
                key={tile.id}
                className={`animate-fade-in-up stagger-${i + 1}`}
              >
                <HealthTileCard tile={tile} />
              </div>
            ))}
          </div>
        ) : (
          <div className="surface-card p-10 flex flex-col items-center gap-3 text-center">
            <Activity size={32} className="text-ds-gray-600" />
            <p className="text-heading-16 text-ds-gray-1000">
              No health data available
            </p>
            <p className="text-copy-14 text-ds-gray-900 max-w-xs">
              Health metrics will appear once the daemon reports a snapshot.
            </p>
          </div>
        )}

        {/* Load averages tile */}
        {!loading && latest && (latest.load_avg_1m != null || latest.load_avg_5m != null) && (
          <div className="surface-card p-5 animate-fade-in-up stagger-5">
            <div className="flex items-center gap-2 mb-4">
              <Activity size={14} className="text-ds-gray-700" />
              <span className="text-label-12 text-ds-gray-900">
                Load Averages
              </span>
            </div>
            <div className="grid grid-cols-2 gap-4">
              {latest.load_avg_1m != null && (
                <div className="surface-inset p-3">
                  <p className="text-label-12 text-ds-gray-900">1 Minute</p>
                  <p className="text-heading-20 text-ds-gray-1000 mt-1 font-mono">
                    {latest.load_avg_1m.toFixed(2)}
                  </p>
                </div>
              )}
              {latest.load_avg_5m != null && (
                <div className="surface-inset p-3">
                  <p className="text-label-12 text-ds-gray-900">5 Minutes</p>
                  <p className="text-heading-20 text-ds-gray-1000 mt-1 font-mono">
                    {latest.load_avg_5m.toFixed(2)}
                  </p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </PageShell>
  );
}
