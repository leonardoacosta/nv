"use client";

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { Bot, Send, User, Loader2, WifiOff } from "lucide-react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { useTRPC } from "@/lib/trpc/react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import { MarkdownContent } from "@/lib/markdown";
import { channelAccentColor } from "@/lib/channel-colors";
// apiFetch retained for SSE streaming chat and Telegram polling
// (no tRPC chat router exists for send/SSE)
import { apiFetch } from "@/lib/api-client";
import type { StoredMessage, ChatSSEEvent } from "@/types/api";
import {
  useDaemonEvents,
  type DaemonEvent,
  type WsStatus,
} from "@/components/providers/DaemonEventContext";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TransportMode = "direct" | "telegram";

/** Shape of the WsEvent payload the daemon broadcasts for message.* events */
interface MessageWsEvent {
  type: "message.user" | "message.chunk" | "message.complete" | "message.typing" | "ping";
  channel?: string;
  sender?: string;
  messageId?: string;
  content?: string;
  chunk?: string;
  timestamp: number;
}

/** Per-message streaming accumulation state */
interface StreamingEntry {
  messageId: string;
  channel: string;
  accumulated: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function isMessageWsEvent(payload: unknown): payload is MessageWsEvent {
  return (
    typeof payload === "object" &&
    payload !== null &&
    "type" in payload &&
    typeof (payload as Record<string, unknown>).type === "string"
  );
}

// ---------------------------------------------------------------------------
// TypingIndicator — pulsing dots
// ---------------------------------------------------------------------------

function TypingIndicator() {
  return (
    <div className="flex items-center gap-1 py-1">
      <span className="w-1.5 h-1.5 rounded-full bg-ds-gray-700 animate-bounce [animation-delay:0ms]" />
      <span className="w-1.5 h-1.5 rounded-full bg-ds-gray-700 animate-bounce [animation-delay:150ms]" />
      <span className="w-1.5 h-1.5 rounded-full bg-ds-gray-700 animate-bounce [animation-delay:300ms]" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// ChannelBadge — small colored dot + channel name
// ---------------------------------------------------------------------------

function ChannelBadge({ channel }: { channel: string }) {
  const accent = channelAccentColor(channel.toLowerCase());
  return (
    <span className="inline-flex items-center gap-1">
      <span
        className="w-1.5 h-1.5 rounded-full shrink-0"
        style={{ backgroundColor: accent }}
      />
      <span
        className="text-[10px] font-mono capitalize"
        style={{ color: accent }}
      >
        {channel}
      </span>
    </span>
  );
}

// ---------------------------------------------------------------------------
// MessageBubble
// ---------------------------------------------------------------------------

interface MessageBubbleProps {
  message: StoredMessage;
}

function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.direction === "inbound";

  return (
    <div
      className={`flex gap-2.5 ${isUser ? "justify-end" : "justify-start"}`}
    >
      {/* Nova avatar */}
      {!isUser && (
        <div className="w-7 h-7 rounded-full bg-ds-gray-300 flex items-center justify-center shrink-0 mt-1">
          <Bot size={14} className="text-ds-gray-900" />
        </div>
      )}

      <div
        className={`max-w-[75%] rounded-lg px-3.5 py-2.5 ${
          isUser
            ? "bg-blue-700/10 border border-blue-700/20"
            : "bg-ds-gray-200 border border-ds-gray-400"
        }`}
      >
        {/* Header: sender + channel badge + timestamp */}
        <div className="flex items-center gap-2 mb-1.5">
          <span
            className={`text-[11px] font-medium ${
              isUser ? "text-blue-700" : "text-ds-gray-900"
            }`}
          >
            {isUser ? message.sender || "You" : "Nova"}
          </span>
          <ChannelBadge channel={message.channel} />
          <span
            className="text-[10px] font-mono text-ds-gray-700 ml-auto"
            suppressHydrationWarning
          >
            {formatTimestamp(message.timestamp)}
          </span>
        </div>

        {/* Content */}
        {isUser ? (
          <p className="text-copy-13 text-ds-gray-1000 whitespace-pre-wrap break-words">
            {message.content}
          </p>
        ) : (
          <div className="text-ds-gray-1000">
            <MarkdownContent content={message.content} />
          </div>
        )}
      </div>

      {/* User avatar */}
      {isUser && (
        <div className="w-7 h-7 rounded-full bg-blue-700/20 flex items-center justify-center shrink-0 mt-1">
          <User size={14} className="text-blue-700" />
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// StreamingBubble — Nova bubble that shows typing indicator then streaming text
// Task 4.5: accepts optional channel prop to show correct ChannelBadge
// ---------------------------------------------------------------------------

function StreamingBubble({ text, channel = "dashboard" }: { text: string; channel?: string }) {
  return (
    <div className="flex gap-2.5 justify-start">
      <div className="w-7 h-7 rounded-full bg-ds-gray-300 flex items-center justify-center shrink-0 mt-1">
        <Bot size={14} className="text-ds-gray-900" />
      </div>

      <div className="max-w-[75%] rounded-lg px-3.5 py-2.5 bg-ds-gray-200 border border-ds-gray-400">
        <div className="flex items-center gap-2 mb-1.5">
          <span className="text-[11px] font-medium text-ds-gray-900">
            Nova
          </span>
          <ChannelBadge channel={channel} />
        </div>
        {text ? (
          <div className="text-ds-gray-1000">
            <MarkdownContent content={text} />
          </div>
        ) : (
          <TypingIndicator />
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// TransportBadge — shows "Direct" or "Telegram" mode indicator
// ---------------------------------------------------------------------------

function TransportBadge({ mode }: { mode: TransportMode }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-[11px] font-medium">
      <span
        className={`w-1.5 h-1.5 rounded-full shrink-0 ${
          mode === "direct" ? "bg-green-700" : "bg-[#229ED9]"
        }`}
      />
      <span className="text-ds-gray-700">
        {mode === "direct" ? "Direct" : "Telegram"}
      </span>
    </span>
  );
}

// ---------------------------------------------------------------------------
// DisconnectionBanner — Task 4.7
// ---------------------------------------------------------------------------

function DisconnectionBanner({ status }: { status: WsStatus }) {
  if (status === "connected") return null;
  return (
    <div className="shrink-0 mb-2 flex items-center gap-2 px-3 py-2 rounded-md bg-ds-gray-200 border border-ds-gray-400 text-ds-gray-700 text-[12px]">
      <WifiOff size={13} className="shrink-0" />
      <span>
        {status === "reconnecting"
          ? "Live updates paused — reconnecting..."
          : "Live updates disconnected — check daemon status."}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// SSE stream reader
// ---------------------------------------------------------------------------

async function readSSEStream(
  response: Response,
  onChunk: (text: string) => void,
  onDone: (fullText: string) => void,
  onError: (message: string) => void,
) {
  const reader = response.body!.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const parts = buffer.split("\n\n");
      // Keep the last (possibly incomplete) chunk in the buffer
      buffer = parts.pop() ?? "";

      for (const part of parts) {
        const lines = part.split("\n");
        for (const line of lines) {
          if (!line.startsWith("data: ")) continue;
          const jsonStr = line.slice(6);
          try {
            const event = JSON.parse(jsonStr) as ChatSSEEvent;
            if (event.type === "chunk") {
              onChunk(event.text);
            } else if (event.type === "done") {
              onDone(event.full_text);
            } else if (event.type === "error") {
              onError(event.message);
            }
          } catch {
            // Malformed JSON line — skip
          }
        }
      }
    }
  } catch (err) {
    onError(err instanceof Error ? err.message : "Stream read error");
  }
}

// ---------------------------------------------------------------------------
// Telegram fallback polling
// ---------------------------------------------------------------------------

async function pollForTelegramResponse(
  sentAt: number,
  onResponse: (msg: StoredMessage) => void,
  onTimeout: () => void,
): Promise<void> {
  const MAX_ATTEMPTS = 10;
  const POLL_INTERVAL = 3000;

  for (let i = 0; i < MAX_ATTEMPTS; i++) {
    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL));

    try {
      const res = await apiFetch("/api/messages?channel=telegram&limit=5");
      if (!res.ok) continue;

      const data = (await res.json()) as { messages: StoredMessage[] };
      const novaReply = data.messages?.find(
        (m) =>
          m.sender === "nova" &&
          new Date(m.timestamp).getTime() > sentAt,
      );
      if (novaReply) {
        onResponse(novaReply);
        return;
      }
    } catch {
      // Retry on next attempt
    }
  }

  onTimeout();
}

// ---------------------------------------------------------------------------
// ChatPage
// ---------------------------------------------------------------------------

export default function ChatPage() {
  const trpc = useTRPC();
  const queryClient = useQueryClient();

  // --- Local state ---
  const [pendingMessages, setPendingMessages] = useState<StoredMessage[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [sending, setSending] = useState(false);
  const [streamingText, setStreamingText] = useState("");
  const [sendError, setSendError] = useState<string | null>(null);
  const [transportMode, setTransportMode] = useState<TransportMode>("direct");
  const [telegramPolling, setTelegramPolling] = useState(false);

  // --- Task 4.3: cross-channel streaming state (keyed by messageId) ---
  const [crossChannelStreams, setCrossChannelStreams] = useState<
    Map<string, StreamingEntry>
  >(new Map());

  // --- Refs ---
  const scrollRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // --- Infinite query for chat history ---
  const {
    data,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
    isLoading,
    error: historyError,
    refetch,
  } = useInfiniteQuery(
    trpc.message.chatHistory.infiniteQueryOptions(
      { limit: 25 },
      {
        getNextPageParam: (lastPage) => lastPage.nextCursor ?? undefined,
      },
    ),
  );

  // --- Merged message list: pending (newest) on top in col-reverse layout ---
  // flex-col-reverse means the DOM order is reversed visually:
  // pendingMessages appear at the visual bottom, history pages above.
  const historyMessages = data?.pages.flatMap((page) => page.messages) ?? [];
  const allMessages = [...pendingMessages, ...historyMessages];

  // --- IntersectionObserver for upward pagination ---
  useEffect(() => {
    if (!sentinelRef.current || !scrollRef.current) return;

    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        if (entry?.isIntersecting && hasNextPage && !isFetchingNextPage) {
          void fetchNextPage();
        }
      },
      {
        root: scrollRef.current,
        rootMargin: "200px 0px 0px 0px",
      },
    );

    observer.observe(sentinelRef.current);

    return () => {
      observer.disconnect();
    };
  }, [fetchNextPage, hasNextPage, isFetchingNextPage]);

  // --- Scroll to bottom (col-reverse: scrollTop = 0) ---
  const scrollToBottom = useCallback(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 0;
    }
  }, []);

  // --- Sync pending messages against cache after refetch ---
  useEffect(() => {
    if (!data?.pages[0]) return;
    const cachedIds = new Set(
      data.pages[0].messages.map((m) => `${m.timestamp}-${m.sender}`),
    );
    setPendingMessages((prev) =>
      prev.filter(
        (pm) => !cachedIds.has(`${pm.timestamp}-${pm.sender}`),
      ),
    );
  }, [data?.pages]);

  // --- Auto-grow textarea ---
  const handleTextareaInput = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = "auto";
    // Max 4 lines (~96px)
    const maxHeight = 96;
    textarea.style.height = `${Math.min(textarea.scrollHeight, maxHeight)}px`;
  }, []);

  // ---------------------------------------------------------------------------
  // Tasks 4.1–4.4: WebSocket message event handler
  // ---------------------------------------------------------------------------

  const prevWsStatusRef = useRef<WsStatus>("disconnected");

  const wsStatus = useDaemonEvents(
    useCallback(
      (event: DaemonEvent) => {
        if (!isMessageWsEvent(event.payload)) return;
        const ev = event.payload;

        if (event.type === "message.user") {
          // Task 4.2: append inbound StoredMessage; skip dashboard messages from self
          if (ev.channel === "dashboard") return; // SSE already handled on dashboard channel
          const incoming: StoredMessage = {
            id: Date.now(),
            timestamp: new Date(ev.timestamp).toISOString(),
            direction: "inbound",
            channel: ev.channel ?? "unknown",
            sender: ev.sender ?? "unknown",
            content: ev.content ?? "",
            response_time_ms: null,
            tokens_in: null,
            tokens_out: null,
            type: "conversation",
          };
          setPendingMessages((prev) => [incoming, ...prev]);
        } else if (event.type === "message.chunk") {
          // Task 4.3: accumulate chunks for cross-channel streams only
          // Dashboard SSE already handles dashboard channel streaming
          if (ev.channel === "dashboard") return;
          const id = ev.messageId ?? "unknown";
          setCrossChannelStreams((prev) => {
            const existing = prev.get(id);
            const updated = new Map(prev);
            updated.set(id, {
              messageId: id,
              channel: ev.channel ?? "unknown",
              accumulated: (existing?.accumulated ?? "") + (ev.chunk ?? ""),
            });
            return updated;
          });
        } else if (event.type === "message.complete") {
          // Task 4.4: finalize cross-channel streaming; skip dashboard (SSE handles it)
          if (ev.channel === "dashboard") return;
          const id = ev.messageId ?? "unknown";
          const finalMsg: StoredMessage = {
            id: Date.now() + 1,
            timestamp: new Date(ev.timestamp).toISOString(),
            direction: "outbound",
            channel: ev.channel ?? "unknown",
            sender: ev.sender ?? "nova",
            content: ev.content ?? "",
            response_time_ms: null,
            tokens_in: null,
            tokens_out: null,
            type: "conversation",
          };
          setPendingMessages((prev) => [finalMsg, ...prev]);
          setCrossChannelStreams((prev) => {
            const updated = new Map(prev);
            updated.delete(id);
            return updated;
          });
        }
      },
      [],
    ),
    "message", // Task 4.1: filter to "message" prefix
  );

  // ---------------------------------------------------------------------------
  // Task 4.6: reconnection catch-up — reload history on WS reconnect
  // ---------------------------------------------------------------------------

  useEffect(() => {
    if (prevWsStatusRef.current === "reconnecting" && wsStatus === "connected") {
      void refetch();
    }
    prevWsStatusRef.current = wsStatus;
  }, [wsStatus, refetch]);

  // --- Send message ---
  const handleSend = useCallback(
    async (e?: FormEvent) => {
      e?.preventDefault();
      const trimmed = inputValue.trim();
      if (!trimmed || sending) return;

      // Optimistically append user message to pending
      const userMsg: StoredMessage = {
        id: Date.now(),
        timestamp: new Date().toISOString(),
        direction: "inbound",
        channel: "dashboard",
        sender: "You",
        content: trimmed,
        response_time_ms: null,
        tokens_in: null,
        tokens_out: null,
        type: "conversation",
      };
      setPendingMessages((prev) => [userMsg, ...prev]);
      setInputValue("");
      setSending(true);
      setStreamingText("");
      setSendError(null);

      // Scroll to bottom after sending
      scrollToBottom();

      // Reset textarea height
      if (textareaRef.current) {
        textareaRef.current.style.height = "auto";
      }

      try {
        const res = await apiFetch("/api/chat/send", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ message: trimmed }),
        });

        // Telegram fallback on 503
        if (res.status === 503) {
          setTransportMode("telegram");
          setTelegramPolling(true);

          const sentAt = Date.now();
          await pollForTelegramResponse(
            sentAt,
            (novaReply) => {
              setPendingMessages((prev) => [novaReply, ...prev]);
              setTelegramPolling(false);
              // Invalidate to sync server state
              void queryClient.invalidateQueries({
                queryKey: trpc.message.chatHistory.queryKey(),
              });
            },
            () => {
              setSendError("Telegram response timed out. Nova may still reply shortly.");
              setTelegramPolling(false);
            },
          );

          setSending(false);
          return;
        }

        if (!res.ok) {
          throw new Error(`HTTP ${res.status}`);
        }

        // SSE streaming
        setTransportMode("direct");
        let accumulated = "";

        await readSSEStream(
          res,
          (chunkText) => {
            accumulated += chunkText;
            setStreamingText(accumulated);
          },
          (fullText) => {
            // Append complete Nova message to pending
            const novaMsg: StoredMessage = {
              id: Date.now() + 1,
              timestamp: new Date().toISOString(),
              direction: "outbound",
              channel: "dashboard",
              sender: "nova",
              content: fullText,
              response_time_ms: null,
              tokens_in: null,
              tokens_out: null,
              type: "conversation",
            };
            setPendingMessages((prev) => [novaMsg, ...prev]);
            setStreamingText("");
            // Sync server state after Nova responds
            void queryClient.invalidateQueries({
              queryKey: trpc.message.chatHistory.queryKey(),
            });
          },
          (errMsg) => {
            setSendError(errMsg);
            setStreamingText("");
          },
        );
      } catch (err) {
        setSendError(
          err instanceof Error ? err.message : "Failed to send message",
        );
      } finally {
        setSending(false);
      }
    },
    [inputValue, sending, scrollToBottom, queryClient, trpc],
  );

  // --- Keyboard handling ---
  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        void handleSend();
      }
    },
    [handleSend],
  );

  // Combine errors — show history error unless there's a send error
  const displayError = sendError ?? (historyError?.message ?? null);

  // Cross-channel streaming bubbles as array for rendering
  const crossChannelStreamList = Array.from(crossChannelStreams.values());

  // --- Render ---
  return (
    <PageShell title="Chat" subtitle="Talk to Nova directly">
      <div className="flex flex-col h-[calc(100vh-12rem)]">
        {/* Task 4.7: Disconnection banner */}
        <DisconnectionBanner status={wsStatus} />

        {/* Error banner */}
        {displayError && (
          <div className="shrink-0 mb-3">
            <ErrorBanner
              message="Chat error"
              detail={displayError}
              onRetry={sendError ? () => setSendError(null) : () => void refetch()}
            />
          </div>
        )}

        {/* Messages container — flex-col-reverse so newest is at bottom */}
        <div
          ref={scrollRef}
          className="flex-1 overflow-y-auto min-h-0 flex flex-col-reverse"
          style={{ overflowAnchor: "auto" }}
        >
          {isLoading ? (
            // Skeleton — rendered inside col-reverse so it appears at top
            <div className="flex flex-col gap-4 p-4">
              {Array.from({ length: 8 }).map((_, i) => {
                const isRight = i % 3 === 0;
                return (
                  <div
                    key={i}
                    className={`flex ${isRight ? "justify-end" : "justify-start"}`}
                  >
                    <div
                      className="animate-pulse rounded-lg bg-ds-gray-300"
                      style={{
                        width: `${35 + ((i * 17) % 40)}%`,
                        height: `${36 + ((i * 7) % 24)}px`,
                        opacity: 1 - i * 0.08,
                      }}
                    />
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="flex flex-col gap-3 py-4">
              {/* Empty state */}
              {allMessages.length === 0 && !sending && crossChannelStreamList.length === 0 && (
                <div className="flex flex-col items-center justify-center py-16 text-center">
                  <Bot size={32} className="text-ds-gray-700 mb-3" />
                  <p className="text-copy-13 text-ds-gray-900">
                    No messages yet. Start a conversation with Nova.
                  </p>
                </div>
              )}

              {/* Dashboard SSE streaming / typing indicator */}
              {sending && !telegramPolling && (
                <StreamingBubble text={streamingText} channel="dashboard" />
              )}

              {/* Task 4.3: cross-channel streaming bubbles */}
              {crossChannelStreamList.map((stream) => (
                <StreamingBubble
                  key={stream.messageId}
                  text={stream.accumulated}
                  channel={stream.channel}
                />
              ))}

              {/* Telegram polling indicator */}
              {telegramPolling && (
                <div className="flex gap-2.5 justify-start">
                  <div className="w-7 h-7 rounded-full bg-ds-gray-300 flex items-center justify-center shrink-0 mt-1">
                    <Bot size={14} className="text-ds-gray-900" />
                  </div>
                  <div className="max-w-[75%] rounded-lg px-3.5 py-2.5 bg-ds-gray-200 border border-ds-gray-400">
                    <div className="flex items-center gap-2">
                      <TypingIndicator />
                      <span className="text-[11px] text-ds-gray-700 font-medium">
                        Sent via Telegram — waiting for response...
                      </span>
                    </div>
                  </div>
                </div>
              )}

              {/* All messages (pending + history) */}
              {allMessages.map((msg, idx) => (
                <MessageBubble key={`${msg.id}-${idx}`} message={msg} />
              ))}

              {/* Load-more spinner while fetching next page */}
              {isFetchingNextPage && (
                <div className="flex justify-center py-3">
                  <Loader2 size={16} className="animate-spin text-ds-gray-700" />
                </div>
              )}

              {/* Sentinel for IntersectionObserver — above oldest messages */}
              <div ref={sentinelRef} className="h-px" />
            </div>
          )}
        </div>

        {/* Input bar */}
        <div className="shrink-0 border-t border-ds-gray-400 pt-3">
          <form
            onSubmit={(e) => void handleSend(e)}
            className="flex items-end gap-2"
          >
            <div className="flex-1 relative">
              <textarea
                ref={textareaRef}
                value={inputValue}
                onChange={(e) => {
                  setInputValue(e.target.value);
                  handleTextareaInput();
                }}
                onKeyDown={handleKeyDown}
                placeholder="Message Nova..."
                disabled={sending}
                rows={1}
                className="w-full resize-none rounded-lg border border-ds-gray-400 bg-ds-gray-100 px-3.5 py-2.5 text-copy-13 text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-hidden focus:border-ds-gray-700 transition-colors disabled:opacity-50"
                style={{ maxHeight: "96px" }}
              />
              {/* Transport mode indicator */}
              <div className="absolute right-2 bottom-1">
                <TransportBadge mode={transportMode} />
              </div>
            </div>

            <button
              type="submit"
              disabled={!inputValue.trim() || sending}
              className="flex items-center justify-center w-10 h-10 rounded-lg bg-ds-gray-200 border border-ds-gray-400 text-ds-gray-900 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed shrink-0"
              aria-label="Send message"
            >
              <Send size={16} />
            </button>
          </form>
        </div>
      </div>
    </PageShell>
  );
}
