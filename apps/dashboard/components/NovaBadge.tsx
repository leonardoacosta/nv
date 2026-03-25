interface NovaBadgeProps {
  className?: string;
}

export default function NovaBadge({ className = "" }: NovaBadgeProps) {
  return (
    <span
      className={[
        "inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold font-mono",
        "bg-cosmic-purple/20 text-[#c4b5fd] border border-cosmic-purple/30",
        className,
      ].join(" ")}
    >
      Nova
    </span>
  );
}
