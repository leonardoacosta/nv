import type { ToolDefinition } from "../tools.js";
import type { ServiceConfig } from "../config.js";
import { calendarToday, calendarUpcoming, calendarNext } from "./calendar.js";
import { adoProjects, adoPipelines, adoBuilds } from "./ado.js";
import { adoWorkItems, adoRepos, adoPullRequests, adoBuildLogs, adoCommits, adoPipelineDefinition, adoPipelineUpdate, adoRepoUpdate, adoPipelineRun, adoPipelineVariables, adoBranches, adoRepoDelete } from "./ado-extended.js";
import { outlookInbox, outlookRead, outlookSearch, outlookFolders, outlookSent, outlookFolder, outlookFlag, outlookMove, outlookUnread } from "./mail.js";
import { pimStatus, pimActivate, pimActivateAll } from "./pim.js";

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

    // ── PIM Tools ──────────────────────────────────────────────────
    {
      name: "pim_status",
      description:
        "List all PIM-eligible Azure roles with activation status. Shows both direct and group-based assignments.",
      inputSchema: {
        type: "object",
        properties: {},
        required: [],
        additionalProperties: false,
      },
      handler: async () => pimStatus(config),
    },
    {
      name: "pim_activate",
      description:
        "Activate a specific PIM role by number. Run pim_status first to see available roles. Requires operator confirmation.",
      inputSchema: {
        type: "object",
        properties: {
          role_number: {
            type: "integer",
            description: "The role number from pim_status output.",
          },
          justification: {
            type: "string",
            description: "Optional justification for the activation.",
          },
        },
        required: ["role_number"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const roleNumber = input["role_number"];
        if (typeof roleNumber !== "number") {
          throw new Error("role_number is required and must be a number");
        }
        const justification =
          typeof input["justification"] === "string"
            ? input["justification"]
            : undefined;
        return pimActivate(config, roleNumber, justification);
      },
    },
    {
      name: "pim_activate_all",
      description:
        "Activate all PIM-eligible Azure roles at once. Requires operator confirmation.",
      inputSchema: {
        type: "object",
        properties: {
          justification: {
            type: "string",
            description: "Optional justification for the activation.",
          },
        },
        required: [],
        additionalProperties: false,
      },
      handler: async (input) => {
        const justification =
          typeof input["justification"] === "string"
            ? input["justification"]
            : undefined;
        return pimActivateAll(config, justification);
      },
    },

    // ── ADO Extended Tools ──────────────────────────────────────────
    {
      name: "ado_work_items",
      description:
        "Query Azure DevOps work items. Filter by project, state (Active/New/Closed), and type (Bug/Task/User Story).",
      inputSchema: {
        type: "object",
        properties: {
          project: {
            type: "string",
            description: "Project name to filter by.",
          },
          state: {
            type: "string",
            description:
              "Work item state filter (Active, New, Closed, etc.).",
          },
          type: {
            type: "string",
            description:
              "Work item type filter (Bug, Task, User Story, etc.).",
          },
          limit: {
            type: "integer",
            description:
              "Maximum number of results to return (1-50, default 20).",
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
        const state =
          typeof input["state"] === "string" ? input["state"] : undefined;
        const type =
          typeof input["type"] === "string" ? input["type"] : undefined;
        const limit =
          typeof input["limit"] === "number" ? input["limit"] : 20;
        return adoWorkItems(config, project, state, type, limit);
      },
    },
    {
      name: "ado_repos",
      description: "List Azure DevOps repositories in a project.",
      inputSchema: {
        type: "object",
        properties: {
          project: {
            type: "string",
            description: "Project name to filter by.",
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
        return adoRepos(config, project);
      },
    },
    {
      name: "ado_pull_requests",
      description:
        "List Azure DevOps pull requests. Filter by status (active/completed/abandoned).",
      inputSchema: {
        type: "object",
        properties: {
          project: {
            type: "string",
            description: "Project name to filter by.",
          },
          status: {
            type: "string",
            description:
              "PR status filter (active, completed, abandoned).",
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
        const status =
          typeof input["status"] === "string"
            ? input["status"]
            : undefined;
        return adoPullRequests(config, project, status);
      },
    },
    {
      name: "ado_build_logs",
      description:
        "Get details and logs for a specific Azure DevOps build run.",
      inputSchema: {
        type: "object",
        properties: {
          build_id: {
            type: "integer",
            description: "The build ID to look up.",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
        },
        required: ["build_id"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const buildId = input["build_id"];
        if (typeof buildId !== "number") {
          throw new Error("build_id is required and must be a number");
        }
        const project =
          typeof input["project"] === "string"
            ? input["project"]
            : undefined;
        return adoBuildLogs(config, buildId, project);
      },
    },

    // ── ADO Git & Pipeline Management Tools ────────────────────────
    {
      name: "ado_commits",
      description:
        "Get recent commits from an Azure DevOps Git repository. Useful for contributor analysis and activity tracking.",
      inputSchema: {
        type: "object",
        properties: {
          repo: {
            type: "string",
            description: "Repository name.",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
          limit: {
            type: "integer",
            description: "Maximum number of commits (1-50, default 20).",
            minimum: 1,
            maximum: 50,
          },
        },
        required: ["repo"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const repo = input["repo"];
        if (typeof repo !== "string" || !repo) {
          throw new Error("repo is required");
        }
        const project =
          typeof input["project"] === "string" ? input["project"] : undefined;
        const limit =
          typeof input["limit"] === "number" ? input["limit"] : 20;
        return adoCommits(config, repo, project, limit);
      },
    },
    {
      name: "ado_pipeline_definition",
      description:
        "Get pipeline definition details: triggers, variables, YAML path, default branch.",
      inputSchema: {
        type: "object",
        properties: {
          pipeline_id: {
            type: "integer",
            description: "The pipeline definition ID.",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
        },
        required: ["pipeline_id"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const pipelineId = input["pipeline_id"];
        if (typeof pipelineId !== "number") {
          throw new Error("pipeline_id is required and must be a number");
        }
        const project =
          typeof input["project"] === "string" ? input["project"] : undefined;
        return adoPipelineDefinition(config, pipelineId, project);
      },
    },
    {
      name: "ado_pipeline_update",
      description:
        "Update a pipeline's default branch or settings. Requires operator confirmation.",
      inputSchema: {
        type: "object",
        properties: {
          pipeline_id: {
            type: "integer",
            description: "The pipeline definition ID.",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
          branch: {
            type: "string",
            description: "New default branch (e.g., refs/heads/main).",
          },
        },
        required: ["pipeline_id", "project"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const pipelineId = input["pipeline_id"];
        if (typeof pipelineId !== "number") {
          throw new Error("pipeline_id is required and must be a number");
        }
        const project = input["project"];
        if (typeof project !== "string" || !project) {
          throw new Error("project is required");
        }
        const branch =
          typeof input["branch"] === "string" ? input["branch"] : undefined;
        return adoPipelineUpdate(config, pipelineId, project, branch);
      },
    },
    {
      name: "ado_pipeline_run",
      description:
        "Trigger a pipeline run. Returns the new build run details. Requires operator confirmation.",
      inputSchema: {
        type: "object",
        properties: {
          pipeline_id: {
            type: "integer",
            description: "The pipeline definition ID.",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
          branch: {
            type: "string",
            description: "Branch to build (e.g., refs/heads/main). Uses pipeline default if omitted.",
          },
        },
        required: ["pipeline_id", "project"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const pipelineId = input["pipeline_id"];
        if (typeof pipelineId !== "number") {
          throw new Error("pipeline_id is required and must be a number");
        }
        const project = input["project"];
        if (typeof project !== "string" || !project) {
          throw new Error("project is required");
        }
        const branch =
          typeof input["branch"] === "string" ? input["branch"] : undefined;
        return adoPipelineRun(config, pipelineId, project, branch);
      },
    },
    {
      name: "ado_pipeline_variables",
      description:
        "List variables configured on a pipeline.",
      inputSchema: {
        type: "object",
        properties: {
          pipeline_id: {
            type: "integer",
            description: "The pipeline definition ID.",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
        },
        required: ["pipeline_id"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const pipelineId = input["pipeline_id"];
        if (typeof pipelineId !== "number") {
          throw new Error("pipeline_id is required and must be a number");
        }
        const project =
          typeof input["project"] === "string" ? input["project"] : undefined;
        return adoPipelineVariables(config, pipelineId, project);
      },
    },
    {
      name: "ado_repo_update",
      description:
        "Update repository settings (e.g., set default branch). Requires operator confirmation.",
      inputSchema: {
        type: "object",
        properties: {
          repo: {
            type: "string",
            description: "Repository name.",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
          default_branch: {
            type: "string",
            description: "New default branch (e.g., refs/heads/main).",
          },
        },
        required: ["repo", "project", "default_branch"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const repo = input["repo"];
        if (typeof repo !== "string" || !repo) throw new Error("repo is required");
        const project = input["project"];
        if (typeof project !== "string" || !project) throw new Error("project is required");
        const defaultBranch = input["default_branch"];
        if (typeof defaultBranch !== "string" || !defaultBranch) throw new Error("default_branch is required");
        return adoRepoUpdate(config, repo, project, defaultBranch);
      },
    },
    {
      name: "ado_branches",
      description:
        "List branches in an Azure DevOps Git repository.",
      inputSchema: {
        type: "object",
        properties: {
          repo: {
            type: "string",
            description: "Repository name.",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
        },
        required: ["repo"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const repo = input["repo"];
        if (typeof repo !== "string" || !repo) throw new Error("repo is required");
        const project =
          typeof input["project"] === "string" ? input["project"] : undefined;
        return adoBranches(config, repo, project);
      },
    },
    {
      name: "ado_repo_delete",
      description:
        "Delete an Azure DevOps repository. DESTRUCTIVE — requires operator confirmation.",
      inputSchema: {
        type: "object",
        properties: {
          repo_id: {
            type: "string",
            description: "Repository ID (GUID).",
          },
          project: {
            type: "string",
            description: "Project name.",
          },
        },
        required: ["repo_id", "project"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const repoId = input["repo_id"];
        if (typeof repoId !== "string" || !repoId) throw new Error("repo_id is required");
        const project = input["project"];
        if (typeof project !== "string" || !project) throw new Error("project is required");
        return adoRepoDelete(config, repoId, project);
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
    {
      name: "outlook_folders",
      description:
        "List Outlook mail folders (Inbox, Sent, Drafts, etc.) with folder IDs.",
      inputSchema: {
        type: "object",
        properties: {},
        required: [],
        additionalProperties: false,
      },
      handler: async () => outlookFolders(config),
    },
    {
      name: "outlook_sent",
      description:
        "Get recent sent emails from Outlook. Returns subject, recipient, date, and preview.",
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
        return outlookSent(config, limit);
      },
    },
    {
      name: "outlook_folder",
      description:
        "Read emails from a specific Outlook folder by folder ID. Use outlook_folders to find available folder IDs.",
      inputSchema: {
        type: "object",
        properties: {
          folder_id: {
            type: "string",
            description: "The Outlook folder ID to read from.",
          },
          limit: {
            type: "integer",
            description:
              "Number of emails to return (1-50, default 10).",
            minimum: 1,
            maximum: 50,
          },
        },
        required: ["folder_id"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const folderId = input["folder_id"];
        if (typeof folderId !== "string" || !folderId) {
          throw new Error("folder_id is required");
        }
        const limit =
          typeof input["limit"] === "number" ? input["limit"] : 10;
        return outlookFolder(config, folderId, limit);
      },
    },
    {
      name: "outlook_flag",
      description: "Flag an email for follow-up in Outlook.",
      inputSchema: {
        type: "object",
        properties: {
          message_id: {
            type: "string",
            description: "The Outlook message ID to flag.",
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
        return outlookFlag(config, messageId);
      },
    },
    {
      name: "outlook_move",
      description:
        "Move an email to a different Outlook folder (Archive, Deleted Items, etc.).",
      inputSchema: {
        type: "object",
        properties: {
          message_id: {
            type: "string",
            description: "The Outlook message ID to move.",
          },
          destination_folder: {
            type: "string",
            description:
              "Target folder name (Archive, Deleted Items, etc.).",
          },
        },
        required: ["message_id", "destination_folder"],
        additionalProperties: false,
      },
      handler: async (input) => {
        const messageId = input["message_id"];
        if (typeof messageId !== "string" || !messageId) {
          throw new Error("message_id is required");
        }
        const destinationFolder = input["destination_folder"];
        if (typeof destinationFolder !== "string" || !destinationFolder) {
          throw new Error("destination_folder is required");
        }
        return outlookMove(config, messageId, destinationFolder);
      },
    },
    {
      name: "outlook_unread",
      description:
        "Get unread emails only. Returns subject, sender, date, and preview.",
      inputSchema: {
        type: "object",
        properties: {
          limit: {
            type: "integer",
            description:
              "Number of unread emails to return (1-50, default 10).",
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
        return outlookUnread(config, limit);
      },
    },
  ];
}
