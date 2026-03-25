import type { ReactNode } from "react";
import { TrendingUp, TrendingDown, Minus } from "lucide-react";

export type TrendDirection = "up" | "down" | "flat";

export interface StatCardProps {
  icon: ReactNode;
  label: string;
  value: string | number;
  /** Optional sub-label shown below the value */
  sublabel?: string;
  /** Accent color class applied to the icon container background — e.g. "bg-cosmic-purple/20" */
  accentBg?: string;
  /** Accent color class for the icon — e.g. "text-cosmic-purple" */
  accentText?: string;
  /** Optional trend to display below the value */
  trend?: {
    direction: TrendDirection;
    label: string;
  };
}

const TREND_CONFIG: Record<
  TrendDirection,
  { icon: ReactNode; color: string }
> = {
  up: {
    icon: <TrendingUp size={12} aria-hidden="true" />,
    color: "text-emerald-400",
  },
  down: {
    icon: <TrendingDown size={12} aria-hidden="true" />,
    color: "text-cosmic-rose",
  },
  flat: {
    icon: <Minus size={12} aria-hidden="true" />,
    color: "text-cosmic-muted",
  },
};

/**
 * StatCard — metric tile: icon, label, value, optional accent and trend.
 * Used in dashboard overview grids.
 */
export default function StatCard({
  icon,
  label,
  value,
  sublabel,
  accentBg = "bg-cosmic-purple/20",
  accentText = "text-cosmic-purple",
  trend,
}: StatCardProps) {
  const trendCfg = trend ? TREND_CONFIG[trend.direction] : null;

  return (
    <div className="flex flex-col gap-3 p-4 rounded-cosmic border border-cosmic-border bg-cosmic-surface">
      {/* Icon + label row */}
      <div className="flex items-center gap-2.5">
        <div
          className={`flex items-center justify-center w-8 h-8 rounded-lg shrink-0 ${accentBg} ${accentText}`}
          aria-hidden="true"
        >
          {icon}
        </div>
        <span className="text-xs font-medium text-cosmic-muted uppercase tracking-wide truncate">
          {label}
        </span>
      </div>

      {/* Value */}
      <div>
        <div className="text-2xl font-semibold text-cosmic-bright font-mono leading-none">
          {value}
        </div>
        {sublabel && (
          <div className="mt-0.5 text-xs text-cosmic-muted">{sublabel}</div>
        )}
      </div>

      {/* Trend */}
      {trendCfg && trend && (
        <div className={`flex items-center gap-1 text-xs ${trendCfg.color}`}>
          {trendCfg.icon}
          <span>{trend.label}</span>
        </div>
      )}
    </div>
  );
}
