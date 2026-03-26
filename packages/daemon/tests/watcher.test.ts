import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";

// ─── Helpers ──────────────────────────────────────────────────────────────────

function makeRow(overrides: Partial<{
  id: string;
  detected_action: string;
  owner: string;
  status: string;
  priority: number;
  project_code: string | null;
  source_channel: string;
  source_message: string | null;
  deadline: Date | null;
  last_attempt_at: Date | null;
  created_at: Date;
  updated_at: Date;
}> = {}) {
  return {
    id: "test-id-123",
    detected_action: "Deploy auth service",
    owner: "leo",
    status: "in_progress",
    priority: 1,
    project_code: null,
    source_channel: "telegram",
    source_message: null,
    deadline: null,
    last_attempt_at: null,
    created_at: new Date("2025-01-01T00:00:00Z"),
    updated_at: new Date("2025-01-01T00:00:00Z"),
    ...overrides,
  };
}

// ─── Test: isQuietHours ───────────────────────────────────────────────────────

describe("isQuietHours", () => {
  it("returns true for time inside 22:00–07:00 window (before midnight)", async () => {
    const { isQuietHours } = await import(
      "../src/features/watcher/proactive.js"
    );
    const config = {
      enabled: true,
      intervalMinutes: 30,
      staleThresholdHours: 48,
      approachingDeadlineHours: 24,
      maxRemindersPerInterval: 1,
      quietStart: "22:00",
      quietEnd: "07:00",
    };

    // 23:00 — inside quiet window (after 22:00)
    const nightTime = new Date("2025-01-01T23:00:00");
    assert.equal(isQuietHours(nightTime, config), true);
  });

  it("returns true for time inside 22:00–07:00 window (after midnight)", async () => {
    const { isQuietHours } = await import(
      "../src/features/watcher/proactive.js"
    );
    const config = {
      enabled: true,
      intervalMinutes: 30,
      staleThresholdHours: 48,
      approachingDeadlineHours: 24,
      maxRemindersPerInterval: 1,
      quietStart: "22:00",
      quietEnd: "07:00",
    };

    // 03:00 — inside quiet window (before 07:00)
    const earlyMorning = new Date("2025-01-02T03:00:00");
    assert.equal(isQuietHours(earlyMorning, config), true);
  });

  it("returns false for time outside 22:00–07:00 window", async () => {
    const { isQuietHours } = await import(
      "../src/features/watcher/proactive.js"
    );
    const config = {
      enabled: true,
      intervalMinutes: 30,
      staleThresholdHours: 48,
      approachingDeadlineHours: 24,
      maxRemindersPerInterval: 1,
      quietStart: "22:00",
      quietEnd: "07:00",
    };

    // 14:00 — outside quiet window
    const afternoon = new Date("2025-01-01T14:00:00");
    assert.equal(isQuietHours(afternoon, config), false);
  });

  it("returns false when quietStart === quietEnd (zero-length window)", async () => {
    const { isQuietHours } = await import(
      "../src/features/watcher/proactive.js"
    );
    const config = {
      enabled: true,
      intervalMinutes: 30,
      staleThresholdHours: 48,
      approachingDeadlineHours: 24,
      maxRemindersPerInterval: 1,
      quietStart: "07:00",
      quietEnd: "07:00",
    };

    const anyTime = new Date("2025-01-01T07:00:00");
    assert.equal(isQuietHours(anyTime, config), false);
  });
});

// ─── Test: formatReminderCard ─────────────────────────────────────────────────

describe("formatReminderCard", () => {
  it("overdue obligation with project code produces expected HTML", async () => {
    const { formatReminderCard } = await import(
      "../src/features/watcher/proactive.js"
    );

    const now = new Date("2025-01-10T12:00:00Z");
    const deadline = new Date("2025-01-08T12:00:00Z"); // 2 days ago
    const row = makeRow({
      detected_action: "Deploy auth service by Friday",
      status: "in_progress",
      project_code: "OO",
      deadline,
    });

    const card = formatReminderCard(row, "overdue", now);

    assert.ok(card.includes("<b>[OVERDUE]</b>"), "Should have bold OVERDUE badge");
    assert.ok(card.includes("Deploy auth service by Friday"), "Should include action");
    assert.ok(card.includes("Status: in_progress"), "Should include status");
    assert.ok(card.includes("Overdue by: 2 days"), "Should show overdue duration");
    assert.ok(card.includes("Project: OO"), "Should include project code");
  });

  it("stale obligation without project code omits project line", async () => {
    const { formatReminderCard } = await import(
      "../src/features/watcher/proactive.js"
    );

    const now = new Date("2025-01-10T12:00:00Z");
    const updatedAt = new Date("2025-01-08T09:00:00Z"); // ~51 hours ago
    const row = makeRow({
      detected_action: "Update API docs",
      status: "pending",
      project_code: null,
      updated_at: updatedAt,
    });

    const card = formatReminderCard(row, "stale", now);

    assert.ok(card.includes("<b>[STALE]</b>"), "Should have bold STALE badge");
    assert.ok(card.includes("Status: pending"), "Should include status");
    assert.ok(card.includes("No update in:"), "Should show stale duration");
    assert.ok(!card.includes("Project:"), "Should not include project line");
  });

  it("approaching obligation shows correct badge and deadline context", async () => {
    const { formatReminderCard } = await import(
      "../src/features/watcher/proactive.js"
    );

    const now = new Date("2025-01-10T04:00:00Z");
    const deadline = new Date("2025-01-10T12:00:00Z"); // 8 hours away
    const row = makeRow({
      detected_action: "Migrate production database",
      status: "pending",
      project_code: "TC",
      deadline,
    });

    const card = formatReminderCard(row, "approaching", now);

    assert.ok(card.includes("<b>[APPROACHING]</b>"), "Should have bold APPROACHING badge");
    assert.ok(card.includes("Deadline in: 8 hours"), "Should show 8 hours");
    assert.ok(card.includes("Project: TC"), "Should include project code");
  });
});

// ─── Test: watcherKeyboard ────────────────────────────────────────────────────

describe("watcherKeyboard", () => {
  it("returns keyboard with 3 buttons and correct callback_data prefixes", async () => {
    const { watcherKeyboard } = await import(
      "../src/features/watcher/callbacks.js"
    );

    const id = "abc-123";
    const keyboard = watcherKeyboard(id);

    assert.ok(keyboard.inline_keyboard, "Should have inline_keyboard");
    assert.equal(keyboard.inline_keyboard.length, 1, "Should have 1 row");

    const row = keyboard.inline_keyboard[0];
    assert.ok(row, "Row should exist");
    assert.equal(row.length, 3, "Row should have 3 buttons");

    const [done, snooze, dismiss] = row;
    assert.ok(done, "Done button should exist");
    assert.ok(snooze, "Snooze button should exist");
    assert.ok(dismiss, "Dismiss button should exist");

    assert.equal(done.callback_data, `watcher:done:${id}`);
    assert.equal(snooze.callback_data, `watcher:snooze:${id}`);
    assert.equal(dismiss.callback_data, `watcher:dismiss:${id}`);

    assert.equal(done.text, "Mark Done");
    assert.equal(snooze.text, "Snooze 24h");
    assert.equal(dismiss.text, "Dismiss");
  });
});

// ─── Test: scan() with mocked DB ─────────────────────────────────────────────

describe("ProactiveWatcher.scan()", () => {
  it("maxRemindersPerInterval=1 sends at most 1 notification even when 3 obligations match", async () => {
    const { ProactiveWatcher } = await import(
      "../src/features/watcher/proactive.js"
    );

    const rows = [
      makeRow({ id: "id-1", status: "in_progress", deadline: new Date(Date.now() - 3600_000) }),
      makeRow({ id: "id-2", status: "in_progress", deadline: new Date(Date.now() - 7200_000) }),
      makeRow({ id: "id-3", status: "pending", deadline: new Date(Date.now() - 10800_000) }),
    ];

    const mockPool = {
      query: mock.fn(async () => ({ rows })),
    };

    const sendMessageFn = mock.fn(async () => ({ message_id: 1 }));
    const mockTelegram = {
      sendMessage: sendMessageFn,
      answerCallbackQuery: mock.fn(async () => {}),
      editMessage: mock.fn(async () => {}),
    };

    const mockLogger = {
      info: mock.fn(() => {}),
      warn: mock.fn(() => {}),
      error: mock.fn(() => {}),
      debug: mock.fn(() => {}),
    };

    const config = {
      enabled: true,
      intervalMinutes: 30,
      staleThresholdHours: 48,
      approachingDeadlineHours: 24,
      maxRemindersPerInterval: 1,
      quietStart: "22:00",
      quietEnd: "07:00",
    };

    const watcher = new ProactiveWatcher(
      mockPool as never,
      mockTelegram as never,
      config,
      mockLogger as never,
      "12345",
    );

    await watcher.scan();

    // Only 1 notification should have been sent despite 3 matching rows (×3 queries)
    const sendCalls = (sendMessageFn as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(sendCalls.length, 1, "Should send at most 1 notification (maxRemindersPerInterval=1)");
  });

  it("quiet hours suppresses all sends", async () => {
    const { ProactiveWatcher } = await import(
      "../src/features/watcher/proactive.js"
    );

    const rows = [
      makeRow({ id: "id-1", status: "in_progress" }),
    ];

    const mockPool = {
      query: mock.fn(async () => ({ rows })),
    };

    const sendMessageFn = mock.fn(async () => ({ message_id: 1 }));
    const mockTelegram = {
      sendMessage: sendMessageFn,
    };

    const debugFn = mock.fn(() => {});
    const mockLogger = {
      info: mock.fn(() => {}),
      warn: mock.fn(() => {}),
      error: mock.fn(() => {}),
      debug: debugFn,
    };

    // Quiet window that always encompasses "now" by using 00:00–23:59
    const config = {
      enabled: true,
      intervalMinutes: 30,
      staleThresholdHours: 48,
      approachingDeadlineHours: 24,
      maxRemindersPerInterval: 5,
      quietStart: "00:00",
      quietEnd: "23:59",
    };

    const watcher = new ProactiveWatcher(
      mockPool as never,
      mockTelegram as never,
      config,
      mockLogger as never,
      "12345",
    );

    await watcher.scan();

    const sendCalls = (sendMessageFn as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(sendCalls.length, 0, "No notifications sent during quiet hours");

    const debugCalls = (debugFn as ReturnType<typeof mock.fn>).mock.calls;
    const quietLog = debugCalls.find(
      (c) => typeof c.arguments[0] === "string" && c.arguments[0].includes("quiet hours"),
    );
    assert.ok(quietLog, "Should log quiet hours skip at debug level");
  });
});

// ─── Test: handleWatcherCallback ──────────────────────────────────────────────

describe("handleWatcherCallback", () => {
  function makeDb() {
    return { query: mock.fn(async () => ({ rows: [] })) };
  }

  function makeTelegram() {
    return {
      answerCallbackQuery: mock.fn(async () => {}),
      editMessage: mock.fn(async () => {}),
    };
  }

  it("watcher:done:{id} sets status done and edits message", async () => {
    const { handleWatcherCallback } = await import(
      "../src/features/watcher/callbacks.js"
    );

    const db = makeDb();
    const telegram = makeTelegram();

    await handleWatcherCallback(
      "watcher:done:obligation-456",
      db as never,
      telegram as never,
      99,
      "chat-123",
      "cq-id-1",
    );

    // answerCallbackQuery called first
    const answerCalls = (telegram.answerCallbackQuery as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(answerCalls.length, 1);

    // DB updated with status = 'done'
    const queryCalls = (db.query as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(queryCalls.length, 1);
    const sql: string = queryCalls[0]?.arguments[0] as string;
    assert.ok(sql.includes("status = 'done'"), "SQL should set status=done");
    const params = queryCalls[0]?.arguments[1] as unknown[];
    assert.equal(params?.[1], "obligation-456", "Should use correct obligation ID");

    // Message edited with confirmation
    const editCalls = (telegram.editMessage as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(editCalls.length, 1);
    assert.equal(editCalls[0]?.arguments[2], "Obligation marked done.");
  });

  it("watcher:snooze:{id} advances updated_at by 24h and edits message", async () => {
    const { handleWatcherCallback } = await import(
      "../src/features/watcher/callbacks.js"
    );

    const db = makeDb();
    const telegram = makeTelegram();
    const before = Date.now();

    await handleWatcherCallback(
      "watcher:snooze:obligation-789",
      db as never,
      telegram as never,
      100,
      "chat-456",
      "cq-id-2",
    );

    const after = Date.now();

    const queryCalls = (db.query as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(queryCalls.length, 1);
    const params = queryCalls[0]?.arguments[1] as unknown[];
    const snoozeDate = params?.[0] as Date;
    assert.ok(snoozeDate instanceof Date, "First param should be a Date");

    const snoozeMs = snoozeDate.getTime();
    const expectedMin = before + 24 * 60 * 60 * 1000;
    const expectedMax = after + 24 * 60 * 60 * 1000;
    assert.ok(
      snoozeMs >= expectedMin && snoozeMs <= expectedMax,
      "Snooze date should be ~24h from now",
    );

    const editCalls = (telegram.editMessage as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(editCalls[0]?.arguments[2], "Obligation snoozed for 24 hours.");
  });

  it("watcher:dismiss:{id} sets status cancelled and edits message", async () => {
    const { handleWatcherCallback } = await import(
      "../src/features/watcher/callbacks.js"
    );

    const db = makeDb();
    const telegram = makeTelegram();

    await handleWatcherCallback(
      "watcher:dismiss:obligation-321",
      db as never,
      telegram as never,
      101,
      "chat-789",
      "cq-id-3",
    );

    const queryCalls = (db.query as ReturnType<typeof mock.fn>).mock.calls;
    const sql: string = queryCalls[0]?.arguments[0] as string;
    assert.ok(sql.includes("status = 'cancelled'"), "SQL should set status=cancelled");

    const editCalls = (telegram.editMessage as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(editCalls[0]?.arguments[2], "Obligation dismissed.");
  });

  it("unknown prefix is a no-op (no DB call, no edit)", async () => {
    const { handleWatcherCallback } = await import(
      "../src/features/watcher/callbacks.js"
    );

    const db = makeDb();
    const telegram = makeTelegram();

    await handleWatcherCallback(
      "unknown:prefix:abc",
      db as never,
      telegram as never,
      102,
      "chat-000",
      "cq-id-4",
    );

    // answerCallbackQuery is still called (before prefix check)
    const answerCalls = (telegram.answerCallbackQuery as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(answerCalls.length, 1, "answerCallbackQuery should still be called");

    // No DB operations
    const queryCalls = (db.query as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(queryCalls.length, 0, "No DB calls for unknown prefix");

    // No message edit
    const editCalls = (telegram.editMessage as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(editCalls.length, 0, "No editMessage for unknown prefix");
  });
});
