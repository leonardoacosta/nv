"use client";

import type { ReactNode } from "react";
import { TrendingUp, TrendingDown, Minus } from "lucide-react";

import { useCountUp } from "@/hooks/useCountUp";

export type TrendDirection = "up" | "down" | "flat";
export type StatCardVariant = "default" | "success" | "warning" | "error";

export interface StatCardProps {
  icon: ReactNode;
  label: string;
  value: string | number;
  /** Optional sub-label shown below the value */
  sublabel?: string;
  /** Semantic variant — controls the left accent bar color */
  variant?: StatCardVariant;
  /** Optional trend to display below the value */
  trend?: {
    direction: TrendDirection;
    label: string;
  };
}

const ACCENT_BAR: Record<StatCardVariant, string> = {
  default: "bg-ds-gray-600",
  success: "bg-green-700",
  warning: "bg-amber-700",
  error: "bg-red-700",
};

const TREND_CONFIG: Record<
  TrendDirection,
  { icon: ReactNode; color: string }
> = {
  up: {
    icon: <TrendingUp size={12} aria-hidden="true" />,
    color: "text-green-700",
  },
  down: {
    icon: <TrendingDown size={12} aria-hidden="true" />,
    color: "text-red-700",
  },
  flat: {
    icon: <Minus size={12} aria-hidden="true" />,
    color: "text-ds-gray-700",
  },
};

/**
 * StatCard — metric tile with Geist surface-card material.
 * Left accent bar indicates semantic status. Icon, value, label, optional trend.
 */
export default function StatCard({
  icon,
  label,
  value,
  sublabel,
  variant = "default",
  trend,
}: StatCardProps) {
  const numericValue = typeof value === "number" ? value : 0;
  const animatedValue = useCountUp(numericValue);
  const displayValue = typeof value === "number" ? animatedValue : String(value);

  const trendCfg = trend ? TREND_CONFIG[trend.direction] : null;
  const accentBar = ACCENT_BAR[variant];

  return (
    <div className="surface-card relative flex flex-col gap-3 p-4 overflow-hidden">
      {/* Left accent bar */}
      <div
        className={`absolute left-0 top-0 bottom-0 w-1 ${accentBar} rounded-l-xl`}
        aria-hidden="true"
      />

      {/* Icon + label row */}
      <div className="flex items-center gap-2.5 pl-2">
        <div
          className="flex items-center justify-center w-5 h-5 shrink-0 text-ds-gray-700"
          aria-hidden="true"
        >
          {icon}
        </div>
        <span className="text-label-12 text-ds-gray-900 truncate">
          {label}
        </span>
      </div>

      {/* Value */}
      <div className="pl-2">
        <div className="text-heading-32 text-ds-gray-1000 leading-none">
          {displayValue}
        </div>
        {sublabel && (
          <div className="mt-1 text-label-13 text-ds-gray-900">{sublabel}</div>
        )}
      </div>

      {/* Trend */}
      {trendCfg && trend && (
        <div className={`flex items-center gap-1 text-label-13 pl-2 ${trendCfg.color}`}>
          {trendCfg.icon}
          <span>{trend.label}</span>
        </div>
      )}
    </div>
  );
}
