"use client";

import { useEffect, useState } from "react";
import {
  CheckSquare,
  Layers,
  MessageSquare,
  Terminal,
  DollarSign,
  TrendingUp,
  AlertCircle,
  RefreshCw,
  Cpu,
  MemoryStick,
  Heart,
} from "lucide-react";
import SessionCard, { type Session } from "@/components/SessionCard";
import type {
  ProjectsGetResponse,
  SessionsGetResponse,
  ServerHealthGetResponse,
} from "@/types/api";

interface SummaryData {
  obligations_count: number;
  active_sessions: number;
  projects_count: number;
  messages_today: number;
  tools_today: number;
  cost_today_usd: number;
}

interface ApiObligation {
  id: string;
  title: string;
}

interface HealthSummary {
  status: "ok" | "degraded" | "critical" | "down";
  cpu_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
}

function StatCard({
  icon: Icon,
  label,
  value,
  accent,
}: {
  icon: React.ElementType;
  label: string;
  value: string | number;
  accent?: string;
}) {
  return (
    <div className="flex items-center gap-4 p-5 rounded-cosmic bg-cosmic-surface border border-cosmic-border">
      <div
        className={`flex items-center justify-center w-10 h-10 rounded-lg ${accent ?? "bg-cosmic-purple/20"}`}
      >
        <Icon
          size={20}
          className={accent ? "text-cosmic-rose" : "text-cosmic-purple"}
        />
      </div>
      <div>
        <p className="text-xs text-cosmic-muted uppercase tracking-wide">
          {label}
        </p>
        <p className="text-xl font-semibold font-mono text-cosmic-bright">
          {value}
        </p>
      </div>
    </div>
  );
}

function HealthStatCard({
  icon: Icon,
  label,
  value,
  statusClass,
}: {
  icon: React.ElementType;
  label: string;
  value: string | number;
  statusClass: string;
}) {
  return (
    <div className="flex items-center gap-4 p-5 rounded-cosmic bg-cosmic-surface border border-cosmic-border">
      <div className={`flex items-center justify-center w-10 h-10 rounded-lg ${statusClass}`}>
        <Icon size={20} className={statusClass.includes("emerald") ? "text-emerald-400" : statusClass.includes("orange") ? "text-[#F97316]" : "text-[#EF4444]"} />
      </div>
      <div>
        <p className="text-xs text-cosmic-muted uppercase tracking-wide">
          {label}
        </p>
        <p className={`text-xl font-semibold font-mono ${statusClass.includes("emerald") ? "text-emerald-400" : statusClass.includes("orange") ? "text-[#F97316]" : "text-[#EF4444]"}`}>
          {value}
        </p>
      </div>
    </div>
  );
}

export default function DashboardPage() {
  const [summary, setSummary] = useState<SummaryData | null>(null);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [healthSummary, setHealthSummary] = useState<HealthSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async () => {
    setLoading(true);
    setError(null);
    try {
      const [oblRes, projRes, sessRes, healthRes] = await Promise.allSettled([
        fetch("/api/obligations"),
        fetch("/api/projects"),
        fetch("/api/sessions"),
        fetch("/api/server-health"),
      ]);

      const obligationsCount =
        oblRes.status === "fulfilled" && oblRes.value.ok
          ? ((await oblRes.value.json()) as ApiObligation[]).length
          : 0;

      const projectsCount =
        projRes.status === "fulfilled" && projRes.value.ok
          ? ((await projRes.value.json()) as ProjectsGetResponse).projects.length
          : 0;

      const sessData: Session[] =
        sessRes.status === "fulfilled" && sessRes.value.ok
          ? ((await sessRes.value.json()) as SessionsGetResponse).sessions as unknown as Session[]
          : [];

      // Parse health response
      if (healthRes.status === "fulfilled" && healthRes.value.ok) {
        const hData = (await healthRes.value.json()) as ServerHealthGetResponse;
        if (hData.latest) {
          const mapStatus = (s: ServerHealthGetResponse["status"]): HealthSummary["status"] => {
            if (s === "healthy") return "ok";
            if (s === "critical") return "critical";
            return s;
          };
          setHealthSummary({
            status: mapStatus(hData.status),
            cpu_percent: hData.latest.cpu_percent ?? 0,
            memory_used_mb: hData.latest.memory_used_mb ?? 0,
            memory_total_mb: hData.latest.memory_total_mb ?? 0,
          });
        }
      }

      setSessions(sessData);
      setSummary({
        obligations_count: obligationsCount,
        active_sessions: sessData.filter((s) => s.status === "active").length,
        projects_count: projectsCount,
        messages_today: sessData.reduce((acc, s) => acc + (s.messages ?? 0), 0),
        tools_today: sessData.reduce(
          (acc, s) => acc + (s.tools_executed ?? 0),
          0
        ),
        cost_today_usd: 0,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load data");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchData();
  }, []);

  // Derive status class for health cards
  const healthStatusClass = (status: HealthSummary["status"] | undefined) => {
    if (!status || status === "ok") return "bg-emerald-500/20";
    if (status === "degraded") return "bg-[#F97316]/20";
    return "bg-[#EF4444]/20";
  };

  const healthStatus = healthSummary?.status;

  return (
    <div className="p-8 space-y-8 max-w-6xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">
            Dashboard
          </h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            Nova activity overview
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchData()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {error && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {/* Summary cards */}
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-4">
        {loading ? (
          Array.from({ length: 9 }).map((_, i) => (
            <div
              key={i}
              className="h-20 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
            />
          ))
        ) : (
          <>
            <StatCard
              icon={CheckSquare}
              label="Obligations"
              value={summary?.obligations_count ?? 0}
            />
            <StatCard
              icon={Layers}
              label="Active Sessions"
              value={summary?.active_sessions ?? 0}
            />
            <StatCard
              icon={TrendingUp}
              label="Projects"
              value={summary?.projects_count ?? 0}
            />
            <StatCard
              icon={MessageSquare}
              label="Messages Today"
              value={summary?.messages_today ?? 0}
              accent="bg-cosmic-rose/20"
            />
            <StatCard
              icon={Terminal}
              label="Tools Today"
              value={summary?.tools_today ?? 0}
              accent="bg-cosmic-rose/20"
            />
            <StatCard
              icon={DollarSign}
              label="Cost Today"
              value={`$${(summary?.cost_today_usd ?? 0).toFixed(4)}`}
              accent="bg-cosmic-rose/20"
            />
            {/* Health cards */}
            <HealthStatCard
              icon={Heart}
              label="Server Health"
              value={healthSummary ? healthSummary.status : "—"}
              statusClass={healthStatusClass(healthStatus)}
            />
            <HealthStatCard
              icon={Cpu}
              label="CPU"
              value={healthSummary ? `${healthSummary.cpu_percent.toFixed(1)}%` : "—"}
              statusClass={
                healthSummary && healthSummary.cpu_percent >= 90
                  ? "bg-[#EF4444]/20"
                  : healthSummary && healthSummary.cpu_percent >= 70
                    ? "bg-[#F97316]/20"
                    : "bg-emerald-500/20"
              }
            />
            <HealthStatCard
              icon={MemoryStick}
              label="Memory"
              value={
                healthSummary && healthSummary.memory_total_mb > 0
                  ? `${((healthSummary.memory_used_mb / healthSummary.memory_total_mb) * 100).toFixed(0)}%`
                  : "—"
              }
              statusClass={
                healthSummary && healthSummary.memory_total_mb > 0 &&
                (healthSummary.memory_used_mb / healthSummary.memory_total_mb) >= 0.9
                  ? "bg-[#EF4444]/20"
                  : healthSummary && healthSummary.memory_total_mb > 0 &&
                    (healthSummary.memory_used_mb / healthSummary.memory_total_mb) >= 0.8
                    ? "bg-[#F97316]/20"
                    : "bg-emerald-500/20"
              }
            />
          </>
        )}
      </div>

      {/* Recent sessions */}
      <div>
        <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide mb-3">
          Recent Sessions
        </h2>
        {loading ? (
          <div className="space-y-2">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-16 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
              />
            ))}
          </div>
        ) : sessions.length === 0 ? (
          <div className="flex flex-col items-center gap-3 py-12 text-cosmic-muted">
            <Layers size={32} />
            <p className="text-sm">No sessions active</p>
          </div>
        ) : (
          <div className="space-y-2">
            {sessions.slice(0, 8).map((session) => (
              <SessionCard key={session.id} session={session} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
