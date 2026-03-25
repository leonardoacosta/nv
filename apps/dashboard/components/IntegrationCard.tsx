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
    color: "text-emerald-400",
    bg: "bg-emerald-500/20",
  },
  disconnected: {
    label: "Disconnected",
    icon: XCircle,
    color: "text-cosmic-muted",
    bg: "bg-cosmic-muted/20",
  },
  error: {
    label: "Error",
    icon: AlertCircle,
    color: "text-[#EF4444]",
    bg: "bg-[#EF4444]/20",
  },
  pending: {
    label: "Pending",
    icon: AlertCircle,
    color: "text-[#F97316]",
    bg: "bg-[#F97316]/20",
  },
};

const ICON_COLORS: Record<string, string> = {
  Telegram: "text-[#229ED9]",
  Discord: "text-[#5865F2]",
  Slack: "text-[#E01E5A]",
  GitHub: "text-cosmic-text",
  Notion: "text-cosmic-text",
  Linear: "text-[#5E6AD2]",
  Stripe: "text-[#6772E5]",
  OpenAI: "text-emerald-400",
  Anthropic: "text-cosmic-rose",
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
  const iconColor = ICON_COLORS[integration.name] ?? "text-cosmic-purple";

  return (
    <div className="flex items-center gap-4 p-4 rounded-cosmic border border-cosmic-border bg-cosmic-surface hover:border-cosmic-purple/40 transition-colors group">
      {/* Icon placeholder */}
      <div
        className={`flex items-center justify-center w-10 h-10 rounded-lg bg-cosmic-dark shrink-0 ${iconColor}`}
      >
        <span className="text-sm font-bold font-mono">
          {integration.name.slice(0, 2).toUpperCase()}
        </span>
      </div>

      {/* Info */}
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-cosmic-bright">
          {integration.name}
        </p>
        {integration.description && (
          <p className="text-xs text-cosmic-muted truncate mt-0.5">
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
          className="flex items-center justify-center w-8 h-8 rounded-lg text-cosmic-muted hover:text-cosmic-text hover:bg-cosmic-border transition-colors opacity-0 group-hover:opacity-100 shrink-0"
          aria-label={`Configure ${integration.name}`}
        >
          <Settings size={14} />
        </button>
      )}
    </div>
  );
}
