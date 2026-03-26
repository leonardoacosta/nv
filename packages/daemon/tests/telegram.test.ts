import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type TelegramBot from "node-telegram-bot-api";

import {
  normalizeTextMessage,
  normalizeVoiceMessage,
  normalizePhotoMessage,
  normalizeCallbackQuery,
  obligationKeyboard,
  reminderKeyboard,
} from "../src/channels/telegram.js";

// ─── Fixtures ─────────────────────────────────────────────────────────────────

function makeUser(overrides?: Partial<TelegramBot.User>): TelegramBot.User {
  return {
    id: 123456,
    is_bot: false,
    first_name: "Leo",
    username: "leotestuser",
    ...overrides,
  };
}

function makeChat(id = 999): TelegramBot.Chat {
  return {
    id,
    type: "private",
  };
}

function makeTextMessage(
  overrides?: Partial<TelegramBot.Message>,
): TelegramBot.Message {
  return {
    message_id: 42,
    date: 1700000000,
    chat: makeChat(),
    from: makeUser(),
    text: "Hello Nova",
    ...overrides,
  };
}

// ─── Text Message Normalization ───────────────────────────────────────────────

describe("normalizeTextMessage", () => {
  it("maps type to 'text'", () => {
    const msg = makeTextMessage();
    const result = normalizeTextMessage(msg);
    assert.equal(result.type, "text");
  });

  it("maps text field correctly", () => {
    const msg = makeTextMessage({ text: "Hello Nova" });
    const result = normalizeTextMessage(msg);
    assert.equal(result.text, "Hello Nova");
  });

  it("maps channel to 'telegram'", () => {
    const msg = makeTextMessage();
    const result = normalizeTextMessage(msg);
    assert.equal(result.channel, "telegram");
  });

  it("maps chatId to string of chat.id", () => {
    const msg = makeTextMessage();
    const result = normalizeTextMessage(msg);
    assert.equal(result.chatId, "999");
  });

  it("maps from.id to string of user.id", () => {
    const msg = makeTextMessage();
    const result = normalizeTextMessage(msg);
    assert.equal(result.from.id, "123456");
  });

  it("maps from.firstName to user.first_name", () => {
    const msg = makeTextMessage();
    const result = normalizeTextMessage(msg);
    assert.equal(result.from.firstName, "Leo");
  });

  it("maps from.username to user.username", () => {
    const msg = makeTextMessage();
    const result = normalizeTextMessage(msg);
    assert.equal(result.from.username, "leotestuser");
  });

  it("maps timestamp from unix seconds to Date", () => {
    const msg = makeTextMessage({ date: 1700000000 });
    const result = normalizeTextMessage(msg);
    assert.equal(result.timestamp.getTime(), 1700000000 * 1000);
  });

  it("populates metadata.messageId", () => {
    const msg = makeTextMessage({ message_id: 42 });
    const result = normalizeTextMessage(msg);
    assert.equal(result.metadata["messageId"], 42);
  });

  it("generates a unique id on each call", () => {
    const msg = makeTextMessage();
    const r1 = normalizeTextMessage(msg);
    const r2 = normalizeTextMessage(msg);
    assert.notEqual(r1.id, r2.id);
  });

  it("sets legacy content equal to text", () => {
    const msg = makeTextMessage({ text: "test" });
    const result = normalizeTextMessage(msg);
    assert.equal(result.content, "test");
  });
});

// ─── Voice Message Normalization ─────────────────────────────────────────────

describe("normalizeVoiceMessage", () => {
  function makeVoiceMessage(): TelegramBot.Message {
    return {
      message_id: 77,
      date: 1700000100,
      chat: makeChat(888),
      from: makeUser({ id: 654321, first_name: "Leo" }),
      voice: {
        file_id: "VOICE_FILE_ID_123",
        file_unique_id: "unique_voice",
        duration: 5,
      },
    };
  }

  it("maps type to 'voice'", async () => {
    const msg = makeVoiceMessage();
    // Provide a mock bot that resolves getFileLink
    const mockBot = {
      getFileLink: async (fileId: string) => `https://api.telegram.org/file/bot/test/${fileId}`,
    } as unknown as import("node-telegram-bot-api").default;

    const result = await normalizeVoiceMessage(msg, mockBot);
    assert.equal(result.type, "voice");
  });

  it("sets text to empty string", async () => {
    const msg = makeVoiceMessage();
    const mockBot = {
      getFileLink: async (_fileId: string) => "https://example.com/voice",
    } as unknown as import("node-telegram-bot-api").default;

    const result = await normalizeVoiceMessage(msg, mockBot);
    assert.equal(result.text, "");
  });

  it("populates metadata.fileId from voice.file_id", async () => {
    const msg = makeVoiceMessage();
    const mockBot = {
      getFileLink: async (_fileId: string) => "https://example.com/voice",
    } as unknown as import("node-telegram-bot-api").default;

    const result = await normalizeVoiceMessage(msg, mockBot);
    assert.equal(result.metadata["fileId"], "VOICE_FILE_ID_123");
  });

  it("populates metadata.fileUrl from getFileLink result", async () => {
    const msg = makeVoiceMessage();
    const mockBot = {
      getFileLink: async (_fileId: string) => "https://example.com/voice.ogg",
    } as unknown as import("node-telegram-bot-api").default;

    const result = await normalizeVoiceMessage(msg, mockBot);
    assert.equal(result.metadata["fileUrl"], "https://example.com/voice.ogg");
  });

  it("maps channel to 'telegram'", async () => {
    const msg = makeVoiceMessage();
    const mockBot = {
      getFileLink: async (_fileId: string) => "https://example.com/voice",
    } as unknown as import("node-telegram-bot-api").default;

    const result = await normalizeVoiceMessage(msg, mockBot);
    assert.equal(result.channel, "telegram");
  });

  it("does not throw when getFileLink fails", async () => {
    const msg = makeVoiceMessage();
    const mockBot = {
      getFileLink: async (_fileId: string) => {
        throw new Error("Network error");
      },
    } as unknown as import("node-telegram-bot-api").default;

    const result = await normalizeVoiceMessage(msg, mockBot);
    assert.equal(result.type, "voice");
    assert.equal(result.metadata["fileUrl"], undefined);
  });
});

// ─── Callback Query Normalization ─────────────────────────────────────────────

describe("normalizeCallbackQuery", () => {
  function makeCallbackQuery(
    overrides?: Partial<TelegramBot.CallbackQuery>,
  ): TelegramBot.CallbackQuery {
    return {
      id: "CALLBACK_ID_abc",
      from: makeUser(),
      chat_instance: "instance123",
      data: "ob:approve:obligation-uuid-001",
      message: {
        message_id: 55,
        date: 1700000200,
        chat: makeChat(777),
        from: makeUser(),
        text: "Approve obligation?",
      },
      ...overrides,
    };
  }

  it("maps type to 'callback'", () => {
    const query = makeCallbackQuery();
    const result = normalizeCallbackQuery(query);
    assert.equal(result.type, "callback");
  });

  it("maps text to query.data", () => {
    const query = makeCallbackQuery({ data: "ob:approve:obligation-uuid-001" });
    const result = normalizeCallbackQuery(query);
    assert.equal(result.text, "ob:approve:obligation-uuid-001");
  });

  it("maps metadata.callbackQueryId to query.id", () => {
    const query = makeCallbackQuery();
    const result = normalizeCallbackQuery(query);
    assert.equal(result.metadata["callbackQueryId"], "CALLBACK_ID_abc");
  });

  it("maps metadata.originalMessageId to message.message_id", () => {
    const query = makeCallbackQuery();
    const result = normalizeCallbackQuery(query);
    assert.equal(result.metadata["originalMessageId"], 55);
  });

  it("maps chatId from message.chat.id", () => {
    const query = makeCallbackQuery();
    const result = normalizeCallbackQuery(query);
    assert.equal(result.chatId, "777");
  });

  it("maps channel to 'telegram'", () => {
    const query = makeCallbackQuery();
    const result = normalizeCallbackQuery(query);
    assert.equal(result.channel, "telegram");
  });
});

// ─── obligationKeyboard ───────────────────────────────────────────────────────

describe("obligationKeyboard", () => {
  it("returns a single row of 3 buttons", () => {
    const kb = obligationKeyboard("ob-123");
    assert.equal(kb.inline_keyboard.length, 1);
    assert.equal(kb.inline_keyboard[0]!.length, 3);
  });

  it("Approve button has correct callback_data", () => {
    const kb = obligationKeyboard("ob-123");
    const btn = kb.inline_keyboard[0]![0]!;
    assert.equal(btn.callback_data, "ob:approve:ob-123");
    assert.equal(btn.text, "Approve");
  });

  it("Snooze button has correct callback_data", () => {
    const kb = obligationKeyboard("ob-123");
    const btn = kb.inline_keyboard[0]![1]!;
    assert.equal(btn.callback_data, "ob:snooze:ob-123");
    assert.equal(btn.text, "Snooze");
  });

  it("Dismiss button has correct callback_data", () => {
    const kb = obligationKeyboard("ob-123");
    const btn = kb.inline_keyboard[0]![2]!;
    assert.equal(btn.callback_data, "ob:dismiss:ob-123");
    assert.equal(btn.text, "Dismiss");
  });

  it("embeds the obligationId in all callback_data values", () => {
    const id = "my-obligation-uuid";
    const kb = obligationKeyboard(id);
    const row = kb.inline_keyboard[0]!;
    assert.ok(row[0]!.callback_data!.includes(id));
    assert.ok(row[1]!.callback_data!.includes(id));
    assert.ok(row[2]!.callback_data!.includes(id));
  });
});

// ─── reminderKeyboard ─────────────────────────────────────────────────────────

describe("reminderKeyboard", () => {
  it("returns a single row of 3 buttons", () => {
    const kb = reminderKeyboard("rem-456");
    assert.equal(kb.inline_keyboard.length, 1);
    assert.equal(kb.inline_keyboard[0]!.length, 3);
  });

  it("Done button has correct callback_data", () => {
    const kb = reminderKeyboard("rem-456");
    const btn = kb.inline_keyboard[0]![0]!;
    assert.equal(btn.callback_data, "reminder:done:rem-456");
    assert.equal(btn.text, "Done");
  });

  it("Snooze 1h button has correct callback_data", () => {
    const kb = reminderKeyboard("rem-456");
    const btn = kb.inline_keyboard[0]![1]!;
    assert.equal(btn.callback_data, "reminder:snooze:1h:rem-456");
    assert.equal(btn.text, "Snooze 1h");
  });

  it("Snooze tomorrow button has correct callback_data", () => {
    const kb = reminderKeyboard("rem-456");
    const btn = kb.inline_keyboard[0]![2]!;
    assert.equal(btn.callback_data, "reminder:snooze:tomorrow:rem-456");
    assert.equal(btn.text, "Snooze tomorrow");
  });

  it("embeds the reminderId in all callback_data values", () => {
    const id = "my-reminder-uuid";
    const kb = reminderKeyboard(id);
    const row = kb.inline_keyboard[0]!;
    assert.ok(row[0]!.callback_data!.includes(id));
    assert.ok(row[1]!.callback_data!.includes(id));
    assert.ok(row[2]!.callback_data!.includes(id));
  });
});
