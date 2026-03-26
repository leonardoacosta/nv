import { Clock, Terminal, Radio } from "lucide-react";
import type { DiaryEntryItem } from "@/types/api";

interface DiaryEntryProps {
  entry: DiaryEntryItem;
}

function TriggerTypeBadge({ type }: { type: string }) {
  const colors: Record<string, string> = {
    message: "bg-[#229ED9]/20 text-[#229ED9] border-[#229ED9]/30",
    cron: "bg-ds-gray-alpha-200 text-ds-gray-1000 border-ds-gray-1000/30",
    nexus: "bg-emerald-500/20 text-emerald-400 border-emerald-500/30",
    cli: "bg-amber-500/20 text-amber-400 border-amber-500/30",
    research: "bg-red-700/20 text-red-700 border-red-700/30",
  };
  const cls = colors[type] ?? "bg-ds-gray-alpha-200 text-ds-gray-900 border-ds-gray-400";
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium font-mono border ${cls}`}
    >
      {type}
    </span>
  );
}

function ToolPill({ name }: { name: string }) {
  return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-ds-bg-100 border border-ds-gray-400 text-xs font-mono text-ds-gray-900">
      <Terminal size={10} className="shrink-0" />
      {name}
    </span>
  );
}

export default function DiaryEntry({ entry }: DiaryEntryProps) {
  return (
    <div className="p-4 rounded-xl bg-ds-gray-100 border border-ds-gray-400 hover:border-ds-gray-1000/30 transition-colors">
      {/* Heading row */}
      <div className="flex items-center gap-2 flex-wrap mb-2">
        <span className="text-sm font-mono font-semibold text-ds-gray-1000 tabular-nums">
          {entry.time}
        </span>
        <TriggerTypeBadge type={entry.trigger_type} />
        <span className="flex items-center gap-1 text-xs text-ds-gray-900">
          <Radio size={11} className="shrink-0" />
          {entry.channel_source || entry.trigger_source}
        </span>
        <span className="text-xs text-ds-gray-900">&middot;</span>
        <span className="text-xs font-mono text-ds-gray-1000 truncate max-w-xs">
          {entry.slug}
        </span>
      </div>

      {/* Tools pills */}
      {entry.tools_called.length > 0 && (
        <div className="flex flex-wrap gap-1.5 mb-2">
          {entry.tools_called.map((tool) => (
            <ToolPill key={tool} name={tool} />
          ))}
        </div>
      )}

      {/* Result summary */}
      {entry.result_summary && (
        <p className="text-sm text-ds-gray-1000 mb-2 leading-snug">
          {entry.result_summary}
        </p>
      )}

      {/* Metadata row: latency + token cost */}
      <div className="flex items-center gap-4 flex-wrap mt-1">
        <span className="flex items-center gap-1 text-xs text-ds-gray-900 font-mono">
          <Clock size={11} className="shrink-0" />
          {entry.response_latency_ms.toLocaleString()}ms
        </span>
        <span className="text-xs text-ds-gray-900 font-mono">
          {entry.tokens_in.toLocaleString()} in + {entry.tokens_out.toLocaleString()} out
        </span>
      </div>
    </div>
  );
}
