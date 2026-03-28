"use client";

interface SourceItem {
  name: string;
  status: "ok" | "unavailable" | "empty";
}

interface SourcePillsData {
  sources: SourceItem[];
}

interface SourcePillsProps {
  title?: string;
  data: SourcePillsData;
  className?: string;
}

function statusDotClass(status: "ok" | "unavailable" | "empty"): string {
  if (status === "ok") return "bg-green-700";
  if (status === "unavailable") return "bg-red-700";
  return "bg-ds-gray-500";
}

export default function SourcePills({ title, data, className }: SourcePillsProps) {
  return (
    <div className={`surface-card p-5 space-y-3 ${className ?? ""}`}>
      {title && (
        <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
      )}
      <div className="flex flex-wrap gap-2">
        {data.sources.map((source, i) => (
          <span
            key={i}
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-900"
          >
            <span
              className={`w-2 h-2 rounded-full shrink-0 ${statusDotClass(source.status)}`}
            />
            {source.name}
          </span>
        ))}
      </div>
    </div>
  );
}
