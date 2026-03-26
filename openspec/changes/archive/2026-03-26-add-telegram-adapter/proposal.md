# Proposal: Add Telegram Channel Adapter

## Change ID
`add-telegram-adapter`

## Summary

Implement `src/channels/telegram.ts` — a `TelegramAdapter` class using `node-telegram-bot-api`
that normalizes inbound updates (text, voice, photo, callback_query) into the Nova `Message` type,
exposes send/voice/edit/delete/action primitives, and registers bot commands. This is the primary
user-facing channel for Nova.

## Context
- Depends on: `scaffold-ts-daemon` (TypeScript daemon project structure, `src/types.ts` defines `Message` type, `Config` type with `telegram.token`)
- Stack: TypeScript (Node.js), `node-telegram-bot-api`, `@types/node-telegram-bot-api`
- Primary channel: Telegram is Leo's exclusive interface to Nova — all commands, confirmations, and
  digests flow through this adapter

## Motivation

The TypeScript daemon needs a fully-typed Telegram adapter as its primary I/O channel. The adapter
must handle the full range of Telegram update types (text, voice, photos, callback queries), provide
all outbound primitives the Nova agent needs (send, voice, edit, delete, typing indicator, inline
keyboards), and define the bot commands that expose Nova's capabilities to the user.

## Requirements

### Req-1: TelegramAdapter Class

Create `src/channels/telegram.ts` with a `TelegramAdapter` class:

```typescript
export class TelegramAdapter {
  private bot: TelegramBot;
  private onMessageCallback: ((msg: Message) => void) | null = null;

  constructor(token: string, polling: boolean = true) {
    this.bot = new TelegramBot(token, { polling });
  }

  onMessage(callback: (msg: Message) => void): void;
  async sendMessage(chatId: number | string, text: string, options?: SendMessageOptions): Promise<TelegramBot.Message>;
  async sendVoice(chatId: number | string, buffer: Buffer): Promise<TelegramBot.Message>;
  async editMessage(chatId: number | string, messageId: number, text: string): Promise<void>;
  async deleteMessage(chatId: number | string, messageId: number): Promise<void>;
  async answerCallbackQuery(callbackId: string, text?: string): Promise<void>;
  async sendChatAction(chatId: number | string, action: TelegramBot.ChatAction): Promise<void>;
  stop(): void;
}
```

- Constructor: accepts `token` from config and a `polling` flag (default `true`)
- `sendMessage` uses `parse_mode: 'HTML'` by default
- `sendVoice` accepts a `Buffer`, wraps it as a readable stream before passing to bot API
- `editMessage` calls `bot.editMessageText` with `{ chat_id, message_id }`
- `answerCallbackQuery` acknowledges button presses to dismiss Telegram spinner

### Req-2: Message Normalization

Normalize each Telegram `update` type into the Nova `Message` type from `src/types.ts`:

```typescript
// Nova Message type (from scaffold-ts-daemon)
interface Message {
  id: string;
  channel: 'telegram';
  chatId: string;
  text: string;
  type: 'text' | 'voice' | 'photo' | 'callback';
  from: {
    id: string;
    username?: string;
    firstName: string;
  };
  timestamp: Date;
  metadata: Record<string, unknown>;
}
```

Mapping rules:
- **text message**: `type: 'text'`, `text` from `msg.text`, `metadata.messageId`
- **voice message**: `type: 'voice'`, `text: ''` (placeholder), `metadata.fileId` for STT processing, `metadata.messageId`
- **photo message**: `type: 'photo'`, `text: msg.caption ?? ''`, `metadata.fileIds` (array of all photo sizes), `metadata.messageId`
- **callback_query**: `type: 'callback'`, `text` from `query.data`, `metadata.callbackQueryId`, `metadata.originalMessageId`

### Req-3: Voice Message Handling

When a voice message is received:
1. Extract `msg.voice.file_id` from the update
2. Call `bot.getFileLink(fileId)` to get a download URL
3. Return a `Message` with `type: 'voice'`, empty `text`, and `metadata.fileUrl` set
4. The STT layer (outside this spec) will consume `metadata.fileUrl` and populate `text`

This is a placeholder — no actual STT processing in this spec. The adapter's job is to extract the
file reference and forward it through the normalized `Message`.

### Req-4: Inline Keyboard Builder

Export a `buildKeyboard` helper that constructs `InlineKeyboardMarkup`:

```typescript
export interface KeyboardButton {
  text: string;
  callbackData: string;
}

export function buildKeyboard(rows: KeyboardButton[][]): TelegramBot.InlineKeyboardMarkup;

// Convenience builders
export function obligationKeyboard(obligationId: string): TelegramBot.InlineKeyboardMarkup;
export function reminderKeyboard(reminderId: string): TelegramBot.InlineKeyboardMarkup;
```

- `buildKeyboard`: maps `KeyboardButton[][]` to Telegram's `inline_keyboard` format
- `obligationKeyboard`: builds `[Approve | Snooze | Dismiss]` row with `callback_data: 'ob:approve:{id}'`, `'ob:snooze:{id}'`, `'ob:dismiss:{id}'`
- `reminderKeyboard`: builds `[Done | Snooze 1h | Snooze tomorrow]` row with `callback_data: 'reminder:done:{id}'`, `'reminder:snooze:1h:{id}'`, `'reminder:snooze:tomorrow:{id}'`

### Req-5: Bot Commands Registration

Register bot commands on startup using `bot.setMyCommands`:

| Command | Description |
|---------|-------------|
| `/start` | Start Nova and show status |
| `/help` | Show available commands |
| `/ob` | List active obligations |
| `/diary` | Show today's interaction summary |
| `/status` | Nova daemon status |

Implement handlers for each command that call `onMessageCallback` with a synthetic `Message`:
- `type: 'text'`
- `text` set to the command string (e.g. `/ob`)
- All other fields normalized from the Telegram message

### Req-6: SendMessage Options

Define and export `SendMessageOptions`:

```typescript
export interface SendMessageOptions {
  parseMode?: 'HTML' | 'Markdown' | 'MarkdownV2';
  replyToMessageId?: number;
  keyboard?: TelegramBot.InlineKeyboardMarkup;
  disablePreview?: boolean;
}
```

`sendMessage` translates these to the `node-telegram-bot-api` send options format.

### Req-7: Export Interface

`src/channels/telegram.ts` must export:
- `TelegramAdapter` (default and named)
- `SendMessageOptions`
- `KeyboardButton`
- `buildKeyboard`
- `obligationKeyboard`
- `reminderKeyboard`

## Scope
- **IN**: `TelegramAdapter` class, message normalization, voice file extraction, keyboard builder, bot command registration, `SendMessageOptions`
- **OUT**: STT audio download/transcription, actual obligation/reminder logic, persistence, multi-chat support (single `chatId` from config only), webhook mode (polling only)

## Impact

| Area | Change |
|------|--------|
| `src/channels/telegram.ts` | New file — full `TelegramAdapter` implementation |
| `package.json` | Add `node-telegram-bot-api`, `@types/node-telegram-bot-api` dependencies |
| `src/types.ts` | Confirm `Message` type is compatible; extend if needed |

## Risks

| Risk | Mitigation |
|------|-----------|
| `node-telegram-bot-api` polling conflicts with multiple instances | Single adapter instance per daemon process; polling mode documented |
| Buffer-to-stream conversion for voice | Use `stream.Readable.from(buffer)` — standard Node.js pattern |
| Callback query IDs expire after 60s | `answerCallbackQuery` called immediately in the update handler before forwarding |
| Bot commands not visible in Telegram UI | Call `setMyCommands` in constructor after bot init; log on failure but don't crash |
