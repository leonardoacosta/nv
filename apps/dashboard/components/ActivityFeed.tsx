"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { CheckCircle, XCircle, Plus, Play, Zap, Wifi, WifiOff } from "lucide-react";
import { useDaemonEvents } from "@/components/providers/DaemonEventContext";
import type { ObligationActivity, ObligationActivityGetResponse } from "@/types/api";
import { apiFetch } from "@/lib/api-client";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function relativeTime(timestamp: string): string {
  const diff = Date.now() - new Date(timestamp).getTime();
  const s = Math.floor(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

// ---------------------------------------------------------------------------
// Icon config
// ---------------------------------------------------------------------------

type EventIconConfig = { icon: React.ReactNode; color: string };

function getEventConfig(eventType: string): EventIconConfig {
  if (eventType === "obligation.execution_completed" || eventType === "obligation.confirmed") {
    return { icon: <CheckCircle size={14} aria-hidden="true" />, color: "text-green-700" };
  }
  if (eventType === "obligation.tool_called") {
    return { icon: <Zap size={14} aria-hidden="true" />, color: "text-amber-700" };
  }
  if (eventType === "obligation.execution_failed" || eventType === "obligation.dismissed") {
    return { icon: <XCircle size={14} aria-hidden="true" />, color: "text-red-700" };
  }
  if (eventType === "obligation.detected") {
    return { icon: <Plus size={14} aria-hidden="true" />, color: "text-blue-700" };
  }
  // execution_started, reopened, or any other obligation event
  return { icon: <Play size={14} aria-hidden="true" />, color: "text-ds-gray-700" };
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const POLL_INTERVAL_MS = 10_000;
const MAX_EVENTS = 50;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface ActivityFeedProps {
  /** Optional: called when user clicks an obligation ID link */
  onObligationClick?: (id: string) => void;
}

export default function ActivityFeed({ onObligationClick }: ActivityFeedProps) {
  const [events, setEvents] = useState<ObligationActivity[]>([]);
  const [wsActive, setWsActive] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Fetch initial + poll fallback
  const fetchActivity = useCallback(async () => {
    try {
      const res = await apiFetch(`/api/obligations/activity?limit=${MAX_EVENTS}`);
      if (!res.ok) return;
      const data = (await res.json()) as ObligationActivityGetResponse;
      if (data.events) {
        setEvents(data.events.slice(0, MAX_EVENTS));
      }
    } catch {
      // silently ignore — feed degrades gracefully
    }
  }, []);

  // Initial load
  useEffect(() => {
    void fetchActivity();
  }, [fetchActivity]);

  // WebSocket subscription
  const wsStatus = useDaemonEvents(
    useCallback(
      (ev) => {
        if (!ev.type.startsWith("obligation.")) return;
        const payload = ev.payload as Partial<ObligationActivity>;
        const newEvent: ObligationActivity = {
          id: payload.id ?? `ws-${ev.ts}`,
          event_type: payload.event_type ?? ev.type,
          obligation_id: payload.obligation_id ?? "",
          description: payload.description ?? ev.type,
          timestamp: payload.timestamp ?? new Date(ev.ts).toISOString(),
          metadata: payload.metadata,
        };
        setEvents((prev) => [newEvent, ...prev].slice(0, MAX_EVENTS));
      },
      [],
    ),
    "obligation.",
  );

  // Track whether WS is delivering events
  useEffect(() => {
    setWsActive(wsStatus === "connected");
  }, [wsStatus]);

  // Polling fallback when WS is not connected
  useEffect(() => {
    if (wsActive) {
      if (pollRef.current !== null) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
      return;
    }
    pollRef.current = setInterval(() => {
      void fetchActivity();
    }, POLL_INTERVAL_MS);
    return () => {
      if (pollRef.current !== null) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [wsActive, fetchActivity]);

  return (
    <div className="surface-card flex flex-col gap-0 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-ds-gray-400">
        <span className="text-label-13 font-semibold text-ds-gray-1000">Activity</span>
        <div className="flex items-center gap-1.5">
          {wsActive ? (
            <Wifi size={12} className="text-green-700" aria-label="Live" />
          ) : (
            <WifiOff size={12} className="text-ds-gray-700" aria-label="Polling" />
          )}
          <span className="text-label-12 text-ds-gray-900">
            {wsActive ? "Live" : "Polling"}
          </span>
        </div>
      </div>

      {/* Event list */}
      <div className="overflow-y-auto max-h-[calc(100vh-320px)] divide-y divide-ds-gray-200">
        {events.length === 0 ? (
          <div className="px-4 py-8 text-center text-copy-14 text-ds-gray-700">
            No activity yet
          </div>
        ) : (
          events.map((ev) => {
            const { icon, color } = getEventConfig(ev.event_type);
            return (
              <div key={ev.id} className="flex gap-3 px-4 py-3 hover:bg-ds-gray-alpha-100 transition-colors">
                {/* Icon */}
                <div className={`mt-0.5 shrink-0 ${color}`}>{icon}</div>

                {/* Content */}
                <div className="flex-1 min-w-0">
                  <p className="text-copy-14 text-ds-gray-1000 leading-snug">
                    {ev.description}
                  </p>
                  <div className="flex items-center gap-2 mt-0.5">
                    <span className="text-label-13-mono text-ds-gray-900" suppressHydrationWarning>
                      {relativeTime(ev.timestamp)}
                    </span>
                    {ev.obligation_id && (
                      <button
                        type="button"
                        onClick={() => onObligationClick?.(ev.obligation_id)}
                        className="text-label-12 text-ds-gray-700 font-mono hover:text-ds-gray-1000 transition-colors truncate max-w-[80px]"
                      >
                        {ev.obligation_id.slice(0, 8)}
                      </button>
                    )}
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
