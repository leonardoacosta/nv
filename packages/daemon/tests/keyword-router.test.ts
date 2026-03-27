import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { KeywordRouter } from "../src/brain/keyword-router.js";

describe("KeywordRouter", () => {
  const router = new KeywordRouter();

  describe("calendar_today matches", () => {
    const calendarPhrases = [
      "what's on my calendar",
      "today's schedule",
      "my agenda",
      "do I have any meetings today",
      "am I free today",
      "What's on my calendar today?",
    ];

    for (const phrase of calendarPhrases) {
      it(`matches "${phrase}" to calendar_today`, () => {
        const result = router.match(phrase);
        assert.ok(result, `Expected match for "${phrase}"`);
        assert.equal(result.tool, "calendar_today");
        assert.equal(result.port, 4106);
        assert.equal(result.confidence, 0.95);
      });
    }
  });

  describe("calendar_upcoming matches", () => {
    const phrases = [
      "upcoming events",
      "what's next",
      "next meeting",
      "what do I have this week",
    ];

    for (const phrase of phrases) {
      it(`matches "${phrase}" to calendar_upcoming`, () => {
        const result = router.match(phrase);
        assert.ok(result, `Expected match for "${phrase}"`);
        assert.equal(result.tool, "calendar_upcoming");
      });
    }
  });

  describe("email_inbox matches", () => {
    const phrases = [
      "check my email",
      "any new emails",
      "show my inbox",
      "unread emails",
    ];

    for (const phrase of phrases) {
      it(`matches "${phrase}" to email_inbox`, () => {
        const result = router.match(phrase);
        assert.ok(result, `Expected match for "${phrase}"`);
        assert.equal(result.tool, "email_inbox");
        assert.equal(result.port, 4103);
      });
    }
  });

  describe("email_send matches", () => {
    it('matches "send an email to John" to email_send', () => {
      const result = router.match("send an email to John");
      assert.ok(result);
      assert.equal(result.tool, "email_send");
      assert.equal(result.port, 4103);
      assert.ok(result.params.rawText);
    });
  });

  describe("weather matches", () => {
    const phrases = ["weather", "forecast", "is it going to rain"];

    for (const phrase of phrases) {
      it(`matches "${phrase}" to weather_current`, () => {
        const result = router.match(phrase);
        assert.ok(result, `Expected match for "${phrase}"`);
        assert.equal(result.tool, "weather_current");
        assert.equal(result.port, 4104);
      });
    }
  });

  describe("health_check matches", () => {
    const phrases = ["system status", "health check", "are services running"];

    for (const phrase of phrases) {
      it(`matches "${phrase}" to health_check`, () => {
        const result = router.match(phrase);
        assert.ok(result, `Expected match for "${phrase}"`);
        assert.equal(result.tool, "health_check");
        assert.equal(result.port, 4100);
      });
    }
  });

  describe("datetime matches", () => {
    it('matches "what time is it" to datetime_now', () => {
      const result = router.match("what time is it");
      assert.ok(result);
      assert.equal(result.tool, "datetime_now");
      assert.equal(result.port, 4108);
    });
  });

  describe("memory_read matches with params", () => {
    it('matches "what do you know about project alpha" and extracts topic', () => {
      const result = router.match("what do you know about project alpha");
      assert.ok(result);
      assert.equal(result.tool, "memory_read");
      assert.equal(result.port, 4101);
      assert.equal(result.params.topic, "project alpha");
    });
  });

  describe("no match for ambiguous/unrelated text", () => {
    const nonMatches = [
      "tell me about quantum physics",
      "what is the meaning of life",
      "can you help me write a poem",
      "explain how neural networks work",
      "hello there",
      "",
    ];

    for (const phrase of nonMatches) {
      it(`returns null for "${phrase || "(empty string)"}"`, () => {
        const result = router.match(phrase);
        assert.equal(result, null);
      });
    }
  });

  describe("contact_lookup matches with params", () => {
    it('matches "who is John Smith" and extracts name', () => {
      const result = router.match("who is John Smith");
      assert.ok(result);
      assert.equal(result.tool, "contact_lookup");
      assert.equal(result.port, 4105);
      assert.ok(result.params.name);
    });
  });

  describe("reminders matches", () => {
    it('matches "my reminders" to reminders_list', () => {
      const result = router.match("my reminders");
      assert.ok(result);
      assert.equal(result.tool, "reminders_list");
    });

    it('matches "remind me to buy groceries" to reminder_create', () => {
      const result = router.match("remind me to buy groceries");
      assert.ok(result);
      assert.equal(result.tool, "reminder_create");
      assert.ok(result.params.rawText);
    });
  });
});
