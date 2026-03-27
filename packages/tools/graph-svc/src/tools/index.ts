import type { ToolDefinition } from "../tools.js";
import type { ServiceConfig } from "../config.js";
import { calendarToday, calendarUpcoming, calendarNext } from "./calendar.js";
import { adoProjects, adoPipelines, adoBuilds } from "./ado.js";
import { outlookInbox, outlookRead, outlookSearch } from "./mail.js";

export function registerGraphTools(
  config: ServiceConfig,
): ToolDefinition[] {
  return [
    // ── Calendar Tools ──────────────────────────────────────────────
    {
      name: "calendar_today",
      description:
        "Get today's Outlook calendar events. Returns event titles, times, and attendees. Authenticated and ready to use.",
      inputSchema: {
        type: "object",
        properties: {},
        required: [],
        additionalProperties: false,
      },
      handler: async () => calendarToday(config),
    },
    {
      name: "calendar_upcoming",
      description:
        "Get upcoming Outlook calendar events for the next N days (default 7). Returns event titles, times, and attendees.",
      inputSchema: {
        type: "object",
        properties: {
          days: {
            type: "integer",
            description:
              "Number of days to look ahead (1-14, default 7).",
            minimum: 1,
            maximum: 14,
          },
        },
        required: [],
        additionalProperties: false,
      },
      handler: async (input) => {
        const days = typeof input["days"] === "number" ? input["days"] : 7;
        return calendarUpcoming(config, days);
      },
    },
    {
      name: "calendar_next",
      description:
        "Get the next upcoming Outlook calendar event. Returns title, time, and attendees.",
      inputSchema: {
        type: "object",
        properties: {},
        required: [],
        additionalProperties: false,
      },
      handler: async () => calendarNext(config),
    },

    // ── ADO Tools ───────────────────────────────────────────────────
    {
      name: "ado_projects",
      description: "List Azure DevOps projects you have access to. Returns project names and IDs.",
      inputSchema: {
        type: "object",
        properties: {},
        required: [],
        additionalProperties: false,
      },
      handler: async () => adoProjects(config),
    },
    {
      name: "ado_pipelines",
      description:
        "List Azure DevOps pipelines, optionally filtered by project name. Returns pipeline names and IDs.",
      inputSchema: {
        type: "object",
        properties: {
          project: {
            type: "string",
            description:
              "Project name to filter pipelines by. If omitted, lists all.",
          },
        },
        required: [],
        additionalProperties: false,
      },
      handler: async (input) => {
        const project =
          typeof input["project"] === "string"
            ? input["project"]
            : undefined;
        return adoPipelines(config, project);
      },
    },
    {
      name: "ado_builds",
      description:
        "List recent Azure DevOps builds with status, optionally filtered by project and pipeline.",
      inputSchema: {
        type: "object",
        properties: {
          project: {
            type: "string",
            description: "Project name to filter builds by.",
          },
          pipeline: {
            type: "string",
            description: "Pipeline name to filter builds by.",
          },
          limit: {
            type: "integer",
            description:
              "Maximum number of builds to return (1-50, default 10).",
            minimum: 1,
            maximum: 50,
          },
        },
        required: [],
        additionalProperties: false,
      },
      handler: async (input) => {
        const project =
          typeof input["project"] === "string"
            ? input["project"]
            : undefined;
        const pipeline =
          typeof input["pipeline"] === "string"
            ? input["pipeline"]
            : undefined;
        const limit =
          typeof input["limit"] === "number" ? input["limit"] : 10;
        return adoBuilds(config, project, pipeline, limit);
      },
    },

    // ── Mail Tools ─────────────────────────────────────────────────
    {
      name: "outlook_inbox",
      description:
        "Get recent emails from Outlook inbox. Returns subject, sender, date, and preview. Authenticated and ready.",
      inputSchema: {
        type: "object",
        properties: {
          limit: {
            type: "integer",
            description:
              "Number of emails to return (1-50, default 10).",
            minimum: 1,
            maximum: 50,
          },
        },
        required: [],
        additionalProperties: false,
      },
      handler: async (input) => {
        const limit =
          typeof input["limit"] === "number" ? input["limit"] : 10;
        return outlookInbox(config, limit);
      },
    },
    {
      name: "outlook_read",
      description:
        "Read the full content of an email by message ID.",
      inputSchema: {
        type: "object",
        properties: {
          message_id: {
            type: "string",
            description: "The Outlook message ID to read.",
          },
        },
        required: ["message_id"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const messageId = input["message_id"];
        if (typeof messageId !== "string" || !messageId) {
          throw new Error("message_id is required");
        }
        return outlookRead(config, messageId);
      },
    },
    {
      name: "outlook_search",
      description:
        "Search Outlook emails by keyword. Returns matching emails with subject, sender, and date.",
      inputSchema: {
        type: "object",
        properties: {
          query: {
            type: "string",
            description: "Search keyword or phrase.",
          },
          limit: {
            type: "integer",
            description:
              "Number of results to return (1-50, default 10).",
            minimum: 1,
            maximum: 50,
          },
        },
        required: ["query"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const query = input["query"];
        if (typeof query !== "string" || !query) {
          throw new Error("query is required");
        }
        const limit =
          typeof input["limit"] === "number" ? input["limit"] : 10;
        return outlookSearch(config, query, limit);
      },
    },
  ];
}
