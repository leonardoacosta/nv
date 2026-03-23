import { MessageSquare, Terminal, Clock } from "lucide-react";

interface Session {
  id: string;
  service: string;
  status: "active" | "idle" | "completed";
  messages: number;
  tools_executed: number;
  started_at: string;
  user?: string;
}

const SERVICE_COLORS: Record<string, string> = {
  Telegram: "bg-[#229ED9]/20 text-[#229ED9]",
  Discord: "bg-[#5865F2]/20 text-[#5865F2]",
  Slack: "bg-[#4A154B]/20 text-[#E01E5A]",
  CLI: "bg-cosmic-purple/20 text-cosmic-purple",
  API: "bg-cosmic-rose/20 text-cosmic-rose",
  Web: "bg-emerald-500/20 text-emerald-400",
};

const STATUS_COLORS: Record<string, string> = {
  active: "bg-emerald-500/20 text-emerald-400",
  idle: "bg-amber-500/20 text-amber-400",
  completed: "bg-cosmic-muted/20 text-cosmic-muted",
};

interface SessionCardProps {
  session: Session;
}

export default function SessionCard({ session }: SessionCardProps) {
  const serviceColor =
    SERVICE_COLORS[session.service] ?? "bg-cosmic-muted/20 text-cosmic-muted";
  const statusColor = STATUS_COLORS[session.status] ?? STATUS_COLORS.idle;

  const elapsed = () => {
    const start = new Date(session.started_at).getTime();
    const now = Date.now();
    const diffMs = now - start;
    const diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 60) return `${diffMin}m`;
    const diffHr = Math.floor(diffMin / 60);
    return `${diffHr}h ${diffMin % 60}m`;
  };

  return (
    <div className="flex items-start gap-4 p-4 rounded-cosmic bg-cosmic-surface border border-cosmic-border hover:border-cosmic-purple/50 transition-colors">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span
            className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium font-mono ${serviceColor}`}
          >
            {session.service}
          </span>
          <span
            className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${statusColor}`}
          >
            {session.status}
          </span>
          {session.user && (
            <span className="text-xs text-cosmic-muted truncate">
              @{session.user}
            </span>
          )}
        </div>
        <p className="mt-1 text-xs text-cosmic-muted font-mono truncate">
          {session.id}
        </p>
      </div>
      <div className="flex items-center gap-4 shrink-0 text-cosmic-muted">
        <div className="flex items-center gap-1 text-xs font-mono">
          <MessageSquare size={12} />
          <span>{session.messages}</span>
        </div>
        <div className="flex items-center gap-1 text-xs font-mono">
          <Terminal size={12} />
          <span>{session.tools_executed}</span>
        </div>
        <div className="flex items-center gap-1 text-xs font-mono">
          <Clock size={12} />
          <span>{elapsed()}</span>
        </div>
      </div>
    </div>
  );
}

export type { Session };
