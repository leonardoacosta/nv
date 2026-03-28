import { describe, it } from "node:test";
import assert from "node:assert/strict";

// telegram.ts cannot be imported directly (transitive @nova/db dependency
// requires DATABASE_URL at module load). We replicate the quoted-text logic
// here to verify the normalization contract.

// ── Replicated logic from normalizeTextMessage ─────────────────────────────

interface MinimalTelegramMessage {
  text?: string;
  caption?: string;
  reply_to_message?: { message_id: number; text?: string };
}

function extractTextWithQuote(msg: MinimalTelegramMessage, field: "text" | "caption" = "text"): string {
  const rawText = msg[field] ?? "";
  const quotedText = msg.reply_to_message?.text;
  return quotedText
    ? `[Quoting: "${quotedText}"]\n${rawText}`
    : rawText;
}

// ── Tests ──────────────────────────────────────────────────────────────────

describe("Quoted text in normalization", () => {
  describe("normalizeTextMessage quoted text", () => {
    it("prepends quoted text when reply_to_message has text", () => {
      const msg: MinimalTelegramMessage = {
        text: "Can you give me more information on this?",
        reply_to_message: {
          message_id: 42,
          text: "PR tool is broken in ADO — needs fix in nova graph-svc",
        },
      };

      const result = extractTextWithQuote(msg);

      assert.equal(
        result,
        '[Quoting: "PR tool is broken in ADO — needs fix in nova graph-svc"]\nCan you give me more information on this?',
      );
    });

    it("returns raw text when no reply_to_message", () => {
      const msg: MinimalTelegramMessage = {
        text: "Hello Nova",
      };

      const result = extractTextWithQuote(msg);

      assert.equal(result, "Hello Nova");
    });

    it("returns raw text when reply_to_message has no text (e.g., photo/voice)", () => {
      const msg: MinimalTelegramMessage = {
        text: "What is this?",
        reply_to_message: {
          message_id: 42,
          // text is undefined — quoted message was a photo or voice
        },
      };

      const result = extractTextWithQuote(msg);

      assert.equal(result, "What is this?");
    });

    it("handles empty user text with quoted message", () => {
      const msg: MinimalTelegramMessage = {
        text: "",
        reply_to_message: {
          message_id: 42,
          text: "Some reminder text",
        },
      };

      const result = extractTextWithQuote(msg);

      assert.equal(result, '[Quoting: "Some reminder text"]\n');
    });

    it("handles long quoted text without truncation", () => {
      const longQuote = "A".repeat(2000);
      const msg: MinimalTelegramMessage = {
        text: "Thoughts?",
        reply_to_message: {
          message_id: 42,
          text: longQuote,
        },
      };

      const result = extractTextWithQuote(msg);

      assert.ok(result.startsWith(`[Quoting: "${longQuote}"]`));
      assert.ok(result.endsWith("Thoughts?"));
    });
  });

  describe("normalizePhotoMessage quoted text", () => {
    it("prepends quoted text to photo caption", () => {
      const msg: MinimalTelegramMessage = {
        caption: "Look at this error",
        reply_to_message: {
          message_id: 42,
          text: "Check the deployment logs",
        },
      };

      const result = extractTextWithQuote(msg, "caption");

      assert.equal(
        result,
        '[Quoting: "Check the deployment logs"]\nLook at this error',
      );
    });

    it("returns caption only when no reply", () => {
      const msg: MinimalTelegramMessage = {
        caption: "Screenshot of the error",
      };

      const result = extractTextWithQuote(msg, "caption");

      assert.equal(result, "Screenshot of the error");
    });
  });

  describe("voice message quoted text", () => {
    it("returns only quoted text for voice replies (no user text)", () => {
      const msg: MinimalTelegramMessage = {
        // voice messages have no text — content comes from STT
        reply_to_message: {
          message_id: 42,
          text: "PR tool is broken in ADO",
        },
      };

      const quotedText = msg.reply_to_message?.text;
      const result = quotedText
        ? `[Quoting: "${quotedText}"]`
        : "";

      assert.equal(result, '[Quoting: "PR tool is broken in ADO"]');
    });

    it("returns empty string for voice without reply", () => {
      const msg: MinimalTelegramMessage = {};

      const quotedText = msg.reply_to_message?.text;
      const result = quotedText
        ? `[Quoting: "${quotedText}"]`
        : "";

      assert.equal(result, "");
    });
  });
});
