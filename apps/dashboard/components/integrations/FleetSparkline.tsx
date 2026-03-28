"use client";

/**
 * FleetSparkline — 48x16px inline SVG sparkline for fleet service health history.
 *
 * 96 data points mapped from fleetHistory snapshots.
 * Color encoding: green=healthy, red=unreachable/unhealthy, gray=missing (no data).
 * Hover shows uptime_pct_24h via title attribute.
 */

interface SparklineSnapshot {
  status: string;
  time: string;
}

interface FleetSparklineProps {
  snapshots: SparklineSnapshot[];
  uptimePct: number;
  /** Total slots to render (fills gaps with gray). Default 96. */
  slots?: number;
}

const WIDTH = 48;
const HEIGHT = 16;
const BAR_GAP = 0.5;

function statusColor(status: string): string {
  if (status === "healthy") return "var(--ds-green-700, #1a7f37)";
  if (status === "unreachable" || status === "unhealthy") return "var(--ds-red-700, #cf222e)";
  return "var(--ds-gray-400, #888)";
}

export default function FleetSparkline({
  snapshots,
  uptimePct,
  slots = 96,
}: FleetSparklineProps) {
  // Normalize to `slots` bars — pad with "missing" entries if fewer snapshots
  const normalized: { status: string }[] = Array.from({ length: slots }, (_, i) => {
    const snap = snapshots[i];
    return snap ? { status: snap.status } : { status: "missing" };
  });

  const barW = (WIDTH - BAR_GAP * (slots - 1)) / slots;

  return (
    <svg
      width={WIDTH}
      height={HEIGHT}
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      aria-label={`Service uptime: ${uptimePct}% over last 24h`}
      role="img"
    >
      <title>{uptimePct}% uptime (24h)</title>
      {normalized.map((bar, i) => {
        const x = i * (barW + BAR_GAP);
        const color = bar.status === "missing" ? "var(--ds-gray-300, #ccc)" : statusColor(bar.status);
        return (
          <rect
            key={i}
            x={x}
            y={0}
            width={barW}
            height={HEIGHT}
            fill={color}
            rx={1}
          />
        );
      })}
    </svg>
  );
}
