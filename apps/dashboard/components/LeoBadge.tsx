interface LeoBadgeProps {
  className?: string;
}

export default function LeoBadge({ className = "" }: LeoBadgeProps) {
  return (
    <span
      className={[
        "inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold font-mono",
        "bg-red-700/20 text-[#fda4af] border border-red-700/30",
        className,
      ].join(" ")}
    >
      Leo
    </span>
  );
}
