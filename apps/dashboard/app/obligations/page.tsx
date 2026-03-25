"use client";

import { useEffect, useState } from "react";
import { CheckSquare, AlertCircle, RefreshCw, Clock } from "lucide-react";
import ObligationItem, {
  type Obligation,
} from "@/components/ObligationItem";

type TabKey = "open" | "history";

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
      const data = (await res.json()) as Obligation[];
      setObligations(data);
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
    <div className="p-8 space-y-6 max-w-4xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">
            Obligations
          </h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            Active tasks and commitments
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchObligations()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 p-1 rounded-lg bg-cosmic-surface border border-cosmic-border w-fit">
        {(["open", "history"] as TabKey[]).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={`flex items-center gap-2 px-4 py-1.5 rounded text-sm font-medium transition-colors ${
              tab === t
                ? "bg-cosmic-purple/20 text-cosmic-bright"
                : "text-cosmic-muted hover:text-cosmic-text"
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
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {loading ? (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-20 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
            />
          ))}
        </div>
      ) : tab === "open" ? (
        <div className="space-y-8">
          {/* Nova section */}
          <section>
            <div className="flex items-center gap-2 mb-3">
              <div className="w-6 h-6 rounded bg-cosmic-purple/30 flex items-center justify-center">
                <span className="text-xs font-bold font-mono text-cosmic-purple">
                  N
                </span>
              </div>
              <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide">
                Nova
              </h2>
              <span className="text-xs font-mono text-cosmic-muted">
                {nova.length}
              </span>
            </div>
            {nova.length === 0 ? (
              <p className="text-sm text-cosmic-muted py-4 pl-2">
                No obligations assigned to Nova
              </p>
            ) : (
              <div className="space-y-2">
                {sortByPriority(nova).map((o) => (
                  <ObligationItem key={o.id} obligation={o} />
                ))}
              </div>
            )}
          </section>

          {/* Leo section */}
          <section>
            <div className="flex items-center gap-2 mb-3">
              <div className="w-6 h-6 rounded bg-cosmic-rose/30 flex items-center justify-center">
                <span className="text-xs font-bold font-mono text-cosmic-rose">
                  L
                </span>
              </div>
              <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide">
                Leo
              </h2>
              <span className="text-xs font-mono text-cosmic-muted">
                {leo.length}
              </span>
            </div>
            {leo.length === 0 ? (
              <p className="text-sm text-cosmic-muted py-4 pl-2">
                No obligations assigned to Leo
              </p>
            ) : (
              <div className="space-y-2">
                {sortByPriority(leo).map((o) => (
                  <ObligationItem key={o.id} obligation={o} />
                ))}
              </div>
            )}
          </section>

          {/* Other */}
          {other.length > 0 && (
            <section>
              <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide mb-3">
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
            <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted">
              <CheckSquare size={36} />
              <p className="text-sm">No active obligations found</p>
            </div>
          )}
        </div>
      ) : (
        /* History tab */
        <div className="space-y-2">
          {history.length === 0 ? (
            <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted">
              <Clock size={36} />
              <p className="text-sm">No completed or dismissed obligations</p>
            </div>
          ) : (
            sortByPriority(history).map((o) => (
              <ObligationItem key={o.id} obligation={o} />
            ))
          )}
        </div>
      )}
    </div>
  );
}
