import { Clock, User, Tag } from "lucide-react";

export type Priority = 0 | 1 | 2 | 3 | 4;

export interface Obligation {
  id: string;
  title: string;
  description?: string;
  priority: Priority;
  owner: "nova" | "leo" | string;
  status: "open" | "in_progress" | "completed" | "dismissed";
  due_at?: string;
  tags?: string[];
  created_at: string;
}

const PRIORITY_CONFIG: Record<
  Priority,
  { label: string; color: string; bar: string; bg: string }
> = {
  0: {
    label: "P0",
    color: "text-[#EF4444]",
    bar: "bg-[#EF4444]",
    bg: "bg-[#EF4444]/10 border-[#EF4444]/30",
  },
  1: {
    label: "P1",
    color: "text-[#F97316]",
    bar: "bg-[#F97316]",
    bg: "bg-[#F97316]/10 border-[#F97316]/30",
  },
  2: {
    label: "P2",
    color: "text-ds-gray-1000",
    bar: "bg-ds-gray-700",
    bg: "bg-ds-gray-alpha-100 border-ds-gray-1000/30",
  },
  3: {
    label: "P3",
    color: "text-[#6B7280]",
    bar: "bg-[#6B7280]",
    bg: "bg-[#6B7280]/10 border-[#6B7280]/20",
  },
  4: {
    label: "P4",
    color: "text-[#374151]",
    bar: "bg-[#374151]",
    bg: "bg-[#374151]/10 border-[#374151]/20",
  },
};

interface ObligationItemProps {
  obligation: Obligation;
}

export default function ObligationItem({ obligation }: ObligationItemProps) {
  const p = PRIORITY_CONFIG[obligation.priority] ?? PRIORITY_CONFIG[2];

  return (
    <div
      className={`flex gap-3 p-4 rounded-xl border transition-colors hover:border-ds-gray-1000/40 ${p.bg}`}
    >
      {/* Priority bar */}
      <div className={`w-1 rounded-full shrink-0 self-stretch ${p.bar}`} />

      <div className="flex-1 min-w-0">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 flex-wrap">
            <span
              className={`text-xs font-mono font-bold uppercase ${p.color}`}
            >
              {p.label}
            </span>
            <span className="text-sm font-medium text-ds-gray-1000">
              {obligation.title}
            </span>
          </div>
          <span
            className={`text-xs px-2 py-0.5 rounded font-mono shrink-0 ${
              obligation.status === "in_progress"
                ? "bg-amber-500/20 text-amber-400"
                : obligation.status === "open"
                  ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                  : "bg-ds-gray-alpha-200 text-ds-gray-900"
            }`}
          >
            {obligation.status}
          </span>
        </div>

        {obligation.description && (
          <p className="mt-1 text-xs text-ds-gray-900 line-clamp-2">
            {obligation.description}
          </p>
        )}

        <div className="flex items-center gap-4 mt-2 flex-wrap">
          {obligation.due_at && (
            <div className="flex items-center gap-1 text-xs text-ds-gray-900 font-mono">
              <Clock size={11} />
              <span suppressHydrationWarning>
                {new Date(obligation.due_at).toLocaleDateString()}
              </span>
            </div>
          )}
          <div className="flex items-center gap-1 text-xs text-ds-gray-900">
            <User size={11} />
            <span className="capitalize">{obligation.owner}</span>
          </div>
          {obligation.tags && obligation.tags.length > 0 && (
            <div className="flex items-center gap-1">
              <Tag size={11} className="text-ds-gray-900" />
              <div className="flex gap-1 flex-wrap">
                {obligation.tags.map((tag) => (
                  <span
                    key={tag}
                    className="text-xs px-1.5 py-0.5 rounded bg-ds-gray-100 text-ds-gray-900 font-mono"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
