"use client";

interface PRItem {
  title: string;
  repo: string;
  url?: string;
  status: "open" | "merged" | "closed";
}

interface PRListData {
  prs: PRItem[];
}

interface PRListProps {
  title?: string;
  data: PRListData;
  className?: string;
}

function statusColor(status: "open" | "merged" | "closed"): string {
  if (status === "merged") return "bg-ds-blue-700/10 text-ds-blue-700 border-ds-blue-700/30";
  if (status === "closed") return "bg-ds-red-700/10 text-ds-red-700 border-ds-red-700/30";
  return "bg-ds-green-700/10 text-ds-green-700 border-ds-green-700/30";
}

function statusLabel(status: "open" | "merged" | "closed"): string {
  if (status === "merged") return "Merged";
  if (status === "closed") return "Closed";
  return "Open";
}

function PRDot({ status }: { status: "open" | "merged" | "closed" }) {
  const color =
    status === "merged"
      ? "bg-ds-blue-700"
      : status === "closed"
        ? "bg-ds-red-700"
        : "bg-ds-green-700";
  return <span className={`w-2 h-2 rounded-full shrink-0 ${color}`} />;
}

export default function PRList({ title, data, className }: PRListProps) {
  return (
    <div className={`surface-card overflow-hidden ${className ?? ""}`}>
      {title && (
        <div className="px-5 pt-4 pb-2">
          <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
        </div>
      )}
      <ul className="divide-y divide-ds-gray-400">
        {data.prs.map((pr, i) => {
          const inner = (
            <div className="flex items-start gap-3 px-5 py-3 hover:bg-ds-gray-100 transition-colors">
              <PRDot status={pr.status} />
              <div className="flex-1 min-w-0 space-y-0.5">
                <p className="text-copy-13 text-ds-gray-1000 leading-tight truncate">
                  {pr.title}
                </p>
                <p className="text-label-12 text-ds-gray-700">{pr.repo}</p>
              </div>
              <span
                className={`px-2 py-0.5 rounded-full border text-label-12 shrink-0 ${statusColor(pr.status)}`}
              >
                {statusLabel(pr.status)}
              </span>
            </div>
          );

          return pr.url ? (
            <li key={i}>
              <a href={pr.url} target="_blank" rel="noopener noreferrer">
                {inner}
              </a>
            </li>
          ) : (
            <li key={i}>{inner}</li>
          );
        })}
      </ul>
    </div>
  );
}
