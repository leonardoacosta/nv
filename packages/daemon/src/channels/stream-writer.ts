import type { TelegramAdapter } from "./telegram.js";
import { humanizeToolName } from "./tool-names.js";
import { createLogger } from "../logger.js";

const log = createLogger("stream-writer");

/** Maximum Telegram message length. */
const MAX_LEN = 4096;

/** Minimum ms between sendDraft calls. */
const DRAFT_THROTTLE_MS = 300;

/** Minimum ms between editMessageText fallback calls. */
const EDIT_THROTTLE_MS = 1000;

/**
 * Manages the sendMessageDraft lifecycle for a single agent response.
 * Falls back to editMessageText on a placeholder if drafts are unavailable.
 */
export class TelegramStreamWriter {
  private readonly adapter: TelegramAdapter;
  private readonly chatId: string;
  private readonly draftId: number;

  private currentText = "";
  private readonly activeTools = new Map<string, { name: string; humanized: string; startedAt: number }>();
  private lastFlushAt = 0;
  private flushTimer: ReturnType<typeof setTimeout> | null = null;

  /** null = untested, true = supported, false = unsupported */
  private draftSupported: boolean | null = null;
  private fallbackMessageId: number | null = null;

  constructor(adapter: TelegramAdapter, chatId: string) {
    this.adapter = adapter;
    this.chatId = chatId;
    // Random non-zero integer for draft identification
    this.draftId = Math.floor(Math.random() * 2_147_483_646) + 1;
  }

  // ── Event Handlers ──────────────────────────────────────────────────────────

  onTextDelta(text: string): void {
    this.currentText += text;
    this.scheduleFlush();
  }

  onToolStart(name: string, callId: string): void {
    const humanized = humanizeToolName(name);
    this.activeTools.set(callId, { name, humanized, startedAt: Date.now() });
    this.scheduleFlush();
  }

  onToolDone(name: string, callId: string, durationMs: number): void {
    const info = this.activeTools.get(callId);
    this.activeTools.delete(callId);

    // Log completion (the status line will update on next flush)
    const label = info?.humanized ?? humanizeToolName(name);
    const secs = Math.round(durationMs / 1000);
    log.debug({ tool: name, callId, durationMs }, `${label} completed (${secs}s)`);

    this.scheduleFlush();
  }

  async finalize(fullText: string): Promise<void> {
    // Cancel any pending flush
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }

    // Split into 4096-char chunks and send as final messages with Markdown
    const chunks = splitMessage(fullText);

    for (const chunk of chunks) {
      try {
        await this.adapter.sendMessage(this.chatId, chunk, {
          parseMode: "Markdown",
          disablePreview: true,
        });
      } catch {
        // Markdown failed -- strip formatting and send plain
        try {
          const plain = stripMarkdown(chunk);
          await this.adapter.sendMessage(this.chatId, plain);
        } catch (sendErr: unknown) {
          log.warn({ chatId: this.chatId, err: sendErr }, "finalize sendMessage chunk failed");
        }
      }
    }

    // Clean up the fallback placeholder if one exists
    if (this.fallbackMessageId !== null) {
      try {
        await this.adapter.deleteMessage(this.chatId, this.fallbackMessageId);
      } catch {
        // Non-fatal -- placeholder may already be gone
      }
      this.fallbackMessageId = null;
    }
  }

  async abort(error: string): Promise<void> {
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }

    try {
      await this.adapter.sendMessage(this.chatId, error);
    } catch (sendErr: unknown) {
      log.warn({ chatId: this.chatId, err: sendErr }, "abort sendMessage failed");
    }

    // Clean up fallback placeholder
    if (this.fallbackMessageId !== null) {
      try {
        await this.adapter.deleteMessage(this.chatId, this.fallbackMessageId);
      } catch {
        // Non-fatal
      }
      this.fallbackMessageId = null;
    }
  }

  // ── Internal ────────────────────────────────────────────────────────────────

  private scheduleFlush(): void {
    if (this.flushTimer) return; // Already scheduled

    const throttleMs = this.draftSupported === false ? EDIT_THROTTLE_MS : DRAFT_THROTTLE_MS;
    const elapsed = Date.now() - this.lastFlushAt;
    const delay = Math.max(0, throttleMs - elapsed);

    this.flushTimer = setTimeout(() => {
      this.flushTimer = null;
      void this.flush();
    }, delay);
  }

  private async flush(): Promise<void> {
    const throttleMs = this.draftSupported === false ? EDIT_THROTTLE_MS : DRAFT_THROTTLE_MS;
    const now = Date.now();
    if (now - this.lastFlushAt < throttleMs) return;

    const displayText = this.buildDisplayText();
    if (!displayText) return;

    // Truncate to Telegram limit
    const truncated = displayText.length > MAX_LEN
      ? displayText.slice(0, MAX_LEN - 3) + "..."
      : displayText;

    this.lastFlushAt = Date.now();

    // Try draft API first (if not known-unsupported)
    if (this.draftSupported !== false) {
      const ok = await this.adapter.sendDraft(this.chatId, this.draftId, truncated);
      if (this.draftSupported === null) {
        this.draftSupported = ok;
      }
      if (ok) return;
      // Draft failed -- fall through to edit
    }

    // Fallback: editMessageText on a single placeholder
    try {
      if (this.fallbackMessageId === null) {
        const placeholderMsg = await this.adapter.sendMessage(this.chatId, "Thinking...");
        this.fallbackMessageId = placeholderMsg.message_id;
      }
      await this.adapter.editMessage(this.chatId, this.fallbackMessageId, truncated);
    } catch (editErr: unknown) {
      log.debug({ chatId: this.chatId, err: editErr }, "fallback editMessage failed");
    }
  }

  private buildDisplayText(): string {
    const parts: string[] = [];

    // Status line for active tools
    if (this.activeTools.size > 0) {
      const toolLines: string[] = [];
      for (const [, info] of this.activeTools) {
        const elapsed = Math.round((Date.now() - info.startedAt) / 1000);
        toolLines.push(`${info.humanized} (${elapsed}s)`);
      }
      parts.push(toolLines.join(" | "));
    }

    // Accumulated text with incomplete Markdown stripped
    const safeText = stripIncompleteMarkdown(this.currentText);
    if (safeText) {
      parts.push(safeText);
    }

    return parts.join("\n\n");
  }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Strip incomplete Markdown delimiters at the tail of streaming text
 * to avoid rendering glitches. Only strips unclosed trailing delimiters.
 */
function stripIncompleteMarkdown(text: string): string {
  let result = text;

  // Strip trailing unclosed code fence (``` without closing ```)
  const fenceCount = (result.match(/```/g) ?? []).length;
  if (fenceCount % 2 !== 0) {
    const lastFence = result.lastIndexOf("```");
    result = result.slice(0, lastFence);
  }

  // Strip trailing unclosed bold (**)
  const boldCount = (result.match(/\*\*/g) ?? []).length;
  if (boldCount % 2 !== 0) {
    const lastBold = result.lastIndexOf("**");
    result = result.slice(0, lastBold);
  }

  // Strip trailing unclosed inline code (`)
  const backtickCount = (result.match(/(?<!`)`(?!`)/g) ?? []).length;
  if (backtickCount % 2 !== 0) {
    const lastTick = result.lastIndexOf("`");
    result = result.slice(0, lastTick);
  }

  return result.trimEnd();
}

/** Strip Markdown formatting for plain-text fallback. */
function stripMarkdown(text: string): string {
  return text
    .replace(/\*\*(.+?)\*\*/g, "$1")
    .replace(/\*(.+?)\*/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/```[\s\S]*?```/g, (m) =>
      m.replace(/```\w*\n?/g, "").replace(/```/g, ""),
    );
}

/** Split text into chunks at 4096-char boundaries, preferring newline splits. */
function splitMessage(text: string): string[] {
  const chunks: string[] = [];
  let remaining = text;

  while (remaining.length > 0) {
    if (remaining.length <= MAX_LEN) {
      chunks.push(remaining);
      break;
    }
    // Split at last newline before limit to avoid mid-word breaks
    let splitAt = remaining.lastIndexOf("\n", MAX_LEN);
    if (splitAt < MAX_LEN * 0.5) splitAt = MAX_LEN; // no good newline, hard split
    chunks.push(remaining.slice(0, splitAt));
    remaining = remaining.slice(splitAt).replace(/^\n/, "");
  }

  return chunks;
}
