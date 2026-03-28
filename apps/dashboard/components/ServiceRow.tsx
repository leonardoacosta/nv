"use client";

import { useState } from "react";
import { ChevronRight } from "lucide-react";
import type { FleetServiceStatus } from "@/types/api";
import FleetSparkline from "@/components/integrations/FleetSparkline";

const STATUS_DOT: Record<FleetServiceStatus["status"], string> = {
  healthy: "bg-green-700",
  unreachable: "bg-red-700",
  unknown: "bg-ds-gray-500",
};

const STATUS_LABEL: Record<FleetServiceStatus["status"], string> = {
  healthy: "healthy",
  unreachable: "unreachable",
  unknown: "host only",
};

interface SparklineData {
  snapshots: { status: string; time: string }[];
  uptime_pct_24h: number;
}

interface ServiceRowProps {
  service: FleetServiceStatus;
  lastChecked: Date | null;
  /** Optional sparkline history data from fleetHistory tRPC query. */
  sparkline?: SparklineData;
}

function formatRelativeTime(date: Date): string {
  const diffMs = Date.now() - date.getTime();
  const secs = Math.floor(diffMs / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  return `${hours}h ago`;
}

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function formatLastChecked(iso: string): string {
  try {
    const d = new Date(iso);
    const diffMs = Date.now() - d.getTime();
    const secs = Math.floor(diffMs / 1000);
    if (secs < 60) return `${secs}s ago`;
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    return `${hours}h ago`;
  } catch {
    return iso;
  }
}

export default function ServiceRow({ service, lastChecked, sparkline }: ServiceRowProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="rounded-md border border-transparent hover:border-ds-gray-alpha-200 transition-colors">
      {/* Main row */}
      <button
        type="button"
        onClick={() => setExpanded((prev) => !prev)}
        className="flex items-center gap-3 px-3 py-2 min-h-9 w-full text-left hover:bg-ds-gray-alpha-100 rounded-md transition-colors"
      >
        {/* Expand chevron */}
        <ChevronRight
          size={12}
          className={`shrink-0 text-ds-gray-700 transition-transform duration-150 ${
            expanded ? "rotate-90" : ""
          }`}
        />

        {/* Status dot — CSS transition on background-color (300ms ease) */}
        <span
          className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 transition-[background-color] duration-300 ease-in-out ${STATUS_DOT[service.status]}`}
          aria-label={service.status}
        />

        {/* Service name */}
        <span className="text-label-14 text-ds-gray-1000 flex-1 truncate">
          {service.name}
        </span>

        {/* Sparkline (48x16px) from fleetHistory */}
        {sparkline && sparkline.snapshots.length > 0 && (
          <span className="shrink-0">
            <FleetSparkline
              snapshots={sparkline.snapshots}
              uptimePct={sparkline.uptime_pct_24h}
            />
          </span>
        )}

        {/* Uptime (when available) */}
        {service.uptime_secs != null && (
          <span className="text-label-12 text-ds-gray-700 font-mono shrink-0">
            up {formatUptime(service.uptime_secs)}
          </span>
        )}

        {/* Port */}
        <span className="text-label-12 text-ds-gray-700 font-mono shrink-0">
          :{service.port}
        </span>

        {/* Latency */}
        {service.latency_ms != null && (
          <span className="text-label-12 text-ds-gray-900 font-mono shrink-0 w-12 text-right">
            {service.latency_ms}ms
          </span>
        )}

        {/* Tool count */}
        {service.tools.length > 0 && (
          <span className="px-1.5 py-0.5 rounded-full bg-ds-gray-alpha-200 text-label-12 text-ds-gray-900 font-mono shrink-0">
            {service.tools.length} tools
          </span>
        )}

        {/* Status label */}
        <span className="text-label-12 text-ds-gray-700 shrink-0 font-mono w-16 text-right">
          {STATUS_LABEL[service.status]}
        </span>
      </button>

      {/* Expanded detail */}
      {expanded && (
        <div className="px-3 pb-3 pt-1 ml-9 space-y-2 border-t border-ds-gray-alpha-200">
          {/* Base URL */}
          <div className="flex items-center gap-2">
            <span className="text-label-12 text-ds-gray-700 w-20">URL</span>
            <span className="text-copy-13 text-ds-gray-900 font-mono">
              {service.url}
            </span>
          </div>

          {/* Last checked (from service.last_checked or local lastChecked prop) */}
          <div className="flex items-center gap-2">
            <span className="text-label-12 text-ds-gray-700 w-20">Last check</span>
            <span className="text-copy-13 text-ds-gray-900 font-mono">
              {service.last_checked
                ? formatLastChecked(service.last_checked)
                : lastChecked
                  ? formatRelativeTime(lastChecked)
                  : "never"}
            </span>
          </div>

          {/* Uptime */}
          {service.uptime_secs != null && (
            <div className="flex items-center gap-2">
              <span className="text-label-12 text-ds-gray-700 w-20">Uptime</span>
              <span className="text-copy-13 text-ds-gray-900 font-mono">
                {formatUptime(service.uptime_secs)}
              </span>
            </div>
          )}

          {/* Tool list */}
          {service.tools.length > 0 && (
            <div className="flex items-start gap-2">
              <span className="text-label-12 text-ds-gray-700 w-20 pt-0.5">
                Tools
              </span>
              <div className="flex flex-wrap gap-1">
                {service.tools.map((tool) => (
                  <span
                    key={tool}
                    className="px-1.5 py-0.5 rounded bg-ds-gray-alpha-200 text-label-12 text-ds-gray-900 font-mono"
                  >
                    {tool}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
