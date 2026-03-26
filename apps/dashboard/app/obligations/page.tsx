"use client";

import { useEffect, useState } from "react";
import { CheckSquare, RefreshCw, Clock } from "lucide-react";
import ObligationItem, {
  type Obligation,
} from "@/components/ObligationItem";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import type { DaemonObligation, ObligationsGetResponse } from "@/types/api";

type TabKey = "open" | "history";

/** Map a daemon Obligation to the component's Obligation interface. */
function mapDaemonObligation(o: DaemonObligation): Obligation {
  // Daemon status "done" maps to component "completed"
  const status: Obligation["status"] =
    o.status === "done"
      ? "completed"
      : o.status === "open" || o.status === "in_progress" || o.status === "dismissed"
        ? o.status
        : "open";
  return {
    id: o.id,
    title: o.detected_action,
    description: o.source_message ?? undefined,
    priority: Math.min(Math.max(o.priority, 0), 4) as Obligation["priority"],
    owner: o.owner,
    status,
    due_at: o.deadline ?? undefined,
    created_at: o.created_at,
    project_code: o.project_code ?? undefined,
    source_channel: o.source_channel,
  };
}

export default function ObligationsPage() {
  const [obligations, setObligations] = useState<Obligation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<TabKey>("open");

  const fetchObligations = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/obligations");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as ObligationsGetResponse;
      setObligations((data.obligations ?? []).map(mapDaemonObligation));
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load obligations"
      );
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchObligations();
  }, []);

  const open = obligations.filter(
    (o) => o.status === "open" || o.status === "in_progress"
  );
  const history = obligations.filter(
    (o) => o.status === "completed" || o.status === "dismissed"
  );

  const nova = open.filter((o) => o.owner === "nova");
  const leo = open.filter((o) => o.owner === "leo");
  const other = open.filter((o) => o.owner !== "nova" && o.owner !== "leo");

  const sortByPriority = (items: Obligation[]) =>
    [...items].sort((a, b) => a.priority - b.priority);

  return (
    <div className="p-8 space-y-6 max-w-4xl animate-fade-in-up">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-heading-24 text-ds-gray-1000">
            Obligations
          </h1>
          <p className="mt-1 text-copy-14 text-ds-gray-900">
            Active tasks and commitments
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchObligations()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 p-1 rounded-lg bg-ds-gray-100 border border-ds-gray-400 w-fit">
        {(["open", "history"] as TabKey[]).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={`flex items-center gap-2 px-4 py-1.5 rounded text-sm font-medium transition-colors ${
              tab === t
                ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                : "text-ds-gray-900 hover:text-ds-gray-1000"
            }`}
          >
            {t === "open" ? <CheckSquare size={14} /> : <Clock size={14} />}
            <span className="capitalize">
              {t === "open" ? "Active" : "History"}
            </span>
            <span className="text-xs font-mono opacity-70">
              {t === "open" ? open.length : history.length}
            </span>
          </button>
        ))}
      </div>

      {error && (
        <ErrorBanner
          message="Failed to load obligations"
          detail={error}
          onRetry={() => void fetchObligations()}
        />
      )}

      {loading ? (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-20 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
            />
          ))}
        </div>
      ) : tab === "open" ? (
        <div className="space-y-8">
          {/* Nova section */}
          <section>
            <div className="flex items-center gap-2 mb-3">
              <div className="w-6 h-6 rounded bg-ds-gray-700/30 flex items-center justify-center">
                <span className="text-xs font-bold font-mono text-ds-gray-1000">
                  N
                </span>
              </div>
              <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide">
                Nova
              </h2>
              <span className="text-xs font-mono text-ds-gray-900">
                {nova.length}
              </span>
            </div>
            {nova.length === 0 ? (
              <p className="text-copy-14 text-ds-gray-900 py-4 pl-2">
                No obligations assigned to Nova
              </p>
            ) : (
              <div className="space-y-2">
                {sortByPriority(nova).map((o, idx) => (
                  <div
                    key={o.id}
                    className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
                  >
                    <ObligationItem obligation={o} />
                  </div>
                ))}
              </div>
            )}
          </section>

          {/* Leo section */}
          <section>
            <div className="flex items-center gap-2 mb-3">
              <div className="w-6 h-6 rounded bg-red-700/30 flex items-center justify-center">
                <span className="text-xs font-bold font-mono text-red-700">
                  L
                </span>
              </div>
              <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide">
                Leo
              </h2>
              <span className="text-xs font-mono text-ds-gray-900">
                {leo.length}
              </span>
            </div>
            {leo.length === 0 ? (
              <p className="text-copy-14 text-ds-gray-900 py-4 pl-2">
                No obligations assigned to Leo
              </p>
            ) : (
              <div className="space-y-2">
                {sortByPriority(leo).map((o, idx) => (
                  <div
                    key={o.id}
                    className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
                  >
                    <ObligationItem obligation={o} />
                  </div>
                ))}
              </div>
            )}
          </section>

          {/* Other */}
          {other.length > 0 && (
            <section>
              <h2 className="text-sm font-semibold text-ds-gray-1000 uppercase tracking-wide mb-3">
                Other
              </h2>
              <div className="space-y-2">
                {sortByPriority(other).map((o) => (
                  <ObligationItem key={o.id} obligation={o} />
                ))}
              </div>
            </section>
          )}

          {open.length === 0 && (
            <EmptyState
              title="No active obligations"
              description="All clear. New obligations will appear here when detected."
              icon={<CheckSquare size={40} aria-hidden="true" />}
            />
          )}
        </div>
      ) : (
        /* History tab */
        <div className="space-y-2">
          {history.length === 0 ? (
            <EmptyState
              title="No history yet"
              description="Completed and dismissed obligations will appear here."
              icon={<Clock size={40} aria-hidden="true" />}
            />
          ) : (
            sortByPriority(history).map((o, idx) => (
              <div
                key={o.id}
                className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
              >
                <ObligationItem obligation={o} />
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}
