"use client";

import { Suspense, useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import {
  Terminal,
  CheckCircle,
  XCircle,
  Clock,
  AlertCircle,
  RefreshCw,
  Key,
  DollarSign,
  Hash,
  TrendingUp,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import StatCard from "@/components/layout/StatCard";
import ErrorBanner from "@/components/layout/ErrorBanner";
import SectionHeader from "@/components/layout/SectionHeader";
import ColdStartsPanel from "@/components/ColdStartsPanel";
import type { StatsGetResponse } from "@/types/api";
import { useQuery } from "@tanstack/react-query";
import { useTRPC } from "@/lib/trpc/react";

type UsageTab = "cost" | "performance";

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

function SuccessBar({ rate }: { rate: number }) {
  const pct = Math.min(100, Math.max(0, rate * 100));
  const barClass =
    pct >= 95
      ? "bg-green-700"
      : pct >= 80
        ? "bg-amber-700"
        : "bg-red-700";
  return (
    <div className="flex items-center gap-2">
      <div className="h-1.5 w-20 rounded-full bg-ds-bg-100 overflow-hidden">
        <div
          className={`h-full rounded-full ${barClass}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-label-13-mono text-ds-gray-900">
        {pct.toFixed(0)}%
      </span>
    </div>
  );
}

export default function UsagePageWrapper() {
  return (
    <Suspense>
      <UsagePage />
    </Suspense>
  );
}

function UsagePage() {
  const trpc = useTRPC();
  const searchParams = useSearchParams();
  const initialTab = searchParams.get("tab") === "performance" ? "performance" : "cost";
  const [activeTab, setActiveTab] = useState<UsageTab>(initialTab);
  const statsQuery = useQuery(trpc.system.stats.queryOptions());
  const statsRaw = statsQuery.data as StatsGetResponse | undefined;

  const tools: ToolUsage[] = (statsRaw?.tool_usage?.per_tool ?? [])
    .map((t) => ({
      name: t.name,
      count: t.count,
      avg_duration_ms: t.avg_duration_ms ?? 0,
      success_rate: t.count > 0 ? t.success_count / t.count : 0,
    }))
    .sort((a, b) => b.count - a.count);

  const data: UsageData | null = statsRaw
    ? {
        cost_today_usd: 0,
        cost_week_usd: 0,
        cost_month_usd: 0,
        tokens_today: 0,
        tokens_week: 0,
        tools,
        credentials: [],
      }
    : null;
  const loading = statsQuery.isLoading;
  const error = statsQuery.error?.message ?? null;

  const fetchUsage = () => void statsQuery.refetch();

  const headerAction = (
    <button
      type="button"
      onClick={() => void fetchUsage()}
      disabled={loading}
      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
    >
      <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
      Refresh
    </button>
  );

  return (
    <PageShell
      title="Usage"
      subtitle="Costs, token consumption, and tool metrics"
      action={activeTab === "cost" ? headerAction : undefined}
    >
      {/* Tab switcher */}
      <div className="flex items-center gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400 w-fit mb-3">
        {(["cost", "performance"] as UsageTab[]).map((tab) => (
          <button
            key={tab}
            type="button"
            onClick={() => setActiveTab(tab)}
            className={[
              "px-4 py-1.5 rounded-md text-label-14 transition-colors capitalize",
              activeTab === tab
                ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                : "text-ds-gray-900 hover:text-ds-gray-1000",
            ].join(" ")}
          >
            {tab}
          </button>
        ))}
      </div>

      {activeTab === "performance" ? (
        <ColdStartsPanel />
      ) : (
      <div className="space-y-4 animate-fade-in-up">
        {error && (
          <ErrorBanner
            message="Failed to load usage data"
            detail={error}
            onRetry={() => void fetchUsage()}
          />
        )}

        {/* Cost summary — StatCard grid */}
        <div>
          <div className="mb-2"><SectionHeader label="Cost Summary" /></div>
          {loading ? (
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              {Array.from({ length: 3 }).map((_, i) => (
                <div
                  key={i}
                  className="h-32 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400"
                />
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <StatCard
                icon={<DollarSign size={20} />}
                label="Cost Today"
                value={`$${(data?.cost_today_usd ?? 0).toFixed(4)}`}
                variant="default"
              />
              <StatCard
                icon={<DollarSign size={20} />}
                label="Cost This Week"
                value={`$${(data?.cost_week_usd ?? 0).toFixed(4)}`}
                variant="default"
              />
              <StatCard
                icon={<DollarSign size={20} />}
                label="Cost This Month"
                value={`$${(data?.cost_month_usd ?? 0).toFixed(4)}`}
                variant="default"
              />
            </div>
          )}
        </div>

        {/* Token usage — StatCard grid */}
        <div>
          <div className="mb-2"><SectionHeader label="Token Usage" /></div>
          {loading ? (
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              {Array.from({ length: 2 }).map((_, i) => (
                <div
                  key={i}
                  className="h-32 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400"
                />
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <StatCard
                icon={<Hash size={20} />}
                label="Tokens Today"
                value={(data?.tokens_today ?? 0).toLocaleString()}
                sublabel="input + output"
                variant="default"
              />
              <StatCard
                icon={<TrendingUp size={20} />}
                label="Tokens This Week"
                value={(data?.tokens_week ?? 0).toLocaleString()}
                sublabel="input + output"
                variant="default"
              />
            </div>
          )}
        </div>

        {/* Tool usage table */}
        <div>
          <div className="mb-2"><SectionHeader label="Tool Usage" /></div>
          {loading ? (
            <div className="space-y-1">
              {Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="h-10 animate-pulse rounded bg-ds-gray-100 border border-ds-gray-alpha-400"
                />
              ))}
            </div>
          ) : !data?.tools.length ? (
            <p className="text-copy-13 text-ds-gray-900 py-3">No tool usage recorded</p>
          ) : (
            <div className="surface-card overflow-hidden">
              <table className="w-full text-copy-13">
                <thead>
                  <tr
                    style={{ borderBottom: "1px solid var(--ds-gray-alpha-200)" }}
                    className="bg-ds-gray-alpha-100"
                  >
                    <th className="text-left px-4 py-2.5 text-label-12 text-ds-gray-700 font-medium">
                      Tool
                    </th>
                    <th className="text-right px-4 py-2.5 text-label-12 text-ds-gray-700 font-medium">
                      Count
                    </th>
                    <th className="text-right px-4 py-2.5 text-label-12 text-ds-gray-700 font-medium">
                      Avg Duration
                    </th>
                    <th className="px-4 py-2.5 text-label-12 text-ds-gray-700 font-medium">
                      Success
                    </th>
                  </tr>
                </thead>
                <tbody
                  style={{
                    borderTop: "1px solid var(--ds-gray-alpha-200)",
                  }}
                  className="divide-y divide-ds-gray-alpha-200"
                >
                  {data.tools.map((tool) => (
                    <tr
                      key={tool.name}
                      className="hover:bg-ds-gray-alpha-100 transition-colors"
                    >
                      <td className="px-4 py-2.5">
                        <div className="flex items-center gap-2">
                          <Terminal
                            size={13}
                            className="text-ds-gray-700 shrink-0"
                          />
                          <span className="text-label-13-mono text-ds-gray-1000">
                            {tool.name}
                          </span>
                        </div>
                      </td>
                      <td className="px-4 py-2.5 text-right text-label-13-mono text-ds-gray-1000">
                        {tool.count.toLocaleString()}
                      </td>
                      <td className="px-4 py-2.5 text-right">
                        <div className="flex items-center justify-end gap-1 text-label-13-mono text-ds-gray-900">
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
          <div className="mb-2"><SectionHeader label="Credentials" /></div>
          {loading ? (
            <div className="space-y-2">
              {Array.from({ length: 3 }).map((_, i) => (
                <div
                  key={i}
                  className="h-14 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400"
                />
              ))}
            </div>
          ) : !data?.credentials.length ? (
            <p className="text-copy-13 text-ds-gray-900 py-3">No credential data</p>
          ) : (
            <div className="space-y-2">
              {data.credentials.map((cred) => (
                <div
                  key={cred.name}
                  className="surface-card flex items-center gap-4 p-4"
                >
                  <Key size={16} className="text-ds-gray-700 shrink-0" />
                  <div className="flex-1 min-w-0">
                    <p className="text-label-14 text-ds-gray-1000">
                      {cred.name}
                    </p>
                    {cred.expires_at && (
                      <p
                        className="text-label-13-mono text-ds-gray-900 mt-0.5"
                        suppressHydrationWarning
                      >
                        Expires{" "}
                        {new Date(cred.expires_at).toLocaleDateString()}
                      </p>
                    )}
                  </div>
                  {cred.tokens_used !== undefined &&
                    cred.tokens_limit !== undefined && (
                      <div className="text-label-13-mono text-ds-gray-900">
                        {cred.tokens_used.toLocaleString()} /{" "}
                        {cred.tokens_limit.toLocaleString()}
                      </div>
                    )}
                  <div
                    className={`flex items-center gap-1.5 text-label-13 font-medium ${
                      cred.status === "valid"
                        ? "text-green-700"
                        : cred.status === "expired"
                          ? "text-red-700"
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
      )}
    </PageShell>
  );
}
