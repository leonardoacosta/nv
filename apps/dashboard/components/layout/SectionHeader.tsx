export type StatusDotColor = "green" | "amber" | "red" | "blue" | "muted";

export interface SectionHeaderProps {
  label: string;
  count?: number;
  /** Optional status dot color */
  statusDot?: StatusDotColor;
  /** Accessible label for the status dot */
  statusLabel?: string;
}

const DOT_COLOR: Record<StatusDotColor, string> = {
  green: "bg-green-700",
  amber: "bg-amber-700",
  red: "bg-red-700",
  blue: "bg-blue-700",
  muted: "bg-ds-gray-600",
};

/**
 * SectionHeader — uppercase section label with count badge and optional status dot.
 * Geist text-label-12 style: 12px, 500 weight, 0.05em tracking, uppercase.
 */
export default function SectionHeader({
  label,
  count,
  statusDot,
  statusLabel,
}: SectionHeaderProps) {
  return (
    <div className="flex items-center gap-2.5 py-1">
      {statusDot && (
        <span
          className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${DOT_COLOR[statusDot]}`}
          aria-label={statusLabel ?? statusDot}
          role="img"
        />
      )}

      <span className="text-label-12 text-ds-gray-700">
        {label}
      </span>

      {count !== undefined && (
        <span
          className="inline-flex items-center justify-center px-1.5 py-0.5 min-w-[1.25rem] rounded text-xs font-mono font-medium text-ds-gray-900"
          style={{ background: "var(--ds-gray-alpha-200)" }}
        >
          {count}
        </span>
      )}
    </div>
  );
}
