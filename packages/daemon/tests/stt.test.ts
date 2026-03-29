/**
 * Unit tests for packages/daemon/src/features/stt/client.ts
 * and the STT wiring in normalizeVoiceMessage().
 *
 * Tasks:
 *   [4.1] SttError construction + transcribe() fetch scenarios
 *   [4.2] normalizeVoiceMessage() STT wiring
 */

// Set DATABASE_URL before any module imports so @nova/db client does not throw.
// telegram.ts transitively imports command modules that import @nova/db at load time.
process.env["DATABASE_URL"] = "postgres://test:test@localhost:5432/test_dummy";

import { describe, it, mock, beforeEach } from "node:test";
import assert from "node:assert/strict";
import type TelegramBot from "node-telegram-bot-api";

// ─── Helpers ──────────────────────────────────────────────────────────────────

function makeVoiceMessage(
  overrides?: Partial<TelegramBot.Message>,
): TelegramBot.Message {
  return {
    message_id: 10,
    date: 1700000000,
    chat: { id: 999, type: "private" },
    from: { id: 123, is_bot: false, first_name: "Leo" },
    voice: {
      file_id: "voice-file-001",
      file_unique_id: "uq001",
      duration: 3,
      mime_type: "audio/ogg",
      file_size: 8192,
    },
    ...overrides,
  };
}

/** Build a minimal fetch Response */
function makeFetchResponse(
  status: number,
  body: unknown,
  options?: { abort?: boolean },
): Response {
  if (options?.abort) {
    // Simulate an aborted fetch (timeout)
    throw Object.assign(new Error("The operation was aborted."), { name: "AbortError" });
  }
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: status === 200 ? "OK" : status === 403 ? "Forbidden" : status === 429 ? "Too Many Requests" : "Error",
    arrayBuffer: async () => new ArrayBuffer(100),
    json: async () => body,
  } as Response;
}

// ─── Tests [4.1]: SttError + transcribe() ────────────────────────────────────

describe("SttError", () => {
  it("constructs with code: missing_key", async () => {
    const { SttError } = await import("../src/features/stt/client.js");
    const err = new SttError("missing_key", "ELEVENLABS_API_KEY not set");
    assert.equal(err.code, "missing_key");
    assert.equal(err.name, "SttError");
    assert.equal(err.message, "ELEVENLABS_API_KEY not set");
    assert.ok(err instanceof Error);
  });

  it("constructs with code: download_failed", async () => {
    const { SttError } = await import("../src/features/stt/client.js");
    const err = new SttError("download_failed", "Audio download failed: HTTP 403 Forbidden");
    assert.equal(err.code, "download_failed");
    assert.equal(err.name, "SttError");
  });

  it("constructs with code: api_error", async () => {
    const { SttError } = await import("../src/features/stt/client.js");
    const err = new SttError("api_error", "ElevenLabs STT API error: HTTP 429 Too Many Requests");
    assert.equal(err.code, "api_error");
    assert.equal(err.name, "SttError");
  });

  it("constructs with code: empty_transcript", async () => {
    const { SttError } = await import("../src/features/stt/client.js");
    const err = new SttError("empty_transcript", "ElevenLabs STT returned an empty transcript");
    assert.equal(err.code, "empty_transcript");
    assert.equal(err.name, "SttError");
  });
});

describe("transcribe()", () => {
  // Save and restore the original fetch and env
  const originalFetch = global.fetch;
  const originalEnv = process.env["ELEVENLABS_API_KEY"];

  beforeEach(() => {
    // Reset to a clean state before each test
    global.fetch = originalFetch;
    if (originalEnv !== undefined) {
      process.env["ELEVENLABS_API_KEY"] = originalEnv;
    } else {
      delete process.env["ELEVENLABS_API_KEY"];
    }
  });

  it("throws SttError code: missing_key when ELEVENLABS_API_KEY is not set", async () => {
    const { transcribe, SttError } = await import("../src/features/stt/client.js");
    delete process.env["ELEVENLABS_API_KEY"];

    await assert.rejects(
      () => transcribe("https://example.com/voice.ogg"),
      (err: unknown) => {
        assert.ok(err instanceof SttError);
        assert.equal((err as InstanceType<typeof SttError>).code, "missing_key");
        return true;
      },
    );
  });

  it("throws SttError code: missing_key when ELEVENLABS_API_KEY is empty string", async () => {
    const { transcribe, SttError } = await import("../src/features/stt/client.js");
    process.env["ELEVENLABS_API_KEY"] = "";

    await assert.rejects(
      () => transcribe("https://example.com/voice.ogg"),
      (err: unknown) => {
        assert.ok(err instanceof SttError);
        assert.equal((err as InstanceType<typeof SttError>).code, "missing_key");
        return true;
      },
    );
  });

  it("returns transcript string on successful 200 response", async () => {
    const { transcribe } = await import("../src/features/stt/client.js");
    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let callCount = 0;
    global.fetch = mock.fn(async (_url: string | URL | Request) => {
      callCount++;
      if (callCount === 1) {
        // Download request
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      // ElevenLabs STT request
      return {
        ok: true,
        status: 200,
        statusText: "OK",
        json: async () => ({ text: "hello world" }),
      } as Response;
    }) as typeof fetch;

    const result = await transcribe("https://cdn.telegram.org/voice.ogg");
    assert.equal(result, "hello world");
  });

  it("throws SttError code: download_failed on non-200 download response (403)", async () => {
    const { transcribe, SttError } = await import("../src/features/stt/client.js");
    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    global.fetch = mock.fn(async () => ({
      ok: false,
      status: 403,
      statusText: "Forbidden",
      arrayBuffer: async () => new ArrayBuffer(0),
    } as Response)) as typeof fetch;

    await assert.rejects(
      () => transcribe("https://cdn.telegram.org/voice.ogg"),
      (err: unknown) => {
        assert.ok(err instanceof SttError);
        assert.equal((err as InstanceType<typeof SttError>).code, "download_failed");
        return true;
      },
    );
  });

  it("throws SttError code: download_failed on download AbortError (timeout)", async () => {
    const { transcribe, SttError } = await import("../src/features/stt/client.js");
    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    global.fetch = mock.fn(async () => {
      const err = Object.assign(new Error("The operation was aborted."), { name: "AbortError" });
      throw err;
    }) as typeof fetch;

    await assert.rejects(
      () => transcribe("https://cdn.telegram.org/voice.ogg"),
      (err: unknown) => {
        assert.ok(err instanceof SttError);
        assert.equal((err as InstanceType<typeof SttError>).code, "download_failed");
        return true;
      },
    );
  });

  it("throws SttError code: api_error on ElevenLabs non-2xx response (429)", async () => {
    const { transcribe, SttError } = await import("../src/features/stt/client.js");
    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let callCount = 0;
    global.fetch = mock.fn(async () => {
      callCount++;
      if (callCount === 1) {
        // Download succeeds
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      // ElevenLabs returns 429
      return {
        ok: false,
        status: 429,
        statusText: "Too Many Requests",
        json: async () => ({}),
      } as Response;
    }) as typeof fetch;

    await assert.rejects(
      () => transcribe("https://cdn.telegram.org/voice.ogg"),
      (err: unknown) => {
        assert.ok(err instanceof SttError);
        assert.equal((err as InstanceType<typeof SttError>).code, "api_error");
        return true;
      },
    );
  });

  it("throws SttError code: empty_transcript when ElevenLabs returns { text: '' }", async () => {
    const { transcribe, SttError } = await import("../src/features/stt/client.js");
    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let callCount = 0;
    global.fetch = mock.fn(async () => {
      callCount++;
      if (callCount === 1) {
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      return {
        ok: true,
        status: 200,
        statusText: "OK",
        json: async () => ({ text: "" }),
      } as Response;
    }) as typeof fetch;

    await assert.rejects(
      () => transcribe("https://cdn.telegram.org/voice.ogg"),
      (err: unknown) => {
        assert.ok(err instanceof SttError);
        assert.equal((err as InstanceType<typeof SttError>).code, "empty_transcript");
        return true;
      },
    );
  });

  it("throws SttError code: empty_transcript when ElevenLabs returns whitespace-only text", async () => {
    const { transcribe, SttError } = await import("../src/features/stt/client.js");
    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let callCount = 0;
    global.fetch = mock.fn(async () => {
      callCount++;
      if (callCount === 1) {
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      return {
        ok: true,
        status: 200,
        statusText: "OK",
        json: async () => ({ text: "   " }),
      } as Response;
    }) as typeof fetch;

    await assert.rejects(
      () => transcribe("https://cdn.telegram.org/voice.ogg"),
      (err: unknown) => {
        assert.ok(err instanceof SttError);
        assert.equal((err as InstanceType<typeof SttError>).code, "empty_transcript");
        return true;
      },
    );
  });

  it("throws SttError code: api_error on ElevenLabs request AbortError (timeout)", async () => {
    const { transcribe, SttError } = await import("../src/features/stt/client.js");
    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let callCount = 0;
    global.fetch = mock.fn(async () => {
      callCount++;
      if (callCount === 1) {
        // Download succeeds
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      // ElevenLabs times out
      const err = Object.assign(new Error("The operation was aborted."), { name: "AbortError" });
      throw err;
    }) as typeof fetch;

    await assert.rejects(
      () => transcribe("https://cdn.telegram.org/voice.ogg"),
      (err: unknown) => {
        assert.ok(err instanceof SttError);
        assert.equal((err as InstanceType<typeof SttError>).code, "api_error");
        return true;
      },
    );
  });
});

// ─── Tests [4.2]: normalizeVoiceMessage() STT wiring ─────────────────────────

describe("normalizeVoiceMessage() — STT wiring", () => {
  // We test normalizeVoiceMessage by mocking bot.getFileLink() and the transcribe() function.
  // Since transcribe() is imported at module load in telegram.ts, we use process.env to
  // control SttError paths, and mock the bot to control fileUrl availability.

  function makeMockBot(
    getFileLinkImpl: (fileId: string) => Promise<string>,
  ): TelegramBot {
    return {
      getFileLink: mock.fn(getFileLinkImpl),
    } as unknown as TelegramBot;
  }

  it("happy path: sets text and content to transcript when transcription succeeds", async () => {
    const { normalizeVoiceMessage } = await import("../src/channels/telegram.js");
    const { SttError } = await import("../src/features/stt/client.js");

    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let fetchCallCount = 0;
    global.fetch = mock.fn(async () => {
      fetchCallCount++;
      if (fetchCallCount === 1) {
        // Download
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      // STT API
      return {
        ok: true,
        status: 200,
        statusText: "OK",
        json: async () => ({ text: "schedule meeting tomorrow" }),
      } as Response;
    }) as typeof fetch;

    const bot = makeMockBot(async () => "https://cdn.telegram.org/voice.ogg");
    const msg = makeVoiceMessage();

    const result = await normalizeVoiceMessage(msg, bot);

    assert.equal(result.text, "schedule meeting tomorrow");
    assert.equal(result.content, "schedule meeting tomorrow");
    assert.equal(result.type, "voice");
    assert.equal(result.channel, "telegram");
  });

  it("SttError thrown by transcribe() results in fallback text, no rethrow", async () => {
    const { normalizeVoiceMessage } = await import("../src/channels/telegram.js");
    const { SttError } = await import("../src/features/stt/client.js");

    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let fetchCallCount = 0;
    global.fetch = mock.fn(async () => {
      fetchCallCount++;
      if (fetchCallCount === 1) {
        // Download succeeds
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      // ElevenLabs returns 429 → triggers SttError api_error
      return {
        ok: false,
        status: 429,
        statusText: "Too Many Requests",
        json: async () => ({}),
      } as Response;
    }) as typeof fetch;

    const bot = makeMockBot(async () => "https://cdn.telegram.org/voice.ogg");
    const msg = makeVoiceMessage();

    // Should NOT throw — fallback text is used
    const result = await normalizeVoiceMessage(msg, bot);

    assert.equal(result.text, "[Voice message — transcription unavailable]");
    assert.equal(result.content, "[Voice message — transcription unavailable]");
    assert.equal(result.type, "voice");
  });

  it("generic error from transcribe() results in fallback text, no rethrow", async () => {
    const { normalizeVoiceMessage } = await import("../src/channels/telegram.js");

    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    global.fetch = mock.fn(async () => {
      throw new Error("Network error");
    }) as typeof fetch;

    const bot = makeMockBot(async () => "https://cdn.telegram.org/voice.ogg");
    const msg = makeVoiceMessage();

    const result = await normalizeVoiceMessage(msg, bot);

    assert.equal(result.text, "[Voice message — transcription unavailable]");
    assert.equal(result.content, "[Voice message — transcription unavailable]");
  });

  it("bot.getFileLink() throws → transcribe() not called, file-retrieval fallback text set", async () => {
    const { normalizeVoiceMessage } = await import("../src/channels/telegram.js");

    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    // Track if fetch (transcribe) was called
    const fetchMock = mock.fn(async () => {
      throw new Error("fetch should not be called");
    });
    global.fetch = fetchMock as typeof fetch;

    const bot = makeMockBot(async () => {
      throw new Error("Bot API error: file not found");
    });
    const msg = makeVoiceMessage();

    const result = await normalizeVoiceMessage(msg, bot);

    assert.equal(result.text, "[Voice message — could not retrieve audio file]");
    assert.equal(result.content, "[Voice message — could not retrieve audio file]");
    // fetch should not have been called
    assert.equal((fetchMock as ReturnType<typeof mock.fn>).mock.calls.length, 0);
  });

  it("preserves metadata.fileUrl on happy path", async () => {
    const { normalizeVoiceMessage } = await import("../src/channels/telegram.js");

    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let fetchCallCount = 0;
    global.fetch = mock.fn(async () => {
      fetchCallCount++;
      if (fetchCallCount === 1) {
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      return {
        ok: true,
        status: 200,
        statusText: "OK",
        json: async () => ({ text: "test transcript" }),
      } as Response;
    }) as typeof fetch;

    const fileUrl = "https://cdn.telegram.org/file/voice_abc.ogg";
    const bot = makeMockBot(async () => fileUrl);
    const msg = makeVoiceMessage();

    const result = await normalizeVoiceMessage(msg, bot);

    assert.equal(result.metadata["fileUrl"], fileUrl);
    assert.equal(result.text, "test transcript");
  });

  it("preserves metadata.fileUrl when SttError is thrown (transcription unavailable)", async () => {
    const { normalizeVoiceMessage } = await import("../src/channels/telegram.js");

    process.env["ELEVENLABS_API_KEY"] = "test-api-key";

    let fetchCallCount = 0;
    global.fetch = mock.fn(async () => {
      fetchCallCount++;
      if (fetchCallCount === 1) {
        return {
          ok: true,
          status: 200,
          statusText: "OK",
          arrayBuffer: async () => new ArrayBuffer(100),
        } as Response;
      }
      return {
        ok: false,
        status: 503,
        statusText: "Service Unavailable",
        json: async () => ({}),
      } as Response;
    }) as typeof fetch;

    const fileUrl = "https://cdn.telegram.org/file/voice_xyz.ogg";
    const bot = makeMockBot(async () => fileUrl);
    const msg = makeVoiceMessage();

    const result = await normalizeVoiceMessage(msg, bot);

    assert.equal(result.metadata["fileUrl"], fileUrl);
    assert.equal(result.text, "[Voice message — transcription unavailable]");
  });

  it("metadata.fileUrl is absent when bot.getFileLink() fails", async () => {
    const { normalizeVoiceMessage } = await import("../src/channels/telegram.js");

    process.env["ELEVENLABS_API_KEY"] = "test-api-key";
    global.fetch = mock.fn(async () => { throw new Error("should not call"); }) as typeof fetch;

    const bot = makeMockBot(async () => {
      throw new Error("Bot API error");
    });
    const msg = makeVoiceMessage();

    const result = await normalizeVoiceMessage(msg, bot);

    // fileUrl should not be present in metadata when getFileLink threw
    assert.equal(result.metadata["fileUrl"], undefined);
    assert.equal(result.text, "[Voice message — could not retrieve audio file]");
  });
});
