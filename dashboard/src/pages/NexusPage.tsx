import { useEffect, useState } from "react";
import { AlertCircle, RefreshCw, Layers } from "lucide-react";
import ActiveSession, {
  type ActiveSessionData,
} from "@/components/ActiveSession";
import ServerHealth, { type HealthMetrics } from "@/components/ServerHealth";
import type {
  SessionsGetResponse,
  ServerHealthGetResponse,
} from "@/types/api";

// Shape returned by the Nexus-backed /api/sessions endpoint
type NexusSessionRaw = SessionsGetResponse["sessions"][number];

function mapStatus(raw: string): ActiveSessionData["status"] {
  if (raw === "active") return "active";
  if (raw === "idle") return "idle";
  return "completed";
}

function mapNexusSession(s: NexusSessionRaw): ActiveSessionData {
  return {
    id: s.id,
    service: s.agent_name,
    status: mapStatus(s.status),
    messages: 0,
    tools_executed: 0,
    started_at: s.started_at ?? new Date().toISOString(),
    user: s.project ?? undefined,
    progress: s.progress?.progress_pct,
    current_task: s.progress?.phase_label,
  };
}

export default function NexusPage() {
  const [sessions, setSessions] = useState<ActiveSessionData[]>([]);
  const [health, setHealth] = useState<HealthMetrics | null>(null);
  const [loadingSessions, setLoadingSessions] = useState(true);
  const [loadingHealth, setLoadingHealth] = useState(true);
  const [sessionError, setSessionError] = useState<string | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  const fetchSessions = async () => {
    setLoadingSessions(true);
    setSessionError(null);
    try {
      const res = await fetch("/api/sessions");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as SessionsGetResponse;
      if (data.sessions && Array.isArray(data.sessions)) {
        // Check whether sessions look like Nexus sessions (have agent_name) or channel proxies
        const first = data.sessions[0] as NexusSessionRaw | undefined;
        if (first && "agent_name" in first) {
          setSessions(data.sessions.map(mapNexusSession));
        } else {
          setSessions([]);
        }
      } else {
        setSessions([]);
      }
    } catch (err) {
      setSessionError(
        err instanceof Error ? err.message : "Failed to load sessions"
      );
    } finally {
      setLoadingSessions(false);
    }
  };

  const fetchHealth = async () => {
    setLoadingHealth(true);
    setHealthError(null);
    try {
      const res = await fetch("/api/server-health");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as ServerHealthGetResponse;

      // Map backend status strings to the HealthMetrics union
      const mapBackendStatus = (
        s: ServerHealthGetResponse["status"],
      ): HealthMetrics["status"] => {
        if (s === "healthy") return "ok";
        if (s === "critical") return "down";
        return s; // "degraded" maps directly
      };

      if (data.latest) {
        setHealth({
          cpu_percent: data.latest.cpu_percent ?? 0,
          memory_used_mb: data.latest.memory_used_mb ?? 0,
          memory_total_mb: data.latest.memory_total_mb ?? 0,
          uptime_seconds: data.latest.uptime_seconds ?? 0,
          status: mapBackendStatus(data.status),
        });
      } else {
        setHealth(null);
      }
    } catch (err) {
      setHealthError(
        err instanceof Error ? err.message : "Failed to load health metrics"
      );
    } finally {
      setLoadingHealth(false);
    }
  };

  const refresh = () => {
    void fetchSessions();
    void fetchHealth();
  };

  useEffect(() => {
    void fetchSessions();
    void fetchHealth();
    const interval = setInterval(refresh, 10000);
    return () => clearInterval(interval);
  }, []);

  const active = sessions.filter((s) => s.status === "active");
  const idle = sessions.filter((s) => s.status === "idle");
  const completed = sessions.filter((s) => s.status === "completed");

  return (
    <div className="p-8 space-y-6 max-w-6xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">Nexus</h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            Active sessions and daemon health
          </p>
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={loadingSessions && loadingHealth}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw
            size={14}
            className={
              loadingSessions || loadingHealth ? "animate-spin" : ""
            }
          />
          Refresh
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
        {/* Sessions column — 3/5 */}
        <div className="lg:col-span-3 space-y-4">
          <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide">
            Sessions
          </h2>

          {sessionError && (
            <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
              <AlertCircle size={16} />
              <span className="text-sm">{sessionError}</span>
            </div>
          )}

          {loadingSessions ? (
            <div className="space-y-2">
              {Array.from({ length: 4 }).map((_, i) => (
                <div
                  key={i}
                  className="h-24 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
                />
              ))}
            </div>
          ) : sessions.length === 0 ? (
            <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted">
              <Layers size={36} />
              <p className="text-sm">No sessions active</p>
            </div>
          ) : (
            <div className="space-y-6">
              {active.length > 0 && (
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
                    <span className="text-xs text-cosmic-muted uppercase tracking-wide font-medium">
                      Active ({active.length})
                    </span>
                  </div>
                  <div className="space-y-2">
                    {active.map((s) => (
                      <ActiveSession key={s.id} session={s} />
                    ))}
                  </div>
                </div>
              )}

              {idle.length > 0 && (
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <div className="w-2 h-2 rounded-full bg-amber-500" />
                    <span className="text-xs text-cosmic-muted uppercase tracking-wide font-medium">
                      Idle ({idle.length})
                    </span>
                  </div>
                  <div className="space-y-2">
                    {idle.map((s) => (
                      <ActiveSession key={s.id} session={s} />
                    ))}
                  </div>
                </div>
              )}

              {completed.length > 0 && (
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <div className="w-2 h-2 rounded-full bg-cosmic-muted" />
                    <span className="text-xs text-cosmic-muted uppercase tracking-wide font-medium">
                      Completed ({completed.length})
                    </span>
                  </div>
                  <div className="space-y-2">
                    {completed.slice(0, 5).map((s) => (
                      <ActiveSession key={s.id} session={s} />
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Health column — 2/5 */}
        <div className="lg:col-span-2 space-y-4">
          <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide">
            Daemon Health
          </h2>

          <ServerHealth
            metrics={health}
            loading={loadingHealth}
            error={healthError}
          />

          {/* Quick stats */}
          <div className="grid grid-cols-2 gap-3">
            {[
              {
                label: "Active",
                value: active.length,
                color: "text-emerald-400",
              },
              {
                label: "Idle",
                value: idle.length,
                color: "text-amber-400",
              },
              {
                label: "Total",
                value: sessions.length,
                color: "text-cosmic-text",
              },
              {
                label: "Completed",
                value: completed.length,
                color: "text-cosmic-muted",
              },
            ].map(({ label, value, color }) => (
              <div
                key={label}
                className="p-3 rounded-lg bg-cosmic-surface border border-cosmic-border text-center"
              >
                <p className={`text-xl font-mono font-semibold ${color}`}>
                  {value}
                </p>
                <p className="text-xs text-cosmic-muted mt-0.5">{label}</p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
