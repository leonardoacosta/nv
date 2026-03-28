"use client";

import { useState } from "react";
import { Clock, Terminal, Radio, ChevronDown, DollarSign, Zap } from "lucide-react";
import type { DiaryEntryItem, ToolCallDetail } from "@/types/api";
import { getPlatformColor } from "@/lib/brand-colors";

interface DiaryEntryProps {
  entry: DiaryEntryItem;
}

/* @multi-component — TriggerTypeBadge, ToolPill, MetaBadge, ToolDetailRow are tightly-coupled sub-components */

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

/** Compact metadata badge for collapsed row. */
function MetaBadge({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return (
    <span
      className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-ds-gray-alpha-200 text-label-12 font-mono text-ds-gray-900 shrink-0 ${className}`}
    >
      {children}
    </span>
  );
}

/** Format token count: 1234 → "1.2k", 123 → "123". */
function fmtTokens(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

/** Format cost: 0.00123 → "$0.001", 1.23 → "$1.23". */
function fmtCost(n: number): string {
  if (n < 0.01) return `$${n.toFixed(4)}`;
  if (n < 1) return `$${n.toFixed(3)}`;
  return `$${n.toFixed(2)}`;
}

/** Format latency: 1234 → "1.2s", 500 → "500ms". */
function fmtLatency(ms: number): string {
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
  return `${ms}ms`;
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

/** Expanded tool detail row with name, input summary, and duration. */
function ToolDetailRow({ tool }: { tool: ToolCallDetail }) {
  return (
    <div className="flex items-start gap-2 py-1 border-b border-ds-gray-alpha-200 last:border-b-0">
      <Terminal size={10} className="shrink-0 mt-1 text-ds-gray-700" />
      <div className="flex-1 min-w-0">
        <span className="text-label-12 font-mono text-ds-gray-1000">{tool.name}</span>
        {tool.input_summary && (
          <p className="text-copy-12 text-ds-gray-900 mt-0.5 truncate">{tool.input_summary}</p>
        )}
      </div>
      {tool.duration_ms != null && (
        <span className="text-label-12 font-mono text-ds-gray-700 shrink-0">
          {fmtLatency(tool.duration_ms)}
        </span>
      )}
    </div>
  );
}

export default function DiaryEntry({ entry }: DiaryEntryProps) {
  const [expanded, setExpanded] = useState<boolean>(false);
  const brand = getPlatformColor(entry.channel_source || entry.trigger_source);

  // Determine which tool data to use in collapsed row: prefer tools_detail names, fall back to tools_called
  const toolNames: string[] =
    entry.tools_detail.length > 0
      ? entry.tools_detail.map((t) => t.name)
      : entry.tools_called;

  const visibleTools = toolNames.slice(0, 3);
  const overflowCount = toolNames.length - visibleTools.length;

  // Determine if we have structured tool details for expanded view
  const hasStructuredTools = entry.tools_detail.length > 0;

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

        {/* Metadata badges — hidden below sm breakpoint (task 3.2) */}
        <div className="hidden sm:flex items-center gap-1.5 shrink-0">
          {/* Token badge: in+out */}
          {(entry.tokens_in > 0 || entry.tokens_out > 0) && (
            <MetaBadge>
              <Zap size={9} />
              {fmtTokens(entry.tokens_in)}+{fmtTokens(entry.tokens_out)}
            </MetaBadge>
          )}

          {/* Latency badge */}
          {entry.response_latency_ms > 0 && (
            <MetaBadge>
              <Clock size={9} />
              {fmtLatency(entry.response_latency_ms)}
            </MetaBadge>
          )}

          {/* Cost badge */}
          {entry.cost_usd != null && entry.cost_usd > 0 && (
            <MetaBadge>
              <DollarSign size={9} />
              {fmtCost(entry.cost_usd)}
            </MetaBadge>
          )}

          {/* Tool pills (up to 3) + overflow */}
          {visibleTools.map((name) => (
            <span
              key={name}
              className="hidden md:inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-ds-bg-100 border border-ds-gray-400 text-label-12 font-mono text-ds-gray-900 shrink-0"
            >
              <Terminal size={9} />
              {name}
            </span>
          ))}
          {overflowCount > 0 && (
            <span className="hidden md:inline-flex items-center px-1.5 py-0.5 rounded bg-ds-gray-alpha-200 text-label-12 font-mono text-ds-gray-900 shrink-0">
              +{overflowCount}
            </span>
          )}
        </div>

        <ChevronDown
          size={14}
          className={`shrink-0 text-ds-gray-700 transition-transform ${expanded ? "rotate-180" : ""}`}
        />
      </div>

      {/* Expanded content */}
      {expanded && (
        <div className="mt-2 px-3 flex flex-col gap-2">
          {/* Structured tool details (new format) */}
          {hasStructuredTools && (
            <div className="rounded-md border border-ds-gray-alpha-200 divide-y divide-ds-gray-alpha-200 overflow-hidden">
              {entry.tools_detail.map((tool, i) => (
                <ToolDetailRow key={`${tool.name}-${i}`} tool={tool} />
              ))}
            </div>
          )}

          {/* Legacy flat tool pills (old format fallback) */}
          {!hasStructuredTools && entry.tools_called.length > 0 && (
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

          {/* Metadata row: latency + tokens + cost + model */}
          <div className="flex items-center gap-4 flex-wrap">
            <span className="flex items-center gap-1 text-copy-13 text-ds-gray-900 font-mono">
              <Clock size={11} className="shrink-0" />
              {entry.response_latency_ms.toLocaleString()}ms
            </span>
            <span className="text-copy-13 text-ds-gray-900 font-mono">
              {entry.tokens_in.toLocaleString()} in +{" "}
              {entry.tokens_out.toLocaleString()} out
            </span>
            {entry.cost_usd != null && entry.cost_usd > 0 && (
              <span className="text-copy-13 text-ds-gray-700 font-mono">
                {fmtCost(entry.cost_usd)}
              </span>
            )}
            {entry.model && (
              <span className="text-copy-13 text-ds-gray-700 font-mono">
                {entry.model}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
