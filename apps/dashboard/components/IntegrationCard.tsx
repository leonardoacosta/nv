import { Settings, CheckCircle, XCircle, AlertCircle } from "lucide-react";

export type IntegrationStatus = "connected" | "disconnected" | "error" | "pending";

export interface Integration {
  id: string;
  name: string;
  description?: string;
  status: IntegrationStatus;
  category: "channels" | "tools" | "services";
  icon?: string;
  config?: Record<string, string | number | boolean>;
}

const STATUS_CONFIG: Record<
  IntegrationStatus,
  { label: string; icon: React.ElementType; color: string; bg: string }
> = {
  connected: {
    label: "Connected",
    icon: CheckCircle,
    color: "text-green-700",
    bg: "bg-green-700/10",
  },
  disconnected: {
    label: "Disconnected",
    icon: XCircle,
    color: "text-ds-gray-900",
    bg: "bg-ds-gray-alpha-200",
  },
  error: {
    label: "Error",
    icon: AlertCircle,
    color: "text-red-700",
    bg: "bg-red-700/10",
  },
  pending: {
    label: "Pending",
    icon: AlertCircle,
    color: "text-amber-700",
    bg: "bg-amber-700/10",
  },
};

// ---------------------------------------------------------------------------
// Deterministic hash-to-color for avatar backgrounds
// ---------------------------------------------------------------------------

/** 8 curated dark-theme-friendly colors for avatar backgrounds. */
const AVATAR_PALETTE = [
  "#2563eb", // blue
  "#7c3aed", // violet
  "#db2777", // pink
  "#059669", // emerald
  "#d97706", // amber
  "#dc2626", // red
  "#0891b2", // cyan
  "#4f46e5", // indigo
];

/** Simple djb2 hash producing a deterministic palette index from a service name. */
function hashToColor(name: string): string {
  let hash = 5381;
  for (let i = 0; i < name.length; i++) {
    hash = ((hash << 5) + hash + name.charCodeAt(i)) | 0;
  }
  const index = Math.abs(hash) % AVATAR_PALETTE.length;
  return AVATAR_PALETTE[index]!;
}

interface IntegrationCardProps {
  integration: Integration;
  onConfigure?: (integration: Integration) => void;
}

export default function IntegrationCard({
  integration,
  onConfigure,
}: IntegrationCardProps) {
  const status = STATUS_CONFIG[integration.status];
  const StatusIcon = status.icon;
  const isConnected = integration.status === "connected";
  const isDisconnected = integration.status === "disconnected";
  const avatarColor = hashToColor(integration.name);

  return (
    <div
      className={[
        "surface-card flex items-center gap-4 p-4 group transition-all",
        isDisconnected && "opacity-60",
        isConnected && "shadow-md",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {/* Avatar with hash-based background color */}
      <div
        className="flex items-center justify-center w-10 h-10 rounded-lg shrink-0"
        style={{ backgroundColor: `${avatarColor}20` }}
      >
        <span
          className="text-sm font-bold font-mono"
          style={{ color: avatarColor }}
        >
          {integration.name.slice(0, 2).toUpperCase()}
        </span>
      </div>

      {/* Info */}
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-ds-gray-1000">
          {integration.name}
        </p>
        {integration.description && (
          <p className="text-xs text-ds-gray-900 truncate mt-0.5">
            {integration.description}
          </p>
        )}
      </div>

      {/* Status badge — connected gets glow-pulse-green */}
      <div
        className={[
          "flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium shrink-0",
          status.bg,
          status.color,
          isConnected && "glow-pulse-green",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        <StatusIcon size={12} />
        <span>{status.label}</span>
      </div>

      {/* Configure button */}
      {onConfigure && (
        <button
          type="button"
          onClick={() => onConfigure(integration)}
          className="flex items-center justify-center w-8 h-8 rounded-lg text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-200 transition-colors opacity-0 group-hover:opacity-100 shrink-0"
          aria-label={`Configure ${integration.name}`}
        >
          <Settings size={14} />
        </button>
      )}
    </div>
  );
}
