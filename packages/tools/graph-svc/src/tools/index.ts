import type { ToolDefinition } from "../tools.js";
import type { ServiceConfig } from "../config.js";
import { calendarToday, calendarUpcoming, calendarNext } from "./calendar.js";
import { adoProjects, adoPipelines, adoBuilds } from "./ado.js";

export function registerGraphTools(
  config: ServiceConfig,
): ToolDefinition[] {
  return [
    // ── Calendar Tools ──────────────────────────────────────────────
    {
      name: "calendar_today",
      description:
        "Get today's calendar events from Outlook via the CloudPC.",
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
        "Get upcoming calendar events from Outlook for the next N days.",
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
        "Get the next upcoming calendar event from Outlook via the CloudPC.",
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
      description: "List Azure DevOps projects.",
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
        "List Azure DevOps pipelines, optionally filtered by project.",
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
        "Get recent Azure DevOps builds, optionally filtered by project and pipeline.",
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
  ];
}
