/**
 * MiniChart — inline SVG sparkline for time-series health metrics.
 * Accepts raw data points and renders a polyline scaled to the given dimensions.
 * Optional warn/crit thresholds render as subtle background bands.
 */

/** Downsample an array to at most `maxPoints` by averaging buckets. */
export function downsample(data: number[], maxPoints: number): number[] {
  if (data.length <= maxPoints) return data;
  const bucketSize = data.length / maxPoints;
  const result: number[] = [];
  for (let i = 0; i < maxPoints; i++) {
    const start = Math.floor(i * bucketSize);
    const end = Math.floor((i + 1) * bucketSize);
    const slice = data.slice(start, end);
    const avg = slice.reduce((a, b) => a + b, 0) / slice.length;
    result.push(avg);
  }
  return result;
}

interface MiniChartProps {
  data: number[];
  width?: number;
  height?: number;
  /** Percentage threshold (0-100) above which the warn band is shown */
  warnThreshold?: number;
  /** Percentage threshold (0-100) above which the critical band is shown */
  critThreshold?: number;
  /** The maximum value data can reach (used for scaling). Defaults to 100. */
  maxValue?: number;
}

/** Max points rendered — performance ceiling for the SVG polyline. */
const MAX_RENDER_POINTS = 120;

export default function MiniChart({
  data,
  width = 120,
  height = 32,
  warnThreshold,
  critThreshold,
  maxValue = 100,
}: MiniChartProps) {
  if (data.length === 0) return null;

  const points = downsample(data, MAX_RENDER_POINTS);
  const dataMax = Math.max(...points, maxValue);

  // Build SVG coordinate string
  const coords = points
    .map((v, i) => {
      const x = points.length === 1 ? 0 : (i / (points.length - 1)) * width;
      const y = height - (v / dataMax) * height;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");

  // Most recent value for line color
  const last = points[points.length - 1] ?? 0;
  const pct = (last / dataMax) * 100;
  const lineColor =
    critThreshold !== undefined && pct >= critThreshold
      ? "#EF4444"
      : warnThreshold !== undefined && pct >= warnThreshold
        ? "#F97316"
        : "#8B5CF6"; // ds-gray-1000

  // Threshold bands (expressed as Y coordinates)
  const warnY =
    warnThreshold !== undefined
      ? height - (warnThreshold / 100) * height
      : null;
  const critY =
    critThreshold !== undefined
      ? height - (critThreshold / 100) * height
      : null;

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className="overflow-visible"
    >
      {/* Warn band: from warnY to critY (or bottom if no crit) */}
      {warnY !== null && (
        <rect
          x={0}
          y={critY ?? warnY}
          width={width}
          height={critY !== null ? warnY - critY : height - warnY}
          fill="#F97316"
          opacity={0.08}
        />
      )}

      {/* Critical band: from critY to top */}
      {critY !== null && (
        <rect
          x={0}
          y={0}
          width={width}
          height={critY}
          fill="#EF4444"
          opacity={0.08}
        />
      )}

      {/* Sparkline */}
      <polyline
        points={coords}
        fill="none"
        stroke={lineColor}
        strokeWidth={1.5}
        strokeLinejoin="round"
        strokeLinecap="round"
        opacity={0.9}
      />
    </svg>
  );
}
