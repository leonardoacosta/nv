"use client";

export interface ConfigSourceEntry {
  key: string;
  source: "env" | "file" | "default";
  envVar?: string;
}

interface ConfigSourceBadgeProps {
  source: ConfigSourceEntry;
}

const SOURCE_STYLES: Record<
  ConfigSourceEntry["source"],
  { label: string; className: string }
> = {
  env: {
    label: "ENV",
    className: "bg-blue-700/15 text-blue-700 border-blue-700/30",
  },
  file: {
    label: "FILE",
    className: "bg-ds-gray-alpha-200 text-ds-gray-900 border-ds-gray-400",
  },
  default: {
    label: "DEFAULT",
    className: "bg-transparent text-ds-gray-700 border-ds-gray-400 opacity-70",
  },
};

export default function ConfigSourceBadge({ source }: ConfigSourceBadgeProps) {
  const style = SOURCE_STYLES[source.source];
  if (!style) return null;

  const badge = (
    <span
      className={`inline-flex items-center px-1 py-0.5 rounded text-[9px] font-mono font-bold uppercase border ${style.className} select-none`}
    >
      {style.label}
    </span>
  );

  if (source.source === "env" && source.envVar) {
    return (
      <span
        title={`Set via environment variable: ${source.envVar}`}
        className="cursor-default"
      >
        {badge}
      </span>
    );
  }

  return badge;
}
