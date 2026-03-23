import { Cpu, MemoryStick, Clock, Wifi, WifiOff, AlertCircle } from "lucide-react";

export interface HealthMetrics {
  cpu_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  uptime_seconds: number;
  status: "ok" | "degraded" | "down";
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
  loading?: boolean;
  error?: string | null;
}

export default function ServerHealth({
  metrics,
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

  return (
    <div className="p-5 rounded-cosmic border border-cosmic-border bg-cosmic-surface space-y-5">
      {/* Status header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {metrics.status === "ok" ? (
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
            metrics.status === "ok"
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
        <MetricBar
          icon={Cpu}
          label="CPU"
          value={metrics.cpu_percent}
          max={100}
          unit="%"
          warn={70}
        />
        <MetricBar
          icon={MemoryStick}
          label="Memory"
          value={metrics.memory_used_mb}
          max={metrics.memory_total_mb}
          unit=" MB"
          warn={80}
        />
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
