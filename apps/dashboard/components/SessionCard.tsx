import { MessageSquare, Terminal, Clock } from "lucide-react";
import { getPlatformColor } from "@/lib/brand-colors";

interface Session {
  id: string;
  service: string;
  status: "active" | "idle" | "completed";
  messages: number;
  tools_executed: number;
  started_at: string;
  user?: string;
}

const STATUS_COLORS: Record<string, string> = {
  active: "bg-green-700/20 text-green-700",
  idle: "bg-amber-700/20 text-amber-700",
  completed: "bg-ds-gray-alpha-200 text-ds-gray-900",
};

interface SessionCardProps {
  session: Session;
}

export default function SessionCard({ session }: SessionCardProps) {
  const brand = getPlatformColor(session.service);
  const serviceColor = `${brand.bg} ${brand.text}`;
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
    <div className="flex items-start gap-4 p-4 rounded-xl bg-ds-gray-100 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span
            className={`inline-flex items-center px-2 py-0.5 rounded text-label-12 font-medium font-mono ${serviceColor}`}
          >
            {session.service}
          </span>
          <span
            className={`inline-flex items-center px-2 py-0.5 rounded text-label-12 font-medium ${statusColor}`}
          >
            {session.status}
          </span>
          {session.user && (
            <span className="text-copy-13 text-ds-gray-900 truncate">
              @{session.user}
            </span>
          )}
        </div>
        <p className="mt-1 text-copy-13 text-ds-gray-900 font-mono truncate">
          {session.id}
        </p>
      </div>
      <div className="flex items-center gap-4 shrink-0 text-ds-gray-900">
        <div className="flex items-center gap-1 text-copy-13 font-mono">
          <MessageSquare size={12} />
          <span>{session.messages}</span>
        </div>
        <div className="flex items-center gap-1 text-copy-13 font-mono">
          <Terminal size={12} />
          <span>{session.tools_executed}</span>
        </div>
        <div className="flex items-center gap-1 text-copy-13 font-mono">
          <Clock size={12} />
          <span suppressHydrationWarning>{elapsed()}</span>
        </div>
      </div>
    </div>
  );
}

export type { Session };
