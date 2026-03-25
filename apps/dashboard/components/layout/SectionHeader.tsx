export type StatusDotColor = "green" | "amber" | "red" | "purple" | "muted";

export interface SectionHeaderProps {
  label: string;
  count?: number;
  /** Optional status dot color */
  statusDot?: StatusDotColor;
  /** Accessible label for the status dot */
  statusLabel?: string;
}

const DOT_COLOR: Record<StatusDotColor, string> = {
  green: "bg-emerald-400",
  amber: "bg-amber-400",
  red: "bg-red-400",
  purple: "bg-cosmic-purple",
  muted: "bg-cosmic-muted",
};

/**
 * SectionHeader — uppercase section label with count badge and optional status dot.
 * Used to divide and label content sections within pages.
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
          className={`inline-block w-2 h-2 rounded-full shrink-0 ${DOT_COLOR[statusDot]}`}
          aria-label={statusLabel ?? statusDot}
          role="img"
        />
      )}

      <span className="text-xs font-semibold text-cosmic-muted uppercase tracking-widest">
        {label}
      </span>

      {count !== undefined && (
        <span className="inline-flex items-center justify-center px-1.5 py-0.5 min-w-[1.25rem] rounded text-xs font-mono font-medium bg-cosmic-border text-cosmic-text">
          {count}
        </span>
      )}
    </div>
  );
}
