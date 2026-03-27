import { randomUUID } from "node:crypto";
import { Readable } from "node:stream";
import TelegramBot from "node-telegram-bot-api";

import { createLogger } from "../logger.js";
import type { Message } from "../types.js";
import { buildDiaryReply } from "../telegram/commands/diary.js";
import { buildHelpReply } from "../telegram/commands/help.js";
import { buildMemoryReply } from "../telegram/commands/memory.js";
import { buildSearchReply } from "../telegram/commands/search.js";
import { buildTeamsReply } from "../telegram/commands/teams.js";
import { buildCalendarReply } from "../telegram/commands/calendar.js";
import { buildDiscordReply } from "../telegram/commands/discord.js";
import { buildHealthReply } from "../telegram/commands/health.js";
import { buildRemindReply } from "../telegram/commands/remind.js";
import { buildObReply } from "../telegram/commands/ob.js";
import { buildContactsReply } from "../telegram/commands/contacts.js";
import { buildSoulReply } from "../telegram/commands/soul.js";
import { buildStatusReply } from "../telegram/commands/status.js";
import { buildAzReply } from "../telegram/commands/az.js";
import { buildMailReply } from "../telegram/commands/mail.js";
import { buildPimReply } from "../telegram/commands/pim.js";
import { buildAdoReply } from "../telegram/commands/ado.js";
import { buildDreamReply, buildDreamStatusReply } from "../telegram/commands/dream.js";
import { buildSnapshotReply } from "../telegram/commands/snapshot.js";
import { buildDigestReply } from "../telegram/commands/digest.js";
import { buildStartKeyboard } from "../telegram/commands/start.js";
import { buildToolsKeyboard } from "../telegram/commands/tools.js";
import { buildObligationReply } from "../telegram/commands/obligation.js";
import type { ObligationStore } from "../features/obligations/store.js";
import { ObligationStatus } from "../features/obligations/types.js";

// ─── HTML Escape ─────────────────────────────────────────────────────────────

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

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
  private readonly token: string;
  private onMessageCallback: ((msg: Message) => void) | null = null;
  private readonly log = createLogger("telegram-adapter");
  private obligationStore: ObligationStore | null = null;

  /** Cached draft API availability: null = untested, false = unsupported */
  private draftApiAvailable: boolean | null = null;

  constructor(token: string, polling: boolean = true) {
    this.token = token;
    this.bot = new TelegramBot(token, { polling });

    this.registerCommands();
    this.registerCommandHandlers();
  }

  /**
   * Sets the obligation store for the /obligation command.
   * Must be called after the store is initialized.
   */
  setObligationStore(store: ObligationStore): void {
    this.obligationStore = store;
  }

  // ── Public API ─────────────────────────────────────────────────────────────

  onMessage(callback: (msg: Message) => void): void {
    this.onMessageCallback = callback;

    // Handle plain text messages (non-command)
    this.bot.on("message", (msg) => {
      const text = msg.text ?? "";
      this.log.info(
        {
          chatId: msg.chat.id,
          messageId: msg.message_id,
          textLength: text.length,
          textPreview: text.slice(0, 60),
          hasEntities: (msg.entities?.length ?? 0) > 0,
        },
        "Telegram raw message received",
      );

      // Skip messages that are Telegram bot commands — handled by onText handlers.
      // A command is a single slash-word like /start or /diary, NOT a file path
      // like /home/user/... or a message that merely begins with /.
      if (/^\/[a-z_]+(\s|$|@)/i.test(text)) {
        this.log.debug({ text: text.slice(0, 30) }, "Skipped — matched command pattern");
        return;
      }

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
      ...(options?.parseMode !== undefined ? { parse_mode: options.parseMode } : {}),
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

  /**
   * Send a draft message via Telegram Bot API sendMessageDraft (raw HTTP).
   * Returns true on success, false on failure. Caches availability --
   * if the first call returns 404/method-not-found, future calls short-circuit.
   */
  async sendDraft(
    chatId: string | number,
    draftId: number,
    text: string,
  ): Promise<boolean> {
    // Short-circuit if already known unsupported
    if (this.draftApiAvailable === false) return false;

    try {
      const res = await fetch(
        `https://api.telegram.org/bot${this.token}/sendMessageDraft`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ chat_id: chatId, draft_id: draftId, text }),
        },
      );

      if (res.ok) {
        this.draftApiAvailable = true;
        return true;
      }

      // Check for method-not-found / 404
      if (res.status === 404) {
        this.draftApiAvailable = false;
        this.log.debug("sendMessageDraft returned 404 -- draft API unavailable");
        return false;
      }

      // Try to detect "method not found" in response body
      try {
        const body = (await res.json()) as { description?: string };
        if (body.description?.toLowerCase().includes("method not found")) {
          this.draftApiAvailable = false;
          this.log.debug("sendMessageDraft: method not found -- draft API unavailable");
          return false;
        }
      } catch {
        // Ignore JSON parse errors
      }

      this.log.debug({ status: res.status }, "sendMessageDraft failed");
      return false;
    } catch (err: unknown) {
      this.log.debug({ err }, "sendMessageDraft fetch error");
      // Network error on first call -- mark unavailable
      if (this.draftApiAvailable === null) {
        this.draftApiAvailable = false;
      }
      return false;
    }
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

    const data = query.data ?? "";
    const chatId = String(query.message?.chat.id ?? 0);

    // Route inline keyboard cmd: callbacks to direct handlers
    if (data.startsWith("cmd:")) {
      const cmd = data.slice(4);
      const [command, ...args] = cmd.split(" ");
      const argsText = args.join(" ") || undefined;

      switch (command) {
        case "snapshot":
          return void this.handleDirectCommand(chatId, () => buildSnapshotReply());
        case "calendar":
          return void this.handleDirectCommand(chatId, () => buildCalendarReply());
        case "mail": {
          let subcommand: string | undefined;
          let arg: string | undefined;
          if (argsText) {
            const spaceIdx = argsText.indexOf(" ");
            if (spaceIdx === -1) {
              subcommand = argsText;
            } else {
              subcommand = argsText.slice(0, spaceIdx);
              arg = argsText.slice(spaceIdx + 1).trim();
            }
          }
          return void this.handleDirectCommand(chatId, () => buildMailReply(subcommand, arg));
        }
        case "ob":
          return void this.handleDirectCommand(chatId, () => buildObReply());
        case "memory":
          return void this.handleDirectCommand(chatId, () => buildMemoryReply(argsText));
        case "health":
          return void this.handleDirectCommand(chatId, () => buildHealthReply());
        case "teams":
          return void this.handleDirectCommand(chatId, () => buildTeamsReply());
        case "ado": {
          let subcommand: string | undefined;
          let arg: string | undefined;
          if (argsText) {
            const spaceIdx = argsText.indexOf(" ");
            if (spaceIdx === -1) {
              subcommand = argsText;
            } else {
              subcommand = argsText.slice(0, spaceIdx);
              arg = argsText.slice(spaceIdx + 1).trim();
            }
          }
          return void this.handleDirectCommand(chatId, () => buildAdoReply(subcommand, arg));
        }
        case "pim":
          return void this.handleDirectCommand(chatId, () => buildPimReply(argsText));
        case "dream":
          return void this.handleDirectCommand(chatId, () => buildDreamReply());
        case "digest":
          return void this.handleDirectCommand(chatId, () => buildDigestReply(argsText));
        case "az":
          return void this.handleDirectCommand(chatId, () => buildAzReply(argsText));
        case "discord":
          return void this.handleDirectCommand(chatId, () => buildDiscordReply());
        case "contacts":
          return void this.handleDirectCommand(chatId, () => buildContactsReply());
        case "soul":
          return void this.handleDirectCommand(chatId, () => buildSoulReply());
        default:
          break;
      }
    }

    // Existing obligation/reminder callback handling
    if (!this.onMessageCallback) return;
    this.onMessageCallback(normalizeCallbackQuery(query));
  }

  private registerCommands(): void {
    this.bot
      .setMyCommands([
        { command: "start", description: "Dashboard with quick actions" },
        { command: "snapshot", description: "Briefing snapshot + calendar + mail" },
        { command: "tools", description: "Teams, ADO, PIM, Azure, Discord" },
        { command: "dream", description: "Memory consolidation" },
        { command: "status", description: "Fleet health + daemon status" },
        { command: "remind", description: "Set a reminder (/remind msg time)" },
        { command: "obligation", description: "Create an obligation manually" },
        { command: "help", description: "All commands reference" },
      ])
      .catch((err: unknown) => {
        this.log.error({ err }, "Failed to register bot commands");
      });
  }

  private registerCommandHandlers(): void {
    // ── Direct handlers (fast — call fleet HTTP or DB, no agent) ────────────

    // /diary [date] — interaction summary
    this.bot.onText(/^\/diary(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const dateArg = match?.[3]?.trim();
      void this.handleDiaryCommand(chatId, dateArg);
    });

    // /help — command list
    this.bot.onText(/^\/help(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => Promise.resolve(buildHelpReply()));
    });

    // /memory [topic] — read memory
    this.bot.onText(/^\/memory(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const topicArg = match?.[3]?.trim();
      void this.handleDirectCommand(chatId, () => buildMemoryReply(topicArg));
    });

    // /search [query] — search messages
    this.bot.onText(/^\/search(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const query = match?.[3]?.trim();
      void this.handleDirectCommand(chatId, () => buildSearchReply(query));
    });

    // /teams — list Teams chats
    this.bot.onText(/^\/teams(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildTeamsReply());
    });

    // /calendar — today's events
    this.bot.onText(/^\/calendar(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildCalendarReply());
    });

    // /discord — list Discord servers
    this.bot.onText(/^\/discord(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildDiscordReply());
    });

    // /health — fleet service health
    this.bot.onText(/^\/health(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildHealthReply());
    });

    // /remind [message] [time] — set a reminder
    this.bot.onText(/^\/remind(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const argsText = match?.[3]?.trim();
      void this.handleDirectCommand(chatId, () => buildRemindReply(argsText));
    });

    // /ob — active obligations
    this.bot.onText(/^\/ob(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildObReply());
    });

    // /obligation <action> [p1|p2|p3] — create obligation manually
    this.bot.onText(/^\/obligation(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const argsText = match?.[3]?.trim();
      if (!this.obligationStore) {
        void this.sendMessage(chatId, "Obligation store not initialized.");
        return;
      }
      const store = this.obligationStore;
      void this.handleDirectCommand(chatId, () =>
        buildObligationReply(
          argsText,
          (input) => store.create(input),
          ObligationStatus.Open,
        ),
      );
    });

    // /contacts — list contacts
    this.bot.onText(/^\/contacts(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildContactsReply());
    });

    // /soul — Nova's personality
    this.bot.onText(/^\/soul(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildSoulReply());
    });

    // /status — daemon + fleet status
    this.bot.onText(/^\/status(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildStatusReply());
    });

    // /az [command] — run Azure CLI command
    this.bot.onText(/^\/az(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const command = match?.[3]?.trim();
      void this.handleDirectCommand(chatId, () => buildAzReply(command));
    });

    // /mail [subcommand] [arg] — Outlook email
    this.bot.onText(/^\/mail(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const argsText = match?.[3]?.trim();
      let subcommand: string | undefined;
      let arg: string | undefined;
      if (argsText) {
        const spaceIdx = argsText.indexOf(" ");
        if (spaceIdx === -1) {
          subcommand = argsText;
        } else {
          subcommand = argsText.slice(0, spaceIdx);
          arg = argsText.slice(spaceIdx + 1).trim();
        }
      }
      void this.handleDirectCommand(chatId, () => buildMailReply(subcommand, arg));
    });

    // /pim [args] — PIM role status and activation
    this.bot.onText(/^\/pim(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const argsText = match?.[3]?.trim();
      void this.handleDirectCommand(chatId, () => buildPimReply(argsText));
    });

    // /ado [subcommand] [arg] — Azure DevOps
    this.bot.onText(/^\/ado(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const argsText = match?.[3]?.trim();
      let subcommand: string | undefined;
      let arg: string | undefined;
      if (argsText) {
        const spaceIdx = argsText.indexOf(" ");
        if (spaceIdx === -1) {
          subcommand = argsText;
        } else {
          subcommand = argsText.slice(0, spaceIdx);
          arg = argsText.slice(spaceIdx + 1).trim();
        }
      }
      void this.handleDirectCommand(chatId, () => buildAdoReply(subcommand, arg));
    });

    // /dream [status] — memory consolidation
    this.bot.onText(/^\/dream(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const subArg = match?.[3]?.trim();
      if (subArg === "status") {
        void this.handleDirectCommand(chatId, () => buildDreamStatusReply());
      } else {
        void this.handleDirectCommand(chatId, () => buildDreamReply());
      }
    });

    // /start — inline keyboard dashboard
    this.bot.onText(/^\/start(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.sendMessage(chatId, "Nova Dashboard", {
        keyboard: buildStartKeyboard(),
      });
    });

    // /snapshot — briefing snapshot (calendar + mail + obligations)
    this.bot.onText(/^\/snapshot(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.handleDirectCommand(chatId, () => buildSnapshotReply());
    });

    // /digest [weekly] — proactive digest
    this.bot.onText(/^\/digest(@\S+)?(\s+(.+))?$/, (msg, match) => {
      const chatId = String(msg.chat.id);
      const subcommand = match?.[3]?.trim();
      void this.handleDirectCommand(chatId, () => buildDigestReply(subcommand));
    });

    // /tools — tool keyboard menu
    this.bot.onText(/^\/tools(@\S+)?$/, (msg) => {
      const chatId = String(msg.chat.id);
      void this.sendMessage(chatId, "Tools", {
        keyboard: buildToolsKeyboard(),
      });
    });
  }

  private async handleDirectCommand(
    chatId: string,
    handler: () => Promise<string>,
  ): Promise<void> {
    try {
      const text = await handler();
      await this.sendMessage(chatId, text, { disablePreview: true });
    } catch (err: unknown) {
      this.log.error({ err }, "Command handler failed");
      const serviceName =
        err instanceof Error && "status" in err
          ? "Service unavailable"
          : "Something went wrong";
      await this.sendMessage(chatId, `${serviceName}. Please try again.`);
    }
  }

  private async handleDiaryCommand(chatId: string, dateArg?: string): Promise<void> {
    try {
      const text = await buildDiaryReply(dateArg);
      await this.sendMessage(chatId, `<pre>${escapeHtml(text)}</pre>`, {
        parseMode: "HTML",
        disablePreview: true,
      });
    } catch (err: unknown) {
      this.log.error({ err }, "Failed to handle /diary command");
      await this.sendMessage(chatId, "Failed to retrieve diary entries. Please try again.");
    }
  }
}

export default TelegramAdapter;
