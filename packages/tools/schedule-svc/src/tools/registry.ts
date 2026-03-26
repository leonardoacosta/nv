import { ToolRegistry, type ToolDefinition } from "../tool-registry.js";
import { setReminder, cancelReminder, listReminders } from "./reminders.js";
import { addSchedule, modifySchedule, removeSchedule, listSchedules } from "./schedules.js";
import { startSession, stopSession } from "./sessions.js";

// --- Reminder tools ---

const setReminderTool: ToolDefinition = {
  name: "set_reminder",
  description: "Set a one-shot reminder that fires at a specified time",
  inputSchema: {
    type: "object",
    properties: {
      description: { type: "string", description: "What the reminder is about" },
      due_at: { type: "string", description: "ISO 8601 timestamp when the reminder should fire" },
    },
    required: ["description", "due_at"],
    additionalProperties: false,
  },
  handler: async (input) => setReminder(input as { description: string; due_at: string }),
};

const cancelReminderTool: ToolDefinition = {
  name: "cancel_reminder",
  description: "Cancel a pending reminder by its ID",
  inputSchema: {
    type: "object",
    properties: {
      id: { type: "string", description: "UUID of the reminder to cancel" },
    },
    required: ["id"],
    additionalProperties: false,
  },
  handler: async (input) => cancelReminder(input as { id: string }),
};

const listRemindersTool: ToolDefinition = {
  name: "list_reminders",
  description: "List reminders. By default only active (not cancelled, not delivered) reminders are shown.",
  inputSchema: {
    type: "object",
    properties: {
      status: {
        type: "string",
        enum: ["active", "all"],
        description: "Filter: 'active' (default) shows pending only, 'all' shows everything",
      },
    },
    additionalProperties: false,
  },
  handler: async (input) => listReminders(input as { status?: "active" | "all" }),
};

// --- Schedule tools ---

const addScheduleTool: ToolDefinition = {
  name: "add_schedule",
  description: "Create a recurring schedule with a cron expression",
  inputSchema: {
    type: "object",
    properties: {
      name: { type: "string", description: "Unique name for the schedule" },
      cron: { type: "string", description: "5-field standard cron expression (minute hour day-of-month month day-of-week)" },
      action: { type: "string", description: "Action to execute on each trigger" },
    },
    required: ["name", "cron", "action"],
    additionalProperties: false,
  },
  handler: async (input) => addSchedule(input as { name: string; cron: string; action: string }),
};

const modifyScheduleTool: ToolDefinition = {
  name: "modify_schedule",
  description: "Update an existing schedule's name, cron expression, action, or enabled state",
  inputSchema: {
    type: "object",
    properties: {
      id: { type: "string", description: "UUID of the schedule to modify" },
      updates: {
        type: "object",
        properties: {
          name: { type: "string" },
          cron: { type: "string" },
          action: { type: "string" },
          enabled: { type: "boolean" },
        },
        additionalProperties: false,
      },
    },
    required: ["id", "updates"],
    additionalProperties: false,
  },
  handler: async (input) =>
    modifySchedule(
      input as {
        id: string;
        updates: { name?: string; cron?: string; action?: string; enabled?: boolean };
      },
    ),
};

const removeScheduleTool: ToolDefinition = {
  name: "remove_schedule",
  description: "Delete a schedule by its ID",
  inputSchema: {
    type: "object",
    properties: {
      id: { type: "string", description: "UUID of the schedule to remove" },
    },
    required: ["id"],
    additionalProperties: false,
  },
  handler: async (input) => removeSchedule(input as { id: string }),
};

const listSchedulesTool: ToolDefinition = {
  name: "list_schedules",
  description: "List schedules. By default only enabled schedules are shown.",
  inputSchema: {
    type: "object",
    properties: {
      active: {
        type: "boolean",
        description: "When true (default), show only enabled schedules. When false, show all.",
      },
    },
    additionalProperties: false,
  },
  handler: async (input) => listSchedules(input as { active?: boolean }),
};

// --- Session tools ---

const startSessionTool: ToolDefinition = {
  name: "start_session",
  description: "Record the start of a Claude Code session for a project",
  inputSchema: {
    type: "object",
    properties: {
      name: { type: "string", description: "Project name for the session" },
      metadata: {
        type: "object",
        description: "Optional metadata about the session",
        additionalProperties: true,
      },
    },
    required: ["name"],
    additionalProperties: false,
  },
  handler: async (input) =>
    startSession(input as { name: string; metadata?: Record<string, unknown> }),
};

const stopSessionTool: ToolDefinition = {
  name: "stop_session",
  description: "Stop the most recent running session. Optionally filter by project name.",
  inputSchema: {
    type: "object",
    properties: {
      name: {
        type: "string",
        description: "Project name to match. If omitted, stops the most recent running session across all projects.",
      },
    },
    additionalProperties: false,
  },
  handler: async (input) => stopSession(input as { name?: string }),
};

// --- Build registry ---

export function buildRegistry(): ToolRegistry {
  const registry = new ToolRegistry();

  registry.register(setReminderTool);
  registry.register(cancelReminderTool);
  registry.register(listRemindersTool);
  registry.register(addScheduleTool);
  registry.register(modifyScheduleTool);
  registry.register(removeScheduleTool);
  registry.register(listSchedulesTool);
  registry.register(startSessionTool);
  registry.register(stopSessionTool);

  return registry;
}
