"use client";

import { useEffect, useState } from "react";
import {
  Terminal,
  CheckCircle,
  XCircle,
  Clock,
  AlertCircle,
  RefreshCw,
  Key,
  TrendingUp,
} from "lucide-react";
import type { StatsGetResponse } from "@/types/api";

interface ToolUsage {
  name: string;
  count: number;
  avg_duration_ms: number;
  success_rate: number;
}

interface CredentialStatus {
  name: string;
  status: "valid" | "expired" | "missing";
  tokens_used?: number;
  tokens_limit?: number;
  expires_at?: string;
}

interface UsageData {
  cost_today_usd: number;
  cost_week_usd: number;
  cost_month_usd: number;
  tokens_today: number;
  tokens_week: number;
  tools: ToolUsage[];
  credentials: CredentialStatus[];
}

function CostCard({
  label,
  value,
  accent,
}: {
  label: string;
  value: number;
  accent?: boolean;
}) {
  return (
    <div
      className={`p-4 rounded-xl border ${accent ? "border-ds-gray-1000/40 bg-ds-gray-alpha-100" : "border-ds-gray-400 bg-ds-gray-100"}`}
    >
      <p className="text-xs text-ds-gray-900 uppercase tracking-wide">{label}</p>
      <p
        className={`text-2xl font-mono font-semibold mt-1 ${accent ? "text-ds-gray-1000" : "text-ds-gray-1000"}`}
      >
        ${value.toFixed(4)}
      </p>
    </div>
  );
}

function SuccessBar({ rate }: { rate: number }) {
  const pct = Math.min(100, Math.max(0, rate * 100));
  const color =
    pct >= 95
      ? "bg-emerald-500"
      : pct >= 80
        ? "bg-[#F97316]"
        : "bg-[#EF4444]";
  return (
    <div className="flex items-center gap-2">
      <div className="h-1.5 w-20 rounded-full bg-ds-bg-100 overflow-hidden">
        <div
          className={`h-full rounded-full ${color}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-xs font-mono text-ds-gray-900">
        {pct.toFixed(0)}%
      </span>
    </div>
  );
}

export default function UsagePage() {
  const [data, setData] = useState<UsageData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchUsage = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/stats");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const stats = (await res.json()) as StatsGetResponse;

      const tools: ToolUsage[] = (stats.tool_usage?.per_tool ?? [])
        .map((t) => ({
          name: t.name,
          count: t.count,
          avg_duration_ms: t.avg_duration_ms ?? 0,
          success_rate: t.count > 0 ? t.success_count / t.count : 0,
        }))
        .sort((a, b) => b.count - a.count);

      setData({
        cost_today_usd: 0,
        cost_week_usd: 0,
        cost_month_usd: 0,
        tokens_today: 0,
        tokens_week: 0,
        tools,
        credentials: [],
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load usage");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchUsage();
  }, []);

  return (
    <div className="p-8 space-y-8 max-w-5xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-ds-gray-1000">Usage</h1>
          <p className="mt-1 text-sm text-ds-gray-900">
            Costs, token consumption, and tool metrics
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchUsage()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {error && (
        <div className="flex items-center gap-3 p-4 rounded-xl bg-red-700/10 border border-red-700/30 text-red-700">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {/* Cost summary */}
      <div>
        <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide mb-3">
          Cost Summary
        </h2>
        {loading ? (
          <div className="grid grid-cols-3 gap-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-20 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
              />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <CostCard
              label="Today"
              value={data?.cost_today_usd ?? 0}
              accent
            />
            <CostCard label="This Week" value={data?.cost_week_usd ?? 0} />
            <CostCard label="This Month" value={data?.cost_month_usd ?? 0} />
          </div>
        )}
      </div>

      {/* Token usage */}
      <div>
        <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide mb-3">
          Token Usage
        </h2>
        <div className="grid grid-cols-2 gap-4">
          <div className="p-4 rounded-xl border border-ds-gray-400 bg-ds-gray-100">
            <div className="flex items-center gap-2 text-ds-gray-900 mb-1">
              <TrendingUp size={14} />
              <span className="text-xs uppercase tracking-wide">Today</span>
            </div>
            <p className="text-xl font-mono font-semibold text-ds-gray-1000">
              {loading ? "..." : (data?.tokens_today ?? 0).toLocaleString()}
            </p>
          </div>
          <div className="p-4 rounded-xl border border-ds-gray-400 bg-ds-gray-100">
            <div className="flex items-center gap-2 text-ds-gray-900 mb-1">
              <TrendingUp size={14} />
              <span className="text-xs uppercase tracking-wide">This Week</span>
            </div>
            <p className="text-xl font-mono font-semibold text-ds-gray-1000">
              {loading ? "..." : (data?.tokens_week ?? 0).toLocaleString()}
            </p>
          </div>
        </div>
      </div>

      {/* Tool usage table */}
      <div>
        <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide mb-3">
          Tool Usage
        </h2>
        {loading ? (
          <div className="space-y-1">
            {Array.from({ length: 6 }).map((_, i) => (
              <div
                key={i}
                className="h-10 animate-pulse rounded bg-ds-gray-100 border border-ds-gray-400"
              />
            ))}
          </div>
        ) : !data?.tools.length ? (
          <div className="flex flex-col items-center gap-3 py-12 text-ds-gray-900">
            <Terminal size={32} />
            <p className="text-sm">No tool usage recorded</p>
          </div>
        ) : (
          <div className="rounded-xl border border-ds-gray-400 overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-ds-gray-400 bg-ds-gray-100">
                  <th className="text-left px-4 py-2.5 text-xs text-ds-gray-900 uppercase tracking-wide font-medium">
                    Tool
                  </th>
                  <th className="text-right px-4 py-2.5 text-xs text-ds-gray-900 uppercase tracking-wide font-medium">
                    Count
                  </th>
                  <th className="text-right px-4 py-2.5 text-xs text-ds-gray-900 uppercase tracking-wide font-medium">
                    Avg Duration
                  </th>
                  <th className="px-4 py-2.5 text-xs text-ds-gray-900 uppercase tracking-wide font-medium">
                    Success
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-ds-gray-400">
                {data.tools.map((tool) => (
                  <tr
                    key={tool.name}
                    className="hover:bg-ds-gray-100/50 transition-colors"
                  >
                    <td className="px-4 py-2.5">
                      <div className="flex items-center gap-2">
                        <Terminal size={13} className="text-ds-gray-900" />
                        <span className="font-mono text-ds-gray-1000 text-xs">
                          {tool.name}
                        </span>
                      </div>
                    </td>
                    <td className="px-4 py-2.5 text-right font-mono text-xs text-ds-gray-1000">
                      {tool.count.toLocaleString()}
                    </td>
                    <td className="px-4 py-2.5 text-right">
                      <div className="flex items-center justify-end gap-1 text-xs text-ds-gray-900 font-mono">
                        <Clock size={11} />
                        <span>{tool.avg_duration_ms}ms</span>
                      </div>
                    </td>
                    <td className="px-4 py-2.5">
                      <SuccessBar rate={tool.success_rate} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Credentials */}
      <div>
        <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide mb-3">
          Credentials
        </h2>
        {loading ? (
          <div className="space-y-2">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-14 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
              />
            ))}
          </div>
        ) : !data?.credentials.length ? (
          <div className="flex flex-col items-center gap-3 py-10 text-ds-gray-900">
            <Key size={32} />
            <p className="text-sm">No credential data available</p>
          </div>
        ) : (
          <div className="space-y-2">
            {data.credentials.map((cred) => (
              <div
                key={cred.name}
                className="flex items-center gap-4 p-4 rounded-xl border border-ds-gray-400 bg-ds-gray-100"
              >
                <Key size={16} className="text-ds-gray-900 shrink-0" />
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-ds-gray-1000">
                    {cred.name}
                  </p>
                  {cred.expires_at && (
                    <p className="text-xs text-ds-gray-900 font-mono mt-0.5" suppressHydrationWarning>
                      Expires {new Date(cred.expires_at).toLocaleDateString()}
                    </p>
                  )}
                </div>
                {cred.tokens_used !== undefined &&
                  cred.tokens_limit !== undefined && (
                    <div className="text-xs font-mono text-ds-gray-900">
                      {cred.tokens_used.toLocaleString()} /{" "}
                      {cred.tokens_limit.toLocaleString()}
                    </div>
                  )}
                <div
                  className={`flex items-center gap-1.5 text-xs font-medium ${
                    cred.status === "valid"
                      ? "text-emerald-400"
                      : cred.status === "expired"
                        ? "text-[#EF4444]"
                        : "text-ds-gray-900"
                  }`}
                >
                  {cred.status === "valid" ? (
                    <CheckCircle size={13} />
                  ) : (
                    <XCircle size={13} />
                  )}
                  <span className="capitalize">{cred.status}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
