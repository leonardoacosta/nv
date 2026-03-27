"use client";

import { useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  ChevronDown,
  ChevronRight,
  Globe,
  MessageSquare,
  Terminal,
} from "lucide-react";
import type { SessionEventItem } from "@/types/api";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch {
    return iso;
  }
}

function truncateContent(text: string, maxLen = 200): string {
  if (text.length <= maxLen) return text;
  return `${text.slice(0, maxLen)}...`;
}

const STATUS_CODE_COLOR: Record<string, string> = {
  "2": "bg-green-700/15 text-green-700",
  "3": "bg-blue-700/15 text-blue-700",
  "4": "bg-amber-700/15 text-amber-700",
  "5": "bg-red-700/15 text-red-700",
};

function getStatusCodeColor(code: string | number): string {
  const first = String(code).charAt(0);
  return STATUS_CODE_COLOR[first] ?? "bg-ds-gray-alpha-200 text-ds-gray-1000";
}

// ---------------------------------------------------------------------------
// MessageEvent
// ---------------------------------------------------------------------------

function MessageEvent({ event }: { event: SessionEventItem }) {
  const isInbound = event.direction === "user" || event.direction === "inbound";

  return (
    <div className="flex gap-3">
      <div className="flex flex-col items-center gap-1 shrink-0">
        <div
          className={`flex items-center justify-center size-7 rounded-full ${
            isInbound
              ? "bg-red-700/15 text-red-700"
              : "bg-ds-gray-alpha-200 text-ds-gray-1000"
          }`}
        >
          {isInbound ? <ArrowDown size={13} /> : <ArrowUp size={13} />}
        </div>
        <div className="flex-1 w-px bg-ds-gray-400" />
      </div>
      <div className="flex-1 min-w-0 pb-4">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-label-12 text-ds-gray-900 uppercase tracking-wide">
            {isInbound ? "User" : "Assistant"}
          </span>
          <span
            className="text-copy-13 text-ds-gray-700 font-mono"
            suppressHydrationWarning
          >
            {formatTime(event.created_at)}
          </span>
        </div>
        <p className="text-copy-14 text-ds-gray-1000 leading-relaxed whitespace-pre-wrap break-words">
          {event.content ?? ""}
        </p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ToolCallEvent
// ---------------------------------------------------------------------------

function ToolCallEvent({ event }: { event: SessionEventItem }) {
  const [expanded, setExpanded] = useState(false);
  const meta = event.metadata ?? {};
  const toolName = (meta.tool_name as string) ?? event.content ?? "tool";
  const inputs = meta.inputs != null ? JSON.stringify(meta.inputs, null, 2) : null;
  const outputs = meta.outputs != null ? JSON.stringify(meta.outputs, null, 2) : null;

  return (
    <div className="flex gap-3">
      <div className="flex flex-col items-center gap-1 shrink-0">
        <div className="flex items-center justify-center size-7 rounded-full bg-amber-700/15 text-amber-700">
          <Terminal size={13} />
        </div>
        <div className="flex-1 w-px bg-ds-gray-400" />
      </div>
      <div className="flex-1 min-w-0 pb-4">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-copy-14 font-medium text-ds-gray-1000 font-mono">
            {toolName}
          </span>
          <span
            className="text-copy-13 text-ds-gray-700 font-mono"
            suppressHydrationWarning
          >
            {formatTime(event.created_at)}
          </span>
        </div>

        {/* Truncated preview */}
        {inputs && !expanded && (
          <p className="text-copy-13 text-ds-gray-900 font-mono truncate">
            {truncateContent(inputs, 120)}
          </p>
        )}

        {/* Expand/collapse toggle */}
        {(inputs || outputs) && (
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="flex items-center gap-1 mt-1 text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
          >
            {expanded ? (
              <ChevronDown size={12} />
            ) : (
              <ChevronRight size={12} />
            )}
            {expanded ? "Collapse" : "Show full I/O"}
          </button>
        )}

        {/* Expanded content */}
        {expanded && (
          <div className="mt-2 flex flex-col gap-2">
            {inputs && (
              <div className="rounded-lg bg-ds-gray-100 border border-ds-gray-400 p-3 overflow-x-auto">
                <p className="text-[11px] text-ds-gray-700 uppercase tracking-widest font-medium mb-1">
                  Input
                </p>
                <pre className="text-copy-13 text-ds-gray-1000 font-mono whitespace-pre-wrap break-all">
                  {inputs}
                </pre>
              </div>
            )}
            {outputs && (
              <div className="rounded-lg bg-ds-gray-100 border border-ds-gray-400 p-3 overflow-x-auto">
                <p className="text-[11px] text-ds-gray-700 uppercase tracking-widest font-medium mb-1">
                  Output
                </p>
                <pre className="text-copy-13 text-ds-gray-1000 font-mono whitespace-pre-wrap break-all">
                  {outputs}
                </pre>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ApiRequestEvent
// ---------------------------------------------------------------------------

function ApiRequestEvent({ event }: { event: SessionEventItem }) {
  const meta = event.metadata ?? {};
  const method = (meta.method as string) ?? "GET";
  const endpoint = (meta.endpoint as string) ?? event.content ?? "";
  const statusCode = meta.status_code != null ? String(meta.status_code) : null;

  return (
    <div className="flex gap-3">
      <div className="flex flex-col items-center gap-1 shrink-0">
        <div className="flex items-center justify-center size-7 rounded-full bg-blue-700/15 text-blue-700">
          <Globe size={13} />
        </div>
        <div className="flex-1 w-px bg-ds-gray-400" />
      </div>
      <div className="flex-1 min-w-0 pb-4">
        <div className="flex items-center gap-2 flex-wrap mb-1">
          <span className="text-copy-13 font-mono font-medium text-ds-gray-1000 uppercase">
            {method}
          </span>
          <span className="text-copy-13 font-mono text-ds-gray-900 truncate">
            {endpoint}
          </span>
          {statusCode && (
            <span
              className={`inline-flex items-center px-2 py-0.5 rounded text-label-12 font-mono font-medium ${getStatusCodeColor(statusCode)}`}
            >
              {statusCode}
            </span>
          )}
          <span
            className="text-copy-13 text-ds-gray-700 font-mono"
            suppressHydrationWarning
          >
            {formatTime(event.created_at)}
          </span>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// GenericEvent (fallback)
// ---------------------------------------------------------------------------

function GenericEvent({ event }: { event: SessionEventItem }) {
  return (
    <div className="flex gap-3">
      <div className="flex flex-col items-center gap-1 shrink-0">
        <div className="flex items-center justify-center size-7 rounded-full bg-ds-gray-alpha-200 text-ds-gray-1000">
          <MessageSquare size={13} />
        </div>
        <div className="flex-1 w-px bg-ds-gray-400" />
      </div>
      <div className="flex-1 min-w-0 pb-4">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-label-12 text-ds-gray-900 uppercase tracking-wide">
            {event.event_type}
          </span>
          <span
            className="text-copy-13 text-ds-gray-700 font-mono"
            suppressHydrationWarning
          >
            {formatTime(event.created_at)}
          </span>
        </div>
        {event.content && (
          <p className="text-copy-13 text-ds-gray-1000 whitespace-pre-wrap break-words">
            {event.content}
          </p>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Exported component — routes to the correct renderer
// ---------------------------------------------------------------------------

interface SessionTimelineEventProps {
  event: SessionEventItem;
}

export default function SessionTimelineEvent({
  event,
}: SessionTimelineEventProps) {
  switch (event.event_type) {
    case "message":
      return <MessageEvent event={event} />;
    case "tool_call":
    case "tool_result":
      return <ToolCallEvent event={event} />;
    case "api_request":
      return <ApiRequestEvent event={event} />;
    default:
      return <GenericEvent event={event} />;
  }
}
