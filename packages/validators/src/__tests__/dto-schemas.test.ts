import { describe, test, expect } from "vitest";
import { z } from "zod/v4";
import {
  // Common schemas
  paginationSchema,
  cursorPaginationSchema,
  sortOrderSchema,
  dateRangeSchema,
  uuidParamSchema,
  // Project DTOs
  createProjectSchema,
  updateProjectSchema,
  projectCategoryEnum,
  projectStatusEnum,
  // Obligation DTOs
  createObligationSchema,
  updateObligationSchema,
  obligationFilterSchema,
  obligationStatusEnum,
  // Contact DTOs
  createContactSchema,
  updateContactSchema,
  // Message DTOs
  createMessageSchema,
  messageFilterSchema,
  // Memory DTOs
  createMemorySchema,
  updateMemorySchema,
  // Reminder DTOs
  createReminderSchema,
  updateReminderSchema,
  // Schedule DTOs
  createScheduleSchema,
  updateScheduleSchema,
  // Session DTOs
  createSessionSchema,
  sessionFilterSchema,
  // Briefing DTOs
  createBriefingSchema,
  // Settings DTOs
  upsertSettingSchema,
} from "../index.js";

// ─── Common schemas ─────────────────────────────────────────────────────────

describe("paginationSchema", () => {
  test("applies defaults when no input provided", () => {
    const result = paginationSchema.parse({});
    expect(result).toEqual({ limit: 20, offset: 0 });
  });

  test("accepts valid input", () => {
    const result = paginationSchema.parse({ limit: 50, offset: 10 });
    expect(result).toEqual({ limit: 50, offset: 10 });
  });

  test("rejects limit below 1", () => {
    expect(() => paginationSchema.parse({ limit: 0 })).toThrow();
  });

  test("rejects limit above 100", () => {
    expect(() => paginationSchema.parse({ limit: 101 })).toThrow();
  });

  test("rejects negative offset", () => {
    expect(() => paginationSchema.parse({ offset: -1 })).toThrow();
  });

  test("rejects non-integer limit", () => {
    expect(() => paginationSchema.parse({ limit: 1.5 })).toThrow();
  });
});

describe("cursorPaginationSchema", () => {
  test("applies default limit", () => {
    const result = cursorPaginationSchema.parse({});
    expect(result).toEqual({ limit: 20 });
  });

  test("accepts valid cursor UUID", () => {
    const result = cursorPaginationSchema.parse({
      cursor: "550e8400-e29b-41d4-a716-446655440000",
      limit: 10,
    });
    expect(result.cursor).toBe("550e8400-e29b-41d4-a716-446655440000");
  });

  test("rejects invalid cursor format", () => {
    expect(() =>
      cursorPaginationSchema.parse({ cursor: "not-a-uuid" }),
    ).toThrow();
  });
});

describe("sortOrderSchema", () => {
  test("defaults to desc", () => {
    const result = sortOrderSchema.parse(undefined);
    expect(result).toBe("desc");
  });

  test("accepts asc", () => {
    expect(sortOrderSchema.parse("asc")).toBe("asc");
  });

  test("rejects invalid values", () => {
    expect(() => sortOrderSchema.parse("random")).toThrow();
  });
});

describe("dateRangeSchema", () => {
  test("accepts empty object", () => {
    const result = dateRangeSchema.parse({});
    expect(result).toEqual({});
  });

  test("coerces string dates", () => {
    const result = dateRangeSchema.parse({
      from: "2024-01-01",
      to: "2024-12-31",
    });
    expect(result.from).toBeInstanceOf(Date);
    expect(result.to).toBeInstanceOf(Date);
  });
});

describe("uuidParamSchema", () => {
  test("accepts valid UUID", () => {
    const result = uuidParamSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
    });
    expect(result.id).toBe("550e8400-e29b-41d4-a716-446655440000");
  });

  test("rejects missing id", () => {
    expect(() => uuidParamSchema.parse({})).toThrow();
  });

  test("rejects invalid UUID", () => {
    expect(() => uuidParamSchema.parse({ id: "not-a-uuid" })).toThrow();
  });
});

// ─── Project DTOs ───────────────────────────────────────────────────────────

describe("createProjectSchema", () => {
  test("valid input passes", () => {
    const result = createProjectSchema.parse({
      code: "nv",
      name: "Nova",
    });
    expect(result.code).toBe("nv");
    expect(result.name).toBe("Nova");
  });

  test("accepts all optional fields", () => {
    const result = createProjectSchema.parse({
      code: "nv",
      name: "Nova",
      category: "work",
      status: "active",
      description: "An assistant",
      content: "markdown content",
      path: "/home/user/dev/nv",
    });
    expect(result.category).toBe("work");
    expect(result.status).toBe("active");
  });

  test("rejects missing required field: code", () => {
    expect(() => createProjectSchema.parse({ name: "Nova" })).toThrow();
  });

  test("rejects missing required field: name", () => {
    expect(() => createProjectSchema.parse({ code: "nv" })).toThrow();
  });

  test("rejects empty code", () => {
    expect(() =>
      createProjectSchema.parse({ code: "", name: "Nova" }),
    ).toThrow();
  });

  test("rejects invalid category", () => {
    expect(() =>
      createProjectSchema.parse({
        code: "nv",
        name: "Nova",
        category: "invalid",
      }),
    ).toThrow();
  });

  test("rejects invalid status", () => {
    expect(() =>
      createProjectSchema.parse({
        code: "nv",
        name: "Nova",
        status: "invalid",
      }),
    ).toThrow();
  });
});

describe("updateProjectSchema", () => {
  test("accepts partial updates", () => {
    const result = updateProjectSchema.parse({ name: "Nova v2" });
    expect(result.name).toBe("Nova v2");
  });

  test("accepts empty object (all optional)", () => {
    const result = updateProjectSchema.parse({});
    expect(result).toEqual({});
  });

  test("accepts category update alone", () => {
    const result = updateProjectSchema.parse({ category: "personal" });
    expect(result.category).toBe("personal");
  });

  test("rejects invalid category on update", () => {
    expect(() =>
      updateProjectSchema.parse({ category: "invalid" }),
    ).toThrow();
  });
});

describe("projectCategoryEnum", () => {
  test("accepts valid categories", () => {
    for (const cat of ["work", "personal", "open_source", "archived"]) {
      expect(projectCategoryEnum.parse(cat)).toBe(cat);
    }
  });

  test("rejects invalid category", () => {
    expect(() => projectCategoryEnum.parse("hobby")).toThrow();
  });
});

describe("projectStatusEnum", () => {
  test("accepts valid statuses", () => {
    for (const s of ["active", "paused", "completed", "archived"]) {
      expect(projectStatusEnum.parse(s)).toBe(s);
    }
  });

  test("rejects invalid status", () => {
    expect(() => projectStatusEnum.parse("deleted")).toThrow();
  });
});

// ─── Obligation DTOs ────────────────────────────────────────────────────────

describe("createObligationSchema", () => {
  test("valid input with defaults applied", () => {
    const result = createObligationSchema.parse({
      detectedAction: "Review PR #42",
    });
    expect(result.detectedAction).toBe("Review PR #42");
    expect(result.status).toBe("open");
    expect(result.priority).toBe(2);
    expect(result.owner).toBe("nova");
    expect(result.sourceChannel).toBe("dashboard");
  });

  test("rejects missing detectedAction", () => {
    expect(() => createObligationSchema.parse({})).toThrow();
  });

  test("accepts custom priority", () => {
    const result = createObligationSchema.parse({
      detectedAction: "Fix bug",
      priority: 0,
    });
    expect(result.priority).toBe(0);
  });

  test("rejects priority above 4", () => {
    expect(() =>
      createObligationSchema.parse({
        detectedAction: "Fix bug",
        priority: 5,
      }),
    ).toThrow();
  });

  test("rejects priority below 0", () => {
    expect(() =>
      createObligationSchema.parse({
        detectedAction: "Fix bug",
        priority: -1,
      }),
    ).toThrow();
  });
});

describe("updateObligationSchema", () => {
  test("accepts partial updates", () => {
    const result = updateObligationSchema.parse({ status: "done" });
    expect(result.status).toBe("done");
  });

  test("accepts empty object (defaults from create schema carry over)", () => {
    const result = updateObligationSchema.parse({});
    // Defaults from createObligationSchema.partial() still apply for
    // fields that had .default() — status, priority, owner
    expect(result.status).toBe("open");
    expect(result.priority).toBe(2);
    expect(result.owner).toBe("nova");
  });

  test("omits detectedAction and sourceChannel from updates", () => {
    // These fields are omitted from the update schema
    const shape = updateObligationSchema._zod.def.shape;
    expect(shape).not.toHaveProperty("detectedAction");
    expect(shape).not.toHaveProperty("sourceChannel");
  });
});

describe("obligationFilterSchema", () => {
  test("accepts empty filter", () => {
    const result = obligationFilterSchema.parse({});
    expect(result).toEqual({});
  });

  test("accepts status filter", () => {
    const result = obligationFilterSchema.parse({ status: "open" });
    expect(result.status).toBe("open");
  });

  test("accepts owner filter", () => {
    const result = obligationFilterSchema.parse({ owner: "nova" });
    expect(result.owner).toBe("nova");
  });
});

// ─── Contact DTOs ───────────────────────────────────────────────────────────

describe("createContactSchema", () => {
  test("valid input passes", () => {
    const result = createContactSchema.parse({
      name: "Alice",
      channelIds: { discord: "12345", slack: "U789" },
    });
    expect(result.name).toBe("Alice");
    expect(result.channelIds).toEqual({ discord: "12345", slack: "U789" });
  });

  test("rejects missing name", () => {
    expect(() =>
      createContactSchema.parse({ channelIds: { discord: "12345" } }),
    ).toThrow();
  });

  test("rejects missing channelIds", () => {
    expect(() => createContactSchema.parse({ name: "Alice" })).toThrow();
  });

  test("rejects empty name", () => {
    expect(() =>
      createContactSchema.parse({
        name: "",
        channelIds: { discord: "12345" },
      }),
    ).toThrow();
  });
});

describe("updateContactSchema", () => {
  test("accepts partial updates", () => {
    const result = updateContactSchema.parse({ name: "Bob" });
    expect(result.name).toBe("Bob");
  });

  test("accepts empty object", () => {
    const result = updateContactSchema.parse({});
    expect(result).toEqual({});
  });
});

// ─── Message DTOs ───────────────────────────────────────────────────────────

describe("createMessageSchema", () => {
  test("valid input passes", () => {
    const result = createMessageSchema.parse({
      channel: "discord",
      content: "Hello world",
    });
    expect(result.channel).toBe("discord");
    expect(result.content).toBe("Hello world");
  });

  test("rejects missing channel", () => {
    expect(() =>
      createMessageSchema.parse({ content: "Hello" }),
    ).toThrow();
  });

  test("rejects missing content", () => {
    expect(() =>
      createMessageSchema.parse({ channel: "discord" }),
    ).toThrow();
  });
});

describe("messageFilterSchema", () => {
  test("accepts empty filter", () => {
    const result = messageFilterSchema.parse({});
    expect(result).toEqual({});
  });

  test("accepts channel filter", () => {
    const result = messageFilterSchema.parse({ channel: "discord" });
    expect(result.channel).toBe("discord");
  });

  test("accepts dateRange filter", () => {
    const result = messageFilterSchema.parse({
      dateRange: { from: "2024-01-01" },
    });
    expect(result.dateRange?.from).toBeInstanceOf(Date);
  });
});

// ─── Memory DTOs ────────────────────────────────────────────────────────────

describe("createMemorySchema", () => {
  test("valid input passes", () => {
    const result = createMemorySchema.parse({
      topic: "user-preferences",
      content: "Prefers dark theme",
    });
    expect(result.topic).toBe("user-preferences");
    expect(result.content).toBe("Prefers dark theme");
  });

  test("rejects missing topic", () => {
    expect(() =>
      createMemorySchema.parse({ content: "Something" }),
    ).toThrow();
  });

  test("rejects empty topic", () => {
    expect(() =>
      createMemorySchema.parse({ topic: "", content: "Something" }),
    ).toThrow();
  });

  test("rejects empty content", () => {
    expect(() =>
      createMemorySchema.parse({ topic: "test", content: "" }),
    ).toThrow();
  });
});

describe("updateMemorySchema", () => {
  test("accepts content update", () => {
    const result = updateMemorySchema.parse({ content: "Updated content" });
    expect(result.content).toBe("Updated content");
  });

  test("rejects empty content", () => {
    expect(() => updateMemorySchema.parse({ content: "" })).toThrow();
  });

  test("rejects missing content", () => {
    expect(() => updateMemorySchema.parse({})).toThrow();
  });
});

// ─── Reminder DTOs ──────────────────────────────────────────────────────────

describe("createReminderSchema", () => {
  test("valid input passes", () => {
    const result = createReminderSchema.parse({
      message: "Stand up",
      dueAt: "2024-12-01T09:00:00Z",
      channel: "discord",
    });
    expect(result.message).toBe("Stand up");
    expect(result.dueAt).toBeInstanceOf(Date);
    expect(result.channel).toBe("discord");
  });

  test("rejects missing message", () => {
    expect(() =>
      createReminderSchema.parse({
        dueAt: "2024-12-01T09:00:00Z",
        channel: "discord",
      }),
    ).toThrow();
  });

  test("rejects missing dueAt", () => {
    expect(() =>
      createReminderSchema.parse({
        message: "Stand up",
        channel: "discord",
      }),
    ).toThrow();
  });

  test("rejects missing channel", () => {
    expect(() =>
      createReminderSchema.parse({
        message: "Stand up",
        dueAt: "2024-12-01T09:00:00Z",
      }),
    ).toThrow();
  });
});

describe("updateReminderSchema", () => {
  test("accepts partial updates", () => {
    const result = updateReminderSchema.parse({ message: "Updated reminder" });
    expect(result.message).toBe("Updated reminder");
  });

  test("accepts cancelled flag", () => {
    const result = updateReminderSchema.parse({ cancelled: true });
    expect(result.cancelled).toBe(true);
  });

  test("accepts empty object", () => {
    const result = updateReminderSchema.parse({});
    expect(result).toEqual({});
  });
});

// ─── Schedule DTOs ──────────────────────────────────────────────────────────

describe("createScheduleSchema", () => {
  test("valid input passes", () => {
    const result = createScheduleSchema.parse({
      name: "Daily standup",
      cronExpr: "0 9 * * *",
      action: "send-briefing",
      channel: "discord",
    });
    expect(result.name).toBe("Daily standup");
    expect(result.enabled).toBe(true); // default
  });

  test("rejects missing required fields", () => {
    expect(() =>
      createScheduleSchema.parse({ name: "Test" }),
    ).toThrow();
  });

  test("accepts enabled=false override", () => {
    const result = createScheduleSchema.parse({
      name: "Disabled schedule",
      cronExpr: "0 9 * * *",
      action: "test",
      channel: "discord",
      enabled: false,
    });
    expect(result.enabled).toBe(false);
  });
});

describe("updateScheduleSchema", () => {
  test("accepts partial updates", () => {
    const result = updateScheduleSchema.parse({ name: "Updated schedule" });
    expect(result.name).toBe("Updated schedule");
  });

  test("accepts empty object", () => {
    const result = updateScheduleSchema.parse({});
    expect(result).toEqual({});
  });

  test("accepts enabled toggle", () => {
    const result = updateScheduleSchema.parse({ enabled: false });
    expect(result.enabled).toBe(false);
  });
});

// ─── Session DTOs ───────────────────────────────────────────────────────────

describe("createSessionSchema", () => {
  test("valid input passes", () => {
    const result = createSessionSchema.parse({
      project: "nv",
      command: "/apply",
    });
    expect(result.project).toBe("nv");
    expect(result.command).toBe("/apply");
  });

  test("rejects missing project", () => {
    expect(() =>
      createSessionSchema.parse({ command: "/apply" }),
    ).toThrow();
  });

  test("rejects missing command", () => {
    expect(() =>
      createSessionSchema.parse({ project: "nv" }),
    ).toThrow();
  });

  test("rejects empty project", () => {
    expect(() =>
      createSessionSchema.parse({ project: "", command: "/apply" }),
    ).toThrow();
  });
});

describe("sessionFilterSchema", () => {
  test("accepts empty filter", () => {
    const result = sessionFilterSchema.parse({});
    expect(result).toEqual({});
  });

  test("accepts project filter", () => {
    const result = sessionFilterSchema.parse({ project: "nv" });
    expect(result.project).toBe("nv");
  });

  test("accepts dateRange filter", () => {
    const result = sessionFilterSchema.parse({
      dateRange: { from: "2024-01-01", to: "2024-12-31" },
    });
    expect(result.dateRange?.from).toBeInstanceOf(Date);
    expect(result.dateRange?.to).toBeInstanceOf(Date);
  });
});

// ─── Briefing DTOs ──────────────────────────────────────────────────────────

describe("createBriefingSchema", () => {
  test("valid input passes", () => {
    const result = createBriefingSchema.parse({
      content: "Good morning. Here is your briefing.",
    });
    expect(result.content).toBe("Good morning. Here is your briefing.");
  });

  test("rejects empty content", () => {
    expect(() =>
      createBriefingSchema.parse({ content: "" }),
    ).toThrow();
  });

  test("rejects missing content", () => {
    expect(() => createBriefingSchema.parse({})).toThrow();
  });
});

// ─── Settings DTOs ──────────────────────────────────────────────────────────

describe("upsertSettingSchema", () => {
  test("valid input passes", () => {
    const result = upsertSettingSchema.parse({
      key: "theme",
      value: "dark",
    });
    expect(result.key).toBe("theme");
    expect(result.value).toBe("dark");
  });

  test("rejects missing key", () => {
    expect(() => upsertSettingSchema.parse({ value: "dark" })).toThrow();
  });

  test("rejects missing value", () => {
    expect(() => upsertSettingSchema.parse({ key: "theme" })).toThrow();
  });

  test("rejects empty key", () => {
    expect(() =>
      upsertSettingSchema.parse({ key: "", value: "dark" }),
    ).toThrow();
  });

  test("rejects empty value", () => {
    expect(() =>
      upsertSettingSchema.parse({ key: "theme", value: "" }),
    ).toThrow();
  });
});
