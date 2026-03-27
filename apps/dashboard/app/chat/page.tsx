"use client";

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { Bot, Send, User } from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import { MarkdownContent } from "@/lib/markdown";
import { channelAccentColor } from "@/lib/channel-colors";
import { apiFetch } from "@/lib/api-client";
import type {
  StoredMessage,
  MessagesGetResponse,
  ChatSSEEvent,
} from "@/types/api";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TransportMode = "direct" | "telegram";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
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
            ? "bg-blue-900/20 border border-blue-800/20"
            : "bg-ds-gray-200 border border-ds-gray-400"
        }`}
      >
        {/* Header: sender + channel badge + timestamp */}
        <div className="flex items-center gap-2 mb-1.5">
          <span
            className={`text-[11px] font-medium ${
              isUser ? "text-blue-400" : "text-ds-gray-900"
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
          <p className="text-sm text-ds-gray-1000 whitespace-pre-wrap break-words">
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
        <div className="w-7 h-7 rounded-full bg-blue-900/30 flex items-center justify-center shrink-0 mt-1">
          <User size={14} className="text-blue-400" />
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// StreamingBubble — Nova bubble that shows typing indicator then streaming text
// ---------------------------------------------------------------------------

function StreamingBubble({ text }: { text: string }) {
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
          <ChannelBadge channel="dashboard" />
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
          mode === "direct" ? "bg-green-500" : "bg-[#229ED9]"
        }`}
      />
      <span className="text-ds-gray-700">
        {mode === "direct" ? "Direct" : "Telegram"}
      </span>
    </span>
  );
}

// ---------------------------------------------------------------------------
// Loading skeleton
// ---------------------------------------------------------------------------

function ChatSkeleton() {
  return (
    <div className="flex-1 flex flex-col gap-4 p-4">
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
  // State [3.2]
  const [messages, setMessages] = useState<StoredMessage[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [sending, setSending] = useState(false);
  const [streamingText, setStreamingText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [transportMode, setTransportMode] = useState<TransportMode>("direct");
  const [loading, setLoading] = useState(true);
  const [telegramPolling, setTelegramPolling] = useState(false);

  // Refs [3.7]
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-scroll [3.7]
  const scrollToBottom = useCallback(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  // Scroll on mount, new messages, streaming updates
  useEffect(() => {
    scrollToBottom();
  }, [messages, streamingText, scrollToBottom]);

  // Initial message load [3.3]
  const loadHistory = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch("/api/chat/history");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as MessagesGetResponse;
      // History comes newest first — reverse for chat (oldest at top)
      setMessages((data.messages ?? []).reverse());
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load chat history",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadHistory();
  }, [loadHistory]);

  // Auto-grow textarea [3.8]
  const handleTextareaInput = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = "auto";
    // Max 4 lines (~96px)
    const maxHeight = 96;
    textarea.style.height = `${Math.min(textarea.scrollHeight, maxHeight)}px`;
  }, []);

  // Send message [3.5]
  const handleSend = useCallback(
    async (e?: FormEvent) => {
      e?.preventDefault();
      const trimmed = inputValue.trim();
      if (!trimmed || sending) return;

      // Append user message optimistically
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
      setMessages((prev) => [...prev, userMsg]);
      setInputValue("");
      setSending(true);
      setStreamingText("");
      setError(null);

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

        // Telegram fallback on 503 [3.9]
        if (res.status === 503) {
          setTransportMode("telegram");
          setTelegramPolling(true);

          const sentAt = Date.now();
          await pollForTelegramResponse(
            sentAt,
            (novaReply) => {
              setMessages((prev) => [...prev, novaReply]);
              setTelegramPolling(false);
            },
            () => {
              setError("Telegram response timed out. Nova may still reply shortly.");
              setTelegramPolling(false);
            },
          );

          setSending(false);
          return;
        }

        if (!res.ok) {
          throw new Error(`HTTP ${res.status}`);
        }

        // SSE streaming [3.5]
        setTransportMode("direct");
        let accumulated = "";

        await readSSEStream(
          res,
          (chunkText) => {
            accumulated += chunkText;
            setStreamingText(accumulated);
          },
          (fullText) => {
            // Append complete Nova message
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
            setMessages((prev) => [...prev, novaMsg]);
            setStreamingText("");
          },
          (errMsg) => {
            setError(errMsg);
            setStreamingText("");
          },
        );
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Failed to send message",
        );
      } finally {
        setSending(false);
      }
    },
    [inputValue, sending],
  );

  // Keyboard handling [3.8]
  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        void handleSend();
      }
    },
    [handleSend],
  );

  // [3.11] Wrap in PageShell
  return (
    <PageShell title="Chat" subtitle="Talk to Nova directly">
      <div className="flex flex-col h-[calc(100vh-12rem)]">
        {/* Error banner */}
        {error && (
          <div className="shrink-0 mb-3">
            <ErrorBanner
              message="Chat error"
              detail={error}
              onRetry={() => setError(null)}
            />
          </div>
        )}

        {/* Messages container [3.7] */}
        <div
          ref={scrollRef}
          className="flex-1 overflow-y-auto min-h-0"
        >
          {loading ? (
            <ChatSkeleton />
          ) : (
            <div className="flex flex-col gap-3 py-4">
              {messages.length === 0 && !sending && (
                <div className="flex flex-col items-center justify-center py-16 text-center">
                  <Bot size={32} className="text-ds-gray-700 mb-3" />
                  <p className="text-sm text-ds-gray-900">
                    No messages yet. Start a conversation with Nova.
                  </p>
                </div>
              )}

              {/* Message bubbles [3.4] */}
              {messages.map((msg) => (
                <MessageBubble key={msg.id} message={msg} />
              ))}

              {/* Streaming / typing indicator [3.6] */}
              {sending && !telegramPolling && (
                <StreamingBubble text={streamingText} />
              )}

              {/* Telegram polling indicator [3.9] */}
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

              {/* Scroll anchor */}
              <div ref={bottomRef} />
            </div>
          )}
        </div>

        {/* Input bar [3.8] */}
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
                className="w-full resize-none rounded-lg border border-ds-gray-400 bg-ds-gray-100 px-3.5 py-2.5 text-sm text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-none focus:border-ds-gray-700 transition-colors disabled:opacity-50"
                style={{ maxHeight: "96px" }}
              />
              {/* Transport mode indicator [3.10] */}
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
