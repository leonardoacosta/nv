interface NovaBadgeProps {
  className?: string;
}

export default function NovaBadge({ className = "" }: NovaBadgeProps) {
  return (
    <span
      className={[
        "inline-flex items-center px-2 py-0.5 rounded text-xs font-semibold font-mono",
        "bg-ds-gray-alpha-200 text-[#c4b5fd] border border-ds-gray-1000/30",
        className,
      ].join(" ")}
    >
      Nova
    </span>
  );
}
