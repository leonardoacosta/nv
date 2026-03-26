import { randomUUID } from "node:crypto";
import { Readable } from "node:stream";
import TelegramBot from "node-telegram-bot-api";

import { createLogger } from "../logger.js";
import type { Message } from "../types.js";

// ─── Exported Types ──────────────────────────────────────────────────────────

export interface SendMessageOptions {
  parseMode?: "HTML" | "Markdown" | "MarkdownV2";
  replyToMessageId?: number;
  keyboard?: TelegramBot.InlineKeyboardMarkup;
  disablePreview?: boolean;
}

export interface KeyboardButton {
  text: string;
  callbackData: string;
}

// ─── Keyboard Builders ───────────────────────────────────────────────────────

export function buildKeyboard(
  rows: KeyboardButton[][],
): TelegramBot.InlineKeyboardMarkup {
  return {
    inline_keyboard: rows.map((row) =>
      row.map((btn) => ({
        text: btn.text,
        callback_data: btn.callbackData,
      })),
    ),
  };
}

export function obligationKeyboard(
  obligationId: string,
): TelegramBot.InlineKeyboardMarkup {
  return buildKeyboard([
    [
      { text: "Approve", callbackData: `ob:approve:${obligationId}` },
      { text: "Snooze", callbackData: `ob:snooze:${obligationId}` },
      { text: "Dismiss", callbackData: `ob:dismiss:${obligationId}` },
    ],
  ]);
}

export function reminderKeyboard(
  reminderId: string,
): TelegramBot.InlineKeyboardMarkup {
  return buildKeyboard([
    [
      { text: "Done", callbackData: `reminder:done:${reminderId}` },
      { text: "Snooze 1h", callbackData: `reminder:snooze:1h:${reminderId}` },
      {
        text: "Snooze tomorrow",
        callbackData: `reminder:snooze:tomorrow:${reminderId}`,
      },
    ],
  ]);
}

// ─── Message Normalization (exported for testing) ────────────────────────────

export function normalizeUser(from: TelegramBot.User | undefined): {
  id: string;
  username?: string;
  firstName: string;
} {
  return {
    id: String(from?.id ?? 0),
    username: from?.username,
    firstName: from?.first_name ?? "Unknown",
  };
}

export function normalizeTextMessage(msg: TelegramBot.Message): Message {
  const chatId = String(msg.chat.id);
  const senderId = String(msg.from?.id ?? 0);
  const senderName = msg.from?.first_name ?? "Unknown";
  const text = msg.text ?? "";
  const timestamp = new Date(msg.date * 1000);

  return {
    id: randomUUID(),
    channel: "telegram",
    chatId,
    text,
    type: "text",
    from: normalizeUser(msg.from),
    timestamp,
    metadata: { messageId: msg.message_id },
    // Legacy fields
    threadId: undefined,
    senderId,
    senderName,
    content: text,
    receivedAt: timestamp,
  };
}

export async function normalizeVoiceMessage(
  msg: TelegramBot.Message,
  bot: TelegramBot,
): Promise<Message> {
  const chatId = String(msg.chat.id);
  const senderId = String(msg.from?.id ?? 0);
  const senderName = msg.from?.first_name ?? "Unknown";
  const timestamp = new Date(msg.date * 1000);
  const fileId = msg.voice?.file_id ?? "";

  let fileUrl: string | undefined;
  if (fileId) {
    try {
      fileUrl = await bot.getFileLink(fileId);
    } catch {
      // Non-fatal — STT layer will handle missing fileUrl
    }
  }

  return {
    id: randomUUID(),
    channel: "telegram",
    chatId,
    text: "",
    type: "voice",
    from: normalizeUser(msg.from),
    timestamp,
    metadata: {
      messageId: msg.message_id,
      fileId,
      ...(fileUrl !== undefined ? { fileUrl } : {}),
    },
    // Legacy fields
    threadId: undefined,
    senderId,
    senderName,
    content: "",
    receivedAt: timestamp,
  };
}

export function normalizePhotoMessage(msg: TelegramBot.Message): Message {
  const chatId = String(msg.chat.id);
  const senderId = String(msg.from?.id ?? 0);
  const senderName = msg.from?.first_name ?? "Unknown";
  const text = msg.caption ?? "";
  const timestamp = new Date(msg.date * 1000);
  const fileIds = (msg.photo ?? []).map((p) => p.file_id);

  return {
    id: randomUUID(),
    channel: "telegram",
    chatId,
    text,
    type: "photo",
    from: normalizeUser(msg.from),
    timestamp,
    metadata: { messageId: msg.message_id, fileIds },
    // Legacy fields
    threadId: undefined,
    senderId,
    senderName,
    content: text,
    receivedAt: timestamp,
  };
}

export function normalizeCallbackQuery(query: TelegramBot.CallbackQuery): Message {
  const chatId = String(query.message?.chat.id ?? 0);
  const senderId = String(query.from.id);
  const senderName = query.from.first_name;
  const text = query.data ?? "";
  const timestamp = query.message
    ? new Date(query.message.date * 1000)
    : new Date();

  return {
    id: randomUUID(),
    channel: "telegram",
    chatId,
    text,
    type: "callback",
    from: normalizeUser(query.from),
    timestamp,
    metadata: {
      callbackQueryId: query.id,
      originalMessageId: query.message?.message_id,
    },
    // Legacy fields
    threadId: undefined,
    senderId,
    senderName,
    content: text,
    receivedAt: timestamp,
  };
}

// ─── TelegramAdapter ─────────────────────────────────────────────────────────

export class TelegramAdapter {
  private bot: TelegramBot;
  private onMessageCallback: ((msg: Message) => void) | null = null;
  private readonly log = createLogger("telegram-adapter");

  constructor(token: string, polling: boolean = true) {
    this.bot = new TelegramBot(token, { polling });

    this.registerCommands();
    this.registerCommandHandlers();
  }

  // ── Public API ─────────────────────────────────────────────────────────────

  onMessage(callback: (msg: Message) => void): void {
    this.onMessageCallback = callback;

    // Handle plain text messages (non-command)
    this.bot.on("message", (msg) => {
      // Skip messages that are commands — handled by onText handlers
      const text = msg.text ?? "";
      if (text.startsWith("/")) return;

      void this.handleInboundMessage(msg);
    });

    // Handle callback queries (inline keyboard button presses)
    this.bot.on("callback_query", (query) => {
      void this.handleCallbackQuery(query);
    });
  }

  async sendMessage(
    chatId: number | string,
    text: string,
    options?: SendMessageOptions,
  ): Promise<TelegramBot.Message> {
    const sendOptions: TelegramBot.SendMessageOptions = {
      parse_mode: options?.parseMode ?? "HTML",
      ...(options?.replyToMessageId !== undefined
        ? { reply_to_message_id: options.replyToMessageId }
        : {}),
      ...(options?.keyboard !== undefined
        ? { reply_markup: options.keyboard }
        : {}),
      ...(options?.disablePreview === true
        ? { disable_web_page_preview: true }
        : {}),
    };

    return this.bot.sendMessage(chatId, text, sendOptions);
  }

  async sendVoice(
    chatId: number | string,
    buffer: Buffer,
  ): Promise<TelegramBot.Message> {
    const stream = Readable.from(buffer);
    return this.bot.sendVoice(chatId, stream);
  }

  async editMessage(
    chatId: number | string,
    messageId: number,
    text: string,
  ): Promise<void> {
    await this.bot.editMessageText(text, {
      chat_id: chatId,
      message_id: messageId,
      parse_mode: "HTML",
    });
  }

  async deleteMessage(
    chatId: number | string,
    messageId: number,
  ): Promise<void> {
    await this.bot.deleteMessage(chatId, messageId);
  }

  async answerCallbackQuery(
    callbackId: string,
    text?: string,
  ): Promise<void> {
    await this.bot.answerCallbackQuery(callbackId, { text });
  }

  async sendChatAction(
    chatId: number | string,
    action: TelegramBot.ChatAction,
  ): Promise<void> {
    await this.bot.sendChatAction(chatId, action);
  }

  stop(): void {
    void this.bot.stopPolling();
  }

  // ── Private Helpers ────────────────────────────────────────────────────────

  private async handleInboundMessage(msg: TelegramBot.Message): Promise<void> {
    if (!this.onMessageCallback) return;

    let normalized: Message;

    if (msg.voice) {
      normalized = await normalizeVoiceMessage(msg, this.bot);
    } else if (msg.photo) {
      normalized = normalizePhotoMessage(msg);
    } else {
      normalized = normalizeTextMessage(msg);
    }

    this.onMessageCallback(normalized);
  }

  private async handleCallbackQuery(
    query: TelegramBot.CallbackQuery,
  ): Promise<void> {
    // Acknowledge immediately to dismiss the Telegram spinner (expires after 60s)
    await this.answerCallbackQuery(query.id);

    if (!this.onMessageCallback) return;
    this.onMessageCallback(normalizeCallbackQuery(query));
  }

  private registerCommands(): void {
    this.bot
      .setMyCommands([
        { command: "start", description: "Start Nova and show status" },
        { command: "help", description: "Show available commands" },
        { command: "ob", description: "List active obligations" },
        { command: "diary", description: "Show today's interaction summary" },
        { command: "status", description: "Nova daemon status" },
      ])
      .catch((err: unknown) => {
        this.log.error({ err }, "Failed to register bot commands");
      });
  }

  private registerCommandHandlers(): void {
    const commands = ["/start", "/help", "/ob", "/diary", "/status"] as const;

    for (const command of commands) {
      this.bot.onText(new RegExp(`^\\${command}(@\\S+)?$`), (msg) => {
        if (!this.onMessageCallback) return;
        const normalized = normalizeTextMessage(msg);
        this.onMessageCallback({ ...normalized, text: command, content: command });
      });
    }
  }
}

export default TelegramAdapter;
