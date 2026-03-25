import { Cpu, MemoryStick, Clock, Wifi, WifiOff, AlertCircle, HardDrive, Activity } from "lucide-react";
import type { ServerHealthSnapshot, BackendHealthStatus } from "@/types/api";
import MiniChart from "@/components/MiniChart";

/**
 * HealthMetrics is the display-facing subset passed into this component.
 * All fields come from ServerHealthSnapshot + the top-level status field.
 */
export interface HealthMetrics {
  cpu_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  disk_used_gb?: number | null;
  disk_total_gb?: number | null;
  uptime_seconds: number;
  load_avg_1m?: number | null;
  load_avg_5m?: number | null;
  /** Supports both frontend display values ("ok"/"down") and backend enum values ("healthy"/"critical"). */
  status: "ok" | BackendHealthStatus | "down";
  version?: string;
  pid?: number;
}

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function MetricBar({
  label,
  value,
  max,
  unit,
  warn,
  icon: Icon,
}: {
  label: string;
  value: number;
  max: number;
  unit: string;
  warn?: number;
  icon: React.ElementType;
}) {
  const pct = Math.min(100, (value / max) * 100);
  const isWarn = warn !== undefined && pct >= warn;
  const isCrit = pct >= 90;

  const barColor = isCrit
    ? "bg-[#EF4444]"
    : isWarn
      ? "bg-[#F97316]"
      : "bg-cosmic-purple";

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5 text-xs text-cosmic-muted">
          <Icon size={12} />
          <span>{label}</span>
        </div>
        <span
          className={`text-xs font-mono ${isCrit ? "text-[#EF4444]" : isWarn ? "text-[#F97316]" : "text-cosmic-text"}`}
        >
          {value.toFixed(1)}
          {unit}
        </span>
      </div>
      <div className="h-1.5 rounded-full bg-cosmic-dark overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-700 ${barColor}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

interface ServerHealthProps {
  metrics: HealthMetrics | null;
  history?: ServerHealthSnapshot[];
  loading?: boolean;
  error?: string | null;
}

export default function ServerHealth({
  metrics,
  history,
  loading,
  error,
}: ServerHealthProps) {
  if (loading) {
    return (
      <div className="p-5 rounded-cosmic border border-cosmic-border bg-cosmic-surface space-y-4">
        <div className="h-4 w-32 animate-pulse rounded bg-cosmic-border" />
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="h-6 animate-pulse rounded bg-cosmic-border" />
        ))}
      </div>
    );
  }

  if (error || !metrics) {
    return (
      <div className="p-5 rounded-cosmic border border-cosmic-rose/30 bg-cosmic-rose/10 flex items-center gap-3 text-cosmic-rose">
        <AlertCircle size={16} />
        <span className="text-sm">{error ?? "No metrics available"}</span>
      </div>
    );
  }

  const memPct = (metrics.memory_used_mb / metrics.memory_total_mb) * 100;

  const hasDisk =
    metrics.disk_used_gb != null && metrics.disk_total_gb != null;
  const diskPct = hasDisk
    ? (metrics.disk_used_gb! / metrics.disk_total_gb!) * 100
    : 0;

  // Build sparkline data arrays from history (oldest first)
  const cpuHistory = history?.map((s) => s.cpu_percent ?? 0) ?? [];
  const memHistory = history?.map((s) =>
    s.memory_total_mb && s.memory_total_mb > 0
      ? ((s.memory_used_mb ?? 0) / s.memory_total_mb) * 100
      : 0,
  ) ?? [];
  const diskHistory = history?.map((s) =>
    s.disk_total_gb && s.disk_total_gb > 0
      ? ((s.disk_used_gb ?? 0) / s.disk_total_gb) * 100
      : 0,
  ) ?? [];

  const showCharts = history && history.length > 1;

  return (
    <div className="p-5 rounded-cosmic border border-cosmic-border bg-cosmic-surface space-y-5">
      {/* Status header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {metrics.status === "ok" || metrics.status === "healthy" ? (
            <Wifi size={16} className="text-emerald-400" />
          ) : metrics.status === "degraded" ? (
            <Wifi size={16} className="text-[#F97316]" />
          ) : (
            <WifiOff size={16} className="text-[#EF4444]" />
          )}
          <span className="text-sm font-medium text-cosmic-text">
            Daemon Health
          </span>
        </div>
        <span
          className={`text-xs px-2 py-0.5 rounded font-mono ${
            metrics.status === "ok" || metrics.status === "healthy"
              ? "bg-emerald-500/20 text-emerald-400"
              : metrics.status === "degraded"
                ? "bg-[#F97316]/20 text-[#F97316]"
                : "bg-[#EF4444]/20 text-[#EF4444]"
          }`}
        >
          {metrics.status}
        </span>
      </div>

      {/* Metrics */}
      <div className="space-y-4">
        <div>
          <MetricBar
            icon={Cpu}
            label="CPU"
            value={metrics.cpu_percent}
            max={100}
            unit="%"
            warn={70}
          />
          {showCharts && cpuHistory.length > 1 && (
            <div className="mt-1.5 opacity-70">
              <MiniChart
                data={cpuHistory}
                width={240}
                height={28}
                warnThreshold={70}
                critThreshold={90}
                maxValue={100}
              />
            </div>
          )}
        </div>

        <div>
          <MetricBar
            icon={MemoryStick}
            label="Memory"
            value={metrics.memory_used_mb}
            max={metrics.memory_total_mb}
            unit=" MB"
            warn={80}
          />
          {showCharts && memHistory.length > 1 && (
            <div className="mt-1.5 opacity-70">
              <MiniChart
                data={memHistory}
                width={240}
                height={28}
                warnThreshold={80}
                critThreshold={90}
                maxValue={100}
              />
            </div>
          )}
        </div>

        {hasDisk && (
          <div>
            <MetricBar
              icon={HardDrive}
              label="Disk"
              value={metrics.disk_used_gb!}
              max={metrics.disk_total_gb!}
              unit=" GB"
              warn={80}
            />
            {showCharts && diskHistory.length > 1 && (
              <div className="mt-1.5 opacity-70">
                <MiniChart
                  data={diskHistory}
                  width={240}
                  height={28}
                  warnThreshold={80}
                  critThreshold={90}
                  maxValue={100}
                />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Info row */}
      <div className="flex items-center justify-between pt-2 border-t border-cosmic-border">
        <div className="flex items-center gap-1.5 text-xs text-cosmic-muted font-mono">
          <Clock size={12} />
          <span>{formatUptime(metrics.uptime_seconds)}</span>
        </div>
        <div className="text-xs text-cosmic-muted font-mono">
          {metrics.memory_used_mb.toFixed(0)} /{" "}
          {metrics.memory_total_mb.toFixed(0)} MB ({memPct.toFixed(0)}%)
        </div>
      </div>

      {/* Load average row */}
      {(metrics.load_avg_1m != null || metrics.load_avg_5m != null) && (
        <div className="flex items-center gap-4 text-xs text-cosmic-muted font-mono">
          <div className="flex items-center gap-1.5">
            <Activity size={12} />
            <span>Load</span>
          </div>
          {metrics.load_avg_1m != null && (
            <span>
              1m <span className="text-cosmic-text">{metrics.load_avg_1m.toFixed(2)}</span>
            </span>
          )}
          {metrics.load_avg_5m != null && (
            <span>
              5m <span className="text-cosmic-text">{metrics.load_avg_5m.toFixed(2)}</span>
            </span>
          )}
        </div>
      )}

      {/* Disk usage summary */}
      {hasDisk && (
        <div className="flex items-center justify-between text-xs text-cosmic-muted font-mono">
          <div className="flex items-center gap-1.5">
            <HardDrive size={12} />
            <span>Disk</span>
          </div>
          <span>
            {metrics.disk_used_gb!.toFixed(1)} /{" "}
            {metrics.disk_total_gb!.toFixed(1)} GB ({diskPct.toFixed(0)}%)
          </span>
        </div>
      )}

      {/* Version / PID */}
      {(metrics.version ?? metrics.pid) && (
        <div className="flex items-center justify-between text-xs text-cosmic-muted font-mono">
          {metrics.version && <span>v{metrics.version}</span>}
          {metrics.pid && <span>PID {metrics.pid}</span>}
        </div>
      )}
    </div>
  );
}
