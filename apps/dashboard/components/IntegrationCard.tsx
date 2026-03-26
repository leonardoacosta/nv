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

const ICON_COLORS: Record<string, string> = {
  Telegram: "text-[#229ED9]",
  Discord: "text-[#5865F2]",
  Slack: "text-[#E01E5A]",
  GitHub: "text-ds-gray-1000",
  Notion: "text-ds-gray-1000",
  Linear: "text-[#5E6AD2]",
  Stripe: "text-[#6772E5]",
  OpenAI: "text-emerald-400",
  Anthropic: "text-red-700",
};

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
  const iconColor = ICON_COLORS[integration.name] ?? "text-ds-gray-1000";

  return (
    <div className="surface-card flex items-center gap-4 p-4 group">
      {/* Icon placeholder */}
      <div
        className={`flex items-center justify-center w-10 h-10 rounded-lg bg-ds-bg-100 shrink-0 ${iconColor}`}
      >
        <span className="text-sm font-bold font-mono">
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

      {/* Status */}
      <div
        className={`flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium ${status.bg} ${status.color} shrink-0`}
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
