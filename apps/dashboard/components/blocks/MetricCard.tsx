"use client";

interface MetricCardData {
  label: string;
  value: string | number;
  unit?: string;
  trend?: "up" | "down" | "flat";
  delta?: string;
}

interface MetricCardProps {
  title?: string;
  data: MetricCardData;
  className?: string;
}

function TrendArrow({ trend }: { trend: "up" | "down" | "flat" }) {
  if (trend === "up") {
    return (
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" className="text-ds-green-700 shrink-0">
        <path d="M6 2L10 8H2L6 2Z" fill="currentColor" />
      </svg>
    );
  }
  if (trend === "down") {
    return (
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" className="text-ds-red-700 shrink-0">
        <path d="M6 10L2 4H10L6 10Z" fill="currentColor" />
      </svg>
    );
  }
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" className="text-ds-gray-700 shrink-0">
      <path d="M2 6H10M7 3L10 6L7 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function trendColor(trend?: "up" | "down" | "flat"): string {
  if (trend === "up") return "text-ds-green-700";
  if (trend === "down") return "text-ds-red-700";
  return "text-ds-gray-700";
}

export default function MetricCard({ title, data, className }: MetricCardProps) {
  return (
    <div className={`surface-card p-5 space-y-1 ${className ?? ""}`}>
      {title && (
        <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
      )}
      <p className="text-label-12 text-ds-gray-700">{data.label}</p>
      <div className="flex items-end gap-2">
        <span className="text-heading-20 text-ds-gray-1000 leading-none">
          {data.value}
          {data.unit && (
            <span className="text-copy-13 text-ds-gray-700 ml-1">{data.unit}</span>
          )}
        </span>
        {data.trend && data.trend !== "flat" && (
          <div className={`flex items-center gap-1 pb-0.5 ${trendColor(data.trend)}`}>
            <TrendArrow trend={data.trend} />
            {data.delta && (
              <span className="text-label-12">{data.delta}</span>
            )}
          </div>
        )}
        {data.trend === "flat" && data.delta && (
          <span className={`text-label-12 pb-0.5 ${trendColor(data.trend)}`}>{data.delta}</span>
        )}
      </div>
    </div>
  );
}
