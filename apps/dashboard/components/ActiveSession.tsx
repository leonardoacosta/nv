import { MessageSquare, Terminal, Clock, User } from "lucide-react";

export interface ActiveSessionData {
  id: string;
  service: string;
  status: "active" | "idle" | "completed";
  messages: number;
  tools_executed: number;
  started_at: string;
  user?: string;
  progress?: number;
  current_task?: string;
}

const SERVICE_BADGE: Record<string, string> = {
  Telegram: "bg-[#229ED9]/20 text-[#229ED9]",
  Discord: "bg-[#5865F2]/20 text-[#5865F2]",
  Slack: "bg-[#4A154B]/20 text-[#E01E5A]",
  CLI: "bg-cosmic-purple/20 text-cosmic-purple",
  API: "bg-cosmic-rose/20 text-cosmic-rose",
  Web: "bg-emerald-500/20 text-emerald-400",
};

const STATUS_DOT: Record<string, string> = {
  active: "bg-emerald-500",
  idle: "bg-amber-500",
  completed: "bg-cosmic-muted",
};

interface ActiveSessionProps {
  session: ActiveSessionData;
}

function elapsed(startedAt: string): string {
  const diffMs = Date.now() - new Date(startedAt).getTime();
  const min = Math.floor(diffMs / 60000);
  if (min < 60) return `${min}m`;
  return `${Math.floor(min / 60)}h ${min % 60}m`;
}

export default function ActiveSession({ session }: ActiveSessionProps) {
  const badge =
    SERVICE_BADGE[session.service] ?? "bg-cosmic-muted/20 text-cosmic-muted";
  const dot = STATUS_DOT[session.status] ?? "bg-cosmic-muted";
  const progress = session.progress ?? 0;

  return (
    <div className="p-4 rounded-cosmic border border-cosmic-border bg-cosmic-surface hover:border-cosmic-purple/40 transition-colors space-y-3">
      {/* Header */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 flex-wrap">
          <div className={`w-2 h-2 rounded-full shrink-0 ${dot} ${session.status === "active" ? "animate-pulse" : ""}`} />
          <span
            className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium font-mono ${badge}`}
          >
            {session.service}
          </span>
          {session.user && (
            <div className="flex items-center gap-1 text-xs text-cosmic-muted">
              <User size={11} />
              <span>@{session.user}</span>
            </div>
          )}
        </div>
        <div className="flex items-center gap-1 text-xs text-cosmic-muted font-mono shrink-0">
          <Clock size={11} />
          <span suppressHydrationWarning>{elapsed(session.started_at)}</span>
        </div>
      </div>

      {/* Current task */}
      {session.current_task && (
        <p className="text-xs text-cosmic-muted truncate pl-4">
          {session.current_task}
        </p>
      )}

      {/* Progress bar */}
      {session.status === "active" && (
        <div className="space-y-1">
          <div className="h-1 rounded-full bg-cosmic-dark overflow-hidden">
            <div
              className="h-full rounded-full bg-cosmic-purple transition-all duration-500"
              style={{ width: `${Math.min(100, Math.max(0, progress))}%` }}
            />
          </div>
          <p className="text-xs text-cosmic-muted font-mono text-right">
            {progress}%
          </p>
        </div>
      )}

      {/* Stats */}
      <div className="flex items-center gap-4 pt-1 border-t border-cosmic-border">
        <div className="flex items-center gap-1.5 text-xs text-cosmic-muted font-mono">
          <MessageSquare size={12} />
          <span>{session.messages} msgs</span>
        </div>
        <div className="flex items-center gap-1.5 text-xs text-cosmic-muted font-mono">
          <Terminal size={12} />
          <span>{session.tools_executed} tools</span>
        </div>
        <div className="ml-auto">
          <p className="text-xs text-cosmic-muted font-mono truncate max-w-32">
            {session.id.slice(0, 8)}...
          </p>
        </div>
      </div>
    </div>
  );
}
