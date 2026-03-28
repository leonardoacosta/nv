import { ArrowLeftRight, ArrowRight, ArrowLeft } from "lucide-react";
import type { ChannelStatus } from "@/types/api";

const DIRECTION_CONFIG = {
  bidirectional: {
    label: "Bidirectional",
    icon: ArrowLeftRight,
  },
  inbound: {
    label: "Inbound",
    icon: ArrowLeft,
  },
  outbound: {
    label: "Outbound",
    icon: ArrowRight,
  },
} as const;

const STATUS_DOT: Record<ChannelStatus["status"], string> = {
  connected: "bg-green-700",
  configured: "bg-ds-gray-700",
  disconnected: "bg-red-700",
  unconfigured: "bg-ds-gray-400",
  unknown: "bg-ds-gray-500",
};

function fmtCount(n: number): string {
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

interface ChannelRowProps {
  channel: ChannelStatus;
}

export default function ChannelRow({ channel }: ChannelRowProps) {
  const dir = DIRECTION_CONFIG[channel.direction];
  const DirIcon = dir.icon;

  return (
    <div className="flex items-center gap-3 px-3 py-2 min-h-9 rounded-md hover:bg-ds-gray-alpha-100 transition-colors">
      {/* Status dot — transition on background-color */}
      <span
        className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 transition-[background-color] duration-300 ease-in-out ${STATUS_DOT[channel.status]}`}
        aria-label={channel.status}
      />

      {/* Channel name */}
      <span className="text-label-14 text-ds-gray-1000 flex-1 truncate">
        {channel.name}
      </span>

      {/* Volume metrics (messages_24h and messages_per_hour) */}
      {channel.messages_24h != null && (
        <span className="text-label-12 text-ds-gray-900 font-mono shrink-0">
          {fmtCount(channel.messages_24h)} msgs/24h
        </span>
      )}
      {channel.messages_per_hour != null && (
        <span className="text-label-12 text-ds-gray-700 font-mono shrink-0">
          {channel.messages_per_hour.toFixed(1)}/hr
        </span>
      )}

      {/* Direction badge */}
      <span className="flex items-center gap-1 px-1.5 py-0.5 rounded bg-ds-gray-alpha-200 text-label-12 text-ds-gray-900 shrink-0">
        <DirIcon size={10} />
        {dir.label}
      </span>

      {/* Status label */}
      <span className="text-label-12 text-ds-gray-700 shrink-0 font-mono">
        {channel.status}
      </span>
    </div>
  );
}
