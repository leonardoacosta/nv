"use client";

import { useState } from "react";
import { Clock, Terminal, Radio, ChevronDown } from "lucide-react";
import type { DiaryEntryItem } from "@/types/api";
import { getPlatformColor } from "@/lib/brand-colors";

interface DiaryEntryProps {
  entry: DiaryEntryItem;
}

/* @multi-component — TriggerTypeBadge and ToolPill are tightly-coupled sub-components */

function TriggerTypeBadge({ type }: { type: string }) {
  const colors: Record<string, string> = {
    message: "bg-[#229ED9]/20 text-[#229ED9] border-[#229ED9]/30",
    cron: "bg-ds-gray-alpha-200 text-ds-gray-1000 border-ds-gray-alpha-400",
    nexus: "bg-green-700/20 text-green-700 border-green-700/30",
    cli: "bg-amber-700/20 text-amber-700 border-amber-700/30",
    research: "bg-red-700/20 text-red-700 border-red-700/30",
  };
  const cls =
    colors[type] ??
    "bg-ds-gray-alpha-200 text-ds-gray-900 border-ds-gray-alpha-400";
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-label-12 font-medium font-mono border ${cls}`}
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

/** Extract HH:MM:SS from an ISO timestamp string. */
function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    const h = String(d.getHours()).padStart(2, "0");
    const m = String(d.getMinutes()).padStart(2, "0");
    const s = String(d.getSeconds()).padStart(2, "0");
    return `${h}:${m}:${s}`;
  } catch {
    return iso;
  }
}

export default function DiaryEntry({ entry }: DiaryEntryProps) {
  const [expanded, setExpanded] = useState<boolean>(false);
  const brand = getPlatformColor(entry.channel_source || entry.trigger_source);

  return (
    <div
      className="py-2 border-b border-ds-gray-400 cursor-pointer hover:bg-ds-gray-alpha-100 transition-colors"
      onClick={() => setExpanded((prev) => !prev)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          setExpanded((prev) => !prev);
        }
      }}
    >
      {/* Collapsed row */}
      <div className="flex items-center gap-2 px-3">
        <span className="text-label-13-mono text-ds-gray-1000 tabular-nums shrink-0">
          {formatTime(entry.time)}
        </span>

        <span
          className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-label-12 font-medium ${brand.bg} ${brand.text}`}
        >
          <Radio size={10} className="shrink-0" />
          {entry.channel_source || entry.trigger_source}
        </span>

        <TriggerTypeBadge type={entry.trigger_type} />

        {entry.result_summary && (
          <span className="text-copy-13 text-ds-gray-900 truncate min-w-0 flex-1">
            {entry.result_summary}
          </span>
        )}

        <ChevronDown
          size={14}
          className={`shrink-0 text-ds-gray-700 transition-transform ${expanded ? "rotate-180" : ""}`}
        />
      </div>

      {/* Expanded content */}
      {expanded && (
        <div className="mt-2 px-3 flex flex-col gap-2">
          {/* Tool pills */}
          {entry.tools_called.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {entry.tools_called.map((tool) => (
                <ToolPill key={tool} name={tool} />
              ))}
            </div>
          )}

          {/* Full result summary in code block */}
          {entry.result_summary && (
            <pre className="overflow-x-auto bg-ds-bg-100 rounded p-3 text-xs font-mono text-ds-gray-1000 whitespace-pre-wrap">
              {entry.result_summary}
            </pre>
          )}

          {/* Metadata row: latency + tokens */}
          <div className="flex items-center gap-4 flex-wrap">
            <span className="flex items-center gap-1 text-copy-13 text-ds-gray-900 font-mono">
              <Clock size={11} className="shrink-0" />
              {entry.response_latency_ms.toLocaleString()}ms
            </span>
            <span className="text-copy-13 text-ds-gray-900 font-mono">
              {entry.tokens_in.toLocaleString()} in +{" "}
              {entry.tokens_out.toLocaleString()} out
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
