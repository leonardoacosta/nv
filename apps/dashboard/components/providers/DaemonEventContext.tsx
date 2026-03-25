"use client";

import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  useCallback,
  type ReactNode,
} from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type WsStatus = "connected" | "reconnecting" | "disconnected";

export interface DaemonEvent {
  type: string;
  payload: unknown;
  ts: number;
}

export type DaemonEventHandler = (event: DaemonEvent) => void;

interface DaemonEventContextValue {
  status: WsStatus;
  /** Subscribe to all events, or filter by event type prefix */
  subscribe: (handler: DaemonEventHandler, filter?: string) => () => void;
  /** Last received event (for simple one-off consumers) */
  lastEvent: DaemonEvent | null;
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

const DaemonEventContext = createContext<DaemonEventContextValue | null>(null);

// ---------------------------------------------------------------------------
// Backoff constants
// ---------------------------------------------------------------------------

const BACKOFF_STEPS = [1_000, 2_000, 4_000, 8_000, 15_000, 30_000];

function nextDelay(attempt: number): number {
  return BACKOFF_STEPS[Math.min(attempt, BACKOFF_STEPS.length - 1)] ?? 30_000;
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

export function DaemonEventProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<WsStatus>("disconnected");
  const [lastEvent, setLastEvent] = useState<DaemonEvent | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const attemptRef = useRef(0);
  const retryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const unmountedRef = useRef(false);

  // Subscriber registry: map of handler → optional type-prefix filter
  const handlersRef = useRef<Map<DaemonEventHandler, string | undefined>>(
    new Map(),
  );

  const clearRetry = () => {
    if (retryTimerRef.current !== null) {
      clearTimeout(retryTimerRef.current);
      retryTimerRef.current = null;
    }
  };

  const connect = useCallback(() => {
    if (unmountedRef.current) return;
    if (wsRef.current) {
      wsRef.current.onopen = null;
      wsRef.current.onclose = null;
      wsRef.current.onerror = null;
      wsRef.current.onmessage = null;
      wsRef.current.close();
      wsRef.current = null;
    }

    const wsUrl = (() => {
      if (typeof window === "undefined") return null;
      const proto = window.location.protocol === "https:" ? "wss" : "ws";
      const host = process.env.NEXT_PUBLIC_DAEMON_WS_HOST ?? window.location.host;
      return `${proto}://${host}/ws/events`;
    })();

    if (!wsUrl) return;

    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      if (unmountedRef.current) { ws.close(); return; }
      attemptRef.current = 0;
      setStatus("connected");
    };

    ws.onmessage = (ev) => {
      if (unmountedRef.current) return;
      let event: DaemonEvent;
      try {
        const parsed = JSON.parse(ev.data as string) as {
          type?: string;
          payload?: unknown;
          ts?: number;
        };
        event = {
          type: parsed.type ?? "unknown",
          payload: parsed.payload ?? parsed,
          ts: parsed.ts ?? Date.now(),
        };
      } catch {
        event = { type: "raw", payload: ev.data, ts: Date.now() };
      }

      setLastEvent(event);
      handlersRef.current.forEach((filter, handler) => {
        if (!filter || event.type.startsWith(filter)) {
          handler(event);
        }
      });
    };

    ws.onerror = () => {
      // onclose will fire next; nothing extra needed here
    };

    ws.onclose = () => {
      if (unmountedRef.current) return;
      wsRef.current = null;
      const delay = nextDelay(attemptRef.current);
      attemptRef.current += 1;
      setStatus("reconnecting");
      retryTimerRef.current = setTimeout(connect, delay);
    };
  }, []);

  useEffect(() => {
    unmountedRef.current = false;
    connect();
    return () => {
      unmountedRef.current = true;
      clearRetry();
      if (wsRef.current) {
        wsRef.current.onclose = null; // prevent reconnect loop on unmount
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [connect]);

  const subscribe = useCallback(
    (handler: DaemonEventHandler, filter?: string): (() => void) => {
      handlersRef.current.set(handler, filter);
      return () => {
        handlersRef.current.delete(handler);
      };
    },
    [],
  );

  return (
    <DaemonEventContext.Provider value={{ status, subscribe, lastEvent }}>
      {children}
    </DaemonEventContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * useDaemonEvents — subscribe to daemon WebSocket events.
 *
 * @param handler  called for every event matching the optional filter
 * @param filter   optional event-type prefix (e.g. "session", "approval")
 *
 * @example
 *   useDaemonEvents((ev) => console.log(ev), "session");
 */
export function useDaemonEvents(
  handler: DaemonEventHandler,
  filter?: string,
): WsStatus {
  const ctx = useContext(DaemonEventContext);
  if (!ctx) {
    throw new Error("useDaemonEvents must be used inside DaemonEventProvider");
  }

  const { subscribe, status } = ctx;

  // Stable ref so subscribe() doesn't re-run when handler identity changes
  const handlerRef = useRef<DaemonEventHandler>(handler);
  useEffect(() => {
    handlerRef.current = handler;
  });

  const stableHandler = useCallback<DaemonEventHandler>(
    (ev) => handlerRef.current(ev),
    [],
  );

  useEffect(() => {
    return subscribe(stableHandler, filter);
  }, [subscribe, stableHandler, filter]);

  return status;
}

/**
 * useDaemonStatus — read-only access to the current WebSocket connection status.
 */
export function useDaemonStatus(): WsStatus {
  const ctx = useContext(DaemonEventContext);
  if (!ctx) {
    throw new Error("useDaemonStatus must be used inside DaemonEventProvider");
  }
  return ctx.status;
}

export default DaemonEventContext;
