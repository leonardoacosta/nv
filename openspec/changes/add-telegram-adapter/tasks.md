# Implementation Tasks

<!-- beads:epic:nv-pxa4 -->

## Setup Batch

- [ ] [1.1] [P-1] Add `node-telegram-bot-api` and `@types/node-telegram-bot-api` to `package.json` dependencies; run `npm install` (or equivalent package manager for the ts-daemon project) [owner:api-engineer]

## Implementation Batch

- [ ] [2.1] [P-1] Create `src/channels/telegram.ts` — scaffold `TelegramAdapter` class with constructor accepting `token: string` and `polling: boolean = true`; instantiate `TelegramBot` with polling options [owner:api-engineer]
- [ ] [2.2] [P-1] Implement `onMessage(callback)` method — register handlers on `bot.on('message', ...)` and `bot.on('callback_query', ...)` that normalize updates and call the callback [owner:api-engineer]
- [ ] [2.3] [P-1] Implement message normalization — text messages map to `Message` with `type: 'text'`; voice with `type: 'voice'` + `metadata.fileId` + call `bot.getFileLink` for `metadata.fileUrl`; photo with `type: 'photo'` + `metadata.fileIds`; callback_query with `type: 'callback'` + `metadata.callbackQueryId` + `metadata.originalMessageId` [owner:api-engineer]
- [ ] [2.4] [P-1] Implement `sendMessage(chatId, text, options?)` — calls `bot.sendMessage` with `parse_mode: 'HTML'` default; maps `SendMessageOptions` fields to bot API format (replyToMessageId, reply_markup, disable_web_page_preview) [owner:api-engineer]
- [ ] [2.5] [P-1] Implement `sendVoice(chatId, buffer)` — wraps `Buffer` as `stream.Readable.from(buffer)`, calls `bot.sendVoice` [owner:api-engineer]
- [ ] [2.6] [P-1] Implement `editMessage(chatId, messageId, text)` — calls `bot.editMessageText(text, { chat_id: chatId, message_id: messageId, parse_mode: 'HTML' })` [owner:api-engineer]
- [ ] [2.7] [P-2] Implement `deleteMessage(chatId, messageId)` — calls `bot.deleteMessage(chatId, messageId)` [owner:api-engineer]
- [ ] [2.8] [P-2] Implement `answerCallbackQuery(callbackId, text?)` — calls `bot.answerCallbackQuery(callbackId, { text })`; called immediately in callback_query handler before forwarding to onMessage [owner:api-engineer]
- [ ] [2.9] [P-2] Implement `sendChatAction(chatId, action)` — calls `bot.sendChatAction(chatId, action)` [owner:api-engineer]
- [ ] [2.10] [P-2] Implement `stop()` — calls `bot.stopPolling()` [owner:api-engineer]

## Keyboard Batch

- [ ] [3.1] [P-2] Implement `buildKeyboard(rows: KeyboardButton[][])` — maps to Telegram `inline_keyboard` format; export from module [owner:api-engineer]
- [ ] [3.2] [P-2] Implement `obligationKeyboard(obligationId)` — returns 3-button row: Approve (`ob:approve:{id}`), Snooze (`ob:snooze:{id}`), Dismiss (`ob:dismiss:{id}`) [owner:api-engineer]
- [ ] [3.3] [P-2] Implement `reminderKeyboard(reminderId)` — returns 3-button row: Done (`reminder:done:{id}`), Snooze 1h (`reminder:snooze:1h:{id}`), Snooze tomorrow (`reminder:snooze:tomorrow:{id}`) [owner:api-engineer]

## Commands Batch

- [ ] [4.1] [P-2] Register bot commands on construction — call `bot.setMyCommands([{ command, description }, ...])` for /start, /help, /ob, /diary, /status; log error but do not throw if registration fails [owner:api-engineer]
- [ ] [4.2] [P-2] Register command handlers — `bot.onText(/\/start/, ...)`, `bot.onText(/\/help/, ...)`, `bot.onText(/\/ob/, ...)`, `bot.onText(/\/diary/, ...)`, `bot.onText(/\/status/, ...)`; each calls `onMessageCallback` with normalized `Message` with `type: 'text'` and `text` set to the command string [owner:api-engineer]

## Types Batch

- [ ] [5.1] [P-1] Define and export `SendMessageOptions` interface in `src/channels/telegram.ts` — fields: `parseMode?`, `replyToMessageId?`, `keyboard?`, `disablePreview?` [owner:api-engineer]
- [ ] [5.2] [P-1] Define and export `KeyboardButton` interface — fields: `text: string`, `callbackData: string` [owner:api-engineer]
- [ ] [5.3] [P-2] Verify `Message` type in `src/types.ts` has required fields (`id`, `channel`, `chatId`, `text`, `type`, `from`, `timestamp`, `metadata`); add `type: 'voice' | 'photo'` variants if not already present [owner:api-engineer]

## Verify

- [ ] [6.1] `tsc --noEmit` passes [owner:api-engineer]
- [ ] [6.2] Unit test: text message update normalizes to `Message` with `type: 'text'` and correct field mapping [owner:api-engineer]
- [ ] [6.3] Unit test: voice message update normalizes to `Message` with `type: 'voice'` and `metadata.fileId` populated [owner:api-engineer]
- [ ] [6.4] Unit test: callback_query update normalizes to `Message` with `type: 'callback'` and `metadata.callbackQueryId` populated [owner:api-engineer]
- [ ] [6.5] Unit test: `obligationKeyboard` produces correct callback_data format `ob:approve:{id}` etc. [owner:api-engineer]
- [ ] [6.6] Unit test: `reminderKeyboard` produces correct callback_data format `reminder:done:{id}` etc. [owner:api-engineer]
- [ ] [6.7] [user] Manual integration test: start daemon with real bot token, send `/start` — bot responds [owner:api-engineer]
- [ ] [6.8] [user] Manual integration test: send voice note — adapter yields `Message` with `metadata.fileUrl` populated [owner:api-engineer]
