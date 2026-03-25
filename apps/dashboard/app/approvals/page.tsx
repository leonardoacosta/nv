"use client";

import { useCallback, useEffect, useState } from "react";
import {
  CheckCircle,
  XCircle,
  Clock,
  RefreshCw,
  AlertTriangle,
  FileText,
  GitPullRequest,
  Terminal,
  HelpCircle,
  ChevronRight,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ApprovalActionType =
  | "file_write"
  | "file_delete"
  | "shell_exec"
  | "git_push"
  | "api_call"
  | "other";

export type ApprovalStatus = "pending" | "approved" | "dismissed";

export interface Approval {
  id: string;
  title: string;
  description?: string;
  action_type: ApprovalActionType;
  project?: string;
  proposed_changes?: string;
  context?: string;
  urgency: "low" | "medium" | "high" | "critical";
  status: ApprovalStatus;
  created_at: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ACTION_ICON: Record<ApprovalActionType, React.ElementType> = {
  file_write: FileText,
  file_delete: FileText,
  shell_exec: Terminal,
  git_push: GitPullRequest,
  api_call: GitPullRequest,
  other: HelpCircle,
};

const URGENCY_CONFIG: Record<
  Approval["urgency"],
  { label: string; dot: string; text: string }
> = {
  critical: {
    label: "Critical",
    dot: "bg-[#EF4444]",
    text: "text-[#EF4444]",
  },
  high: { label: "High", dot: "bg-[#F97316]", text: "text-[#F97316]" },
  medium: {
    label: "Medium",
    dot: "bg-amber-400",
    text: "text-amber-400",
  },
  low: { label: "Low", dot: "bg-cosmic-muted", text: "text-cosmic-muted" },
};

function relativeTime(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffH = Math.floor(diffMin / 60);
  if (diffH < 24) return `${diffH}h ago`;
  return `${Math.floor(diffH / 24)}d ago`;
}

// ---------------------------------------------------------------------------
// QueueItem
// ---------------------------------------------------------------------------

interface QueueItemProps {
  approval: Approval;
  selected: boolean;
  onSelect: () => void;
}

function QueueItem({ approval, selected, onSelect }: QueueItemProps) {
  const ActionIcon = ACTION_ICON[approval.action_type] ?? HelpCircle;
  const urg = URGENCY_CONFIG[approval.urgency];

  return (
    <button
      type="button"
      onClick={onSelect}
      className={[
        "w-full text-left flex items-start gap-3 px-4 py-3.5 min-h-11 transition-colors",
        "border-b border-cosmic-border last:border-b-0",
        selected
          ? "bg-cosmic-purple/15"
          : "hover:bg-cosmic-surface/60",
      ].join(" ")}
    >
      {/* Action type icon */}
      <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-cosmic-surface border border-cosmic-border shrink-0 mt-0.5">
        <ActionIcon size={14} className="text-cosmic-muted" />
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-sm font-medium text-cosmic-bright truncate">
            {approval.title}
          </span>
          {/* Urgency dot */}
          <span
            className={`inline-block w-2 h-2 rounded-full shrink-0 ${urg.dot}`}
            aria-label={`Urgency: ${urg.label}`}
            title={`Urgency: ${urg.label}`}
          />
        </div>

        <div className="flex items-center gap-2 mt-0.5 flex-wrap">
          {approval.project && (
            <span className="text-xs font-mono text-cosmic-muted truncate">
              {approval.project}
            </span>
          )}
          <span className="text-xs text-cosmic-muted flex items-center gap-1">
            <Clock size={10} />
            <span suppressHydrationWarning>{relativeTime(approval.created_at)}</span>
          </span>
        </div>
      </div>

      <ChevronRight
        size={14}
        className={`shrink-0 mt-1 transition-colors ${
          selected ? "text-cosmic-purple" : "text-cosmic-muted/40"
        }`}
      />
    </button>
  );
}

// ---------------------------------------------------------------------------
// DetailPanel
// ---------------------------------------------------------------------------

interface DetailPanelProps {
  approval: Approval;
  onApprove: (id: string) => Promise<void>;
  onDismiss: (id: string) => Promise<void>;
  approving: boolean;
  dismissing: boolean;
}

function DetailPanel({
  approval,
  onApprove,
  onDismiss,
  approving,
  dismissing,
}: DetailPanelProps) {
  const ActionIcon = ACTION_ICON[approval.action_type] ?? HelpCircle;
  const urg = URGENCY_CONFIG[approval.urgency];

  return (
    <div className="flex flex-col h-full">
      {/* Detail header */}
      <div className="px-6 py-5 border-b border-cosmic-border shrink-0">
        <div className="flex items-start gap-3">
          <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-cosmic-purple/10 border border-cosmic-purple/30 shrink-0">
            <ActionIcon size={18} className="text-cosmic-purple" />
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-base font-semibold text-cosmic-bright leading-tight">
              {approval.title}
            </h2>
            <div className="flex items-center gap-3 mt-1 flex-wrap">
              {approval.project && (
                <span className="text-xs font-mono text-cosmic-muted">
                  {approval.project}
                </span>
              )}
              <span className={`text-xs font-medium ${urg.text}`}>
                {urg.label} urgency
              </span>
              <span
                className="text-xs text-cosmic-muted flex items-center gap-1"
                suppressHydrationWarning
              >
                <Clock size={10} />
                {relativeTime(approval.created_at)}
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Scrollable body */}
      <div className="flex-1 overflow-y-auto px-6 py-5 space-y-6">
        {approval.description && (
          <section>
            <h3 className="text-xs font-semibold text-cosmic-muted uppercase tracking-widest mb-2">
              Description
            </h3>
            <p className="text-sm text-cosmic-text leading-relaxed">
              {approval.description}
            </p>
          </section>
        )}

        {approval.proposed_changes && (
          <section>
            <h3 className="text-xs font-semibold text-cosmic-muted uppercase tracking-widest mb-2">
              Proposed Changes
            </h3>
            <pre className="text-xs text-cosmic-text font-mono bg-cosmic-dark border border-cosmic-border rounded-cosmic p-4 overflow-x-auto whitespace-pre-wrap">
              {approval.proposed_changes}
            </pre>
          </section>
        )}

        {approval.context && (
          <section>
            <h3 className="text-xs font-semibold text-cosmic-muted uppercase tracking-widest mb-2">
              Context
            </h3>
            <p className="text-sm text-cosmic-muted leading-relaxed">
              {approval.context}
            </p>
          </section>
        )}
      </div>

      {/* Action buttons */}
      <div className="px-6 py-4 border-t border-cosmic-border shrink-0">
        <div className="flex gap-3">
          <button
            type="button"
            onClick={() => void onApprove(approval.id)}
            disabled={approving || dismissing}
            className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 min-h-11 rounded-lg text-sm font-semibold bg-emerald-600 hover:bg-emerald-500 text-white transition-colors disabled:opacity-50"
          >
            <CheckCircle size={16} />
            {approving ? "Approving…" : "Approve"}
          </button>
          <button
            type="button"
            onClick={() => void onDismiss(approval.id)}
            disabled={approving || dismissing}
            className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 min-h-11 rounded-lg text-sm font-semibold bg-cosmic-surface hover:bg-cosmic-rose/20 text-cosmic-muted hover:text-cosmic-rose border border-cosmic-border hover:border-cosmic-rose/40 transition-colors disabled:opacity-50"
          >
            <XCircle size={16} />
            {dismissing ? "Dismissing…" : "Dismiss"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ApprovalsPage
// ---------------------------------------------------------------------------

export default function ApprovalsPage() {
  // 1. State
  const [approvals, setApprovals] = useState<Approval[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [approving, setApproving] = useState(false);
  const [dismissing, setDismissing] = useState(false);
  // Mobile detail panel open
  const [mobileDetailOpen, setMobileDetailOpen] = useState(false);

  // 2. Derived
  const pending = approvals.filter((a) => a.status === "pending");
  const selected = pending.find((a) => a.id === selectedId) ?? null;

  // 3. Fetch
  const fetchApprovals = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/obligations?owner=leo&status=open");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as Approval[];
      setApprovals(data);
      // Auto-select first if none selected
      if (data.length > 0 && !selectedId) {
        setSelectedId(data[0]!.id);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load approvals");
    } finally {
      setLoading(false);
    }
  }, [selectedId]);

  // 4. Initial fetch
  useEffect(() => {
    void fetchApprovals();
  }, [fetchApprovals]);

  // 5. WebSocket — real-time approval.created events
  useDaemonEvents(
    useCallback(
      (_ev) => {
        void fetchApprovals();
      },
      [fetchApprovals],
    ),
    "approval",
  );

  // 6. Handlers
  const handleApprove = async (id: string) => {
    setApproving(true);
    try {
      const res = await fetch(`/api/approvals/${id}/approve`, {
        method: "POST",
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setApprovals((prev) => prev.filter((a) => a.id !== id));
      setSelectedId(null);
      setMobileDetailOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to approve");
    } finally {
      setApproving(false);
    }
  };

  const handleDismiss = async (id: string) => {
    setDismissing(true);
    try {
      const res = await fetch(`/api/obligations/${id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ status: "dismissed" }),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setApprovals((prev) => prev.filter((a) => a.id !== id));
      setSelectedId(null);
      setMobileDetailOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to dismiss");
    } finally {
      setDismissing(false);
    }
  };

  const handleSelect = (id: string) => {
    setSelectedId(id);
    setMobileDetailOpen(true);
  };

  // 7. Action slot
  const action = (
    <button
      type="button"
      onClick={() => void fetchApprovals()}
      disabled={loading}
      className="flex items-center gap-2 px-3 py-2 min-h-11 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
    >
      <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
      <span className="hidden sm:inline">Refresh</span>
    </button>
  );

  return (
    <PageShell
      title="Approvals"
      subtitle="Review and action pending requests"
      action={action}
    >
      {error && (
        <div className="mb-4">
          <ErrorBanner message={error} onRetry={() => void fetchApprovals()} />
        </div>
      )}

      {loading ? (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-16 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
            />
          ))}
        </div>
      ) : pending.length === 0 ? (
        <EmptyState
          title="No pending approvals"
          description="All requests have been actioned."
          icon={<CheckCircle size={24} />}
        />
      ) : (
        // Split view: list (left) + detail (right)
        // On mobile (<md): list only, tapping opens detail overlay
        <div className="flex gap-0 rounded-cosmic border border-cosmic-border overflow-hidden min-h-[500px]">
          {/* Queue list */}
          <div
            className={[
              "flex flex-col border-r border-cosmic-border bg-cosmic-dark overflow-y-auto",
              // Mobile: full width unless detail open
              mobileDetailOpen ? "hidden md:flex md:w-64 lg:w-72 shrink-0" : "w-full md:w-64 lg:w-72 shrink-0",
            ].join(" ")}
          >
            {/* List header */}
            <div className="flex items-center gap-2 px-4 py-3 border-b border-cosmic-border shrink-0">
              <AlertTriangle size={14} className="text-amber-400 shrink-0" />
              <span className="text-xs font-semibold text-cosmic-muted uppercase tracking-widest">
                Queue
              </span>
              <span className="ml-auto inline-flex items-center justify-center px-1.5 py-0.5 min-w-[1.25rem] rounded text-xs font-mono font-medium bg-cosmic-border text-cosmic-text">
                {pending.length}
              </span>
            </div>

            {pending.map((a) => (
              <QueueItem
                key={a.id}
                approval={a}
                selected={a.id === selectedId}
                onSelect={() => handleSelect(a.id)}
              />
            ))}
          </div>

          {/* Detail panel */}
          <div
            className={[
              "flex-1 bg-cosmic-surface",
              // Mobile: shown as full-width overlay when open
              mobileDetailOpen
                ? "flex flex-col w-full md:flex"
                : "hidden md:flex md:flex-col",
            ].join(" ")}
          >
            {/* Mobile back button */}
            <button
              type="button"
              onClick={() => setMobileDetailOpen(false)}
              className="flex md:hidden items-center gap-2 px-4 py-3 text-sm text-cosmic-muted hover:text-cosmic-text border-b border-cosmic-border"
            >
              <ChevronRight size={14} className="rotate-180" />
              Back to queue
            </button>

            {selected ? (
              <DetailPanel
                approval={selected}
                onApprove={handleApprove}
                onDismiss={handleDismiss}
                approving={approving}
                dismissing={dismissing}
              />
            ) : (
              <div className="flex flex-col items-center justify-center h-full gap-3 text-cosmic-muted py-16">
                <AlertTriangle size={32} />
                <p className="text-sm">Select an item to review</p>
              </div>
            )}
          </div>
        </div>
      )}
    </PageShell>
  );
}
