import { count, desc, eq, gte, ilike, sql } from "drizzle-orm";
import { z } from "zod";

import { db } from "@nova/db";
import {
  contacts,
  diary,
  memory,
  messages,
  obligations,
  sessions,
} from "@nova/db";

import { createTRPCRouter, protectedProcedure } from "../trpc.js";
import { fleetFetch } from "../lib/fleet.js";

// ── Static fleet service registry ────────────────────────────────────

interface FleetServiceEntry {
  name: string;
  url: string;
  port: number;
  tools: string[];
}

const FLEET_SERVICES: FleetServiceEntry[] = [
  { name: "tool-router", url: "http://127.0.0.1:4100", port: 4100, tools: [] },
  {
    name: "memory-svc",
    url: "http://127.0.0.1:4101",
    port: 4101,
    tools: ["read_memory", "write_memory", "search_memory"],
  },
  {
    name: "messages-svc",
    url: "http://127.0.0.1:4102",
    port: 4102,
    tools: ["get_recent_messages", "search_messages"],
  },
  {
    name: "channels-svc",
    url: "http://127.0.0.1:4103",
    port: 4103,
    tools: ["list_channels", "send_to_channel"],
  },
  {
    name: "discord-svc",
    url: "http://127.0.0.1:4104",
    port: 4104,
    tools: [
      "discord_list_guilds",
      "discord_list_channels",
      "discord_read_messages",
    ],
  },
  {
    name: "teams-svc",
    url: "http://127.0.0.1:4105",
    port: 4105,
    tools: [
      "teams_list_chats",
      "teams_read_chat",
      "teams_messages",
      "teams_channels",
      "teams_presence",
      "teams_send",
    ],
  },
  {
    name: "schedule-svc",
    url: "http://127.0.0.1:4106",
    port: 4106,
    tools: [
      "set_reminder",
      "cancel_reminder",
      "list_reminders",
      "add_schedule",
      "modify_schedule",
      "remove_schedule",
      "list_schedules",
      "start_session",
      "stop_session",
    ],
  },
  {
    name: "graph-svc",
    url: "http://127.0.0.1:4107",
    port: 4107,
    tools: [
      "calendar_today",
      "calendar_upcoming",
      "calendar_next",
      "ado_projects",
      "ado_pipelines",
      "ado_builds",
      "outlook_inbox",
      "outlook_read",
      "outlook_search",
      "outlook_folders",
      "outlook_sent",
      "outlook_folder",
    ],
  },
  {
    name: "meta-svc",
    url: "http://127.0.0.1:4108",
    port: 4108,
    tools: ["check_services", "self_assessment_run", "update_soul"],
  },
  {
    name: "azure-svc",
    url: "http://127.0.0.1:4109",
    port: 4109,
    tools: ["azure_cli"],
  },
];

const KNOWN_CHANNELS = [
  { name: "Telegram", status: "configured" as const, direction: "bidirectional" as const },
  { name: "Discord", status: "configured" as const, direction: "bidirectional" as const },
  { name: "Microsoft Teams", status: "configured" as const, direction: "bidirectional" as const },
];

function getObligationSeverity(
  status: string,
): "error" | "warning" | "info" {
  const lower = status.toLowerCase();
  if (lower.includes("failed") || lower.includes("error")) return "error";
  if (lower === "open" || lower.includes("detected")) return "warning";
  return "info";
}

export const systemRouter = createTRPCRouter({
  /**
   * Health check (DB ping).
   */
  health: protectedProcedure.query(async () => {
    try {
      await db.execute(sql`SELECT 1`);
      return {
        daemon: {
          database: { status: "healthy" },
          note: "Fleet service health is monitored by meta-svc on the host. Dashboard queries Postgres directly.",
        },
        latest: null,
        status: "healthy",
        history: [],
      };
    } catch (e) {
      const message = e instanceof Error ? e.message : "Unknown error";
      return {
        daemon: {
          database: { status: "unhealthy", error: message },
        },
        latest: null,
        status: "critical",
        history: [],
      };
    }
  }),

  /**
   * Latency monitoring (placeholder -- meta-svc handles this on the host).
   */
  latency: protectedProcedure.query(() => {
    return {
      services: {},
      timestamp: new Date().toISOString(),
      note: "Latency monitoring handled by meta-svc on the host. Dashboard queries Postgres directly.",
    };
  }),

  /**
   * Stats: entity counts across tables.
   */
  stats: protectedProcedure.query(async () => {
    const [msgCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(messages);
    const [oblCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(obligations);
    const [contactCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(contacts);
    const [memCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(memory);
    const [diaryCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(diary);

    return {
      tool_usage: {
        total_invocations: 0,
        invocations_today: 0,
        per_tool: [] as { tool: string; count: number }[],
      },
      counts: {
        messages: msgCount?.count ?? 0,
        obligations: oblCount?.count ?? 0,
        contacts: contactCount?.count ?? 0,
        memory: memCount?.count ?? 0,
        diary: diaryCount?.count ?? 0,
      },
    };
  }),

  /**
   * Fleet service registry (static, no HTTP calls).
   */
  fleetStatus: protectedProcedure.query(() => {
    const services = FLEET_SERVICES.map((svc) => ({
      ...svc,
      status: "unknown" as const,
      latency_ms: null as number | null,
    }));

    return {
      fleet: {
        status: "unknown",
        services,
        healthy_count: 0,
        total_count: services.length,
      },
      channels: KNOWN_CHANNELS,
    };
  }),

  /**
   * Activity feed: merged timeline from messages, obligations, diary, sessions (last 24h).
   */
  activityFeed: protectedProcedure.query(async () => {
    const twentyFourHoursAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);

    const [messageRows, obligationRows, diaryRows, sessionRows] =
      await Promise.all([
        db
          .select()
          .from(messages)
          .where(gte(messages.createdAt, twentyFourHoursAgo))
          .orderBy(desc(messages.createdAt))
          .limit(50),
        db
          .select()
          .from(obligations)
          .where(gte(obligations.createdAt, twentyFourHoursAgo))
          .orderBy(desc(obligations.createdAt))
          .limit(50),
        db
          .select()
          .from(diary)
          .where(gte(diary.createdAt, twentyFourHoursAgo))
          .orderBy(desc(diary.createdAt))
          .limit(50),
        db
          .select()
          .from(sessions)
          .where(gte(sessions.startedAt, twentyFourHoursAgo))
          .orderBy(desc(sessions.startedAt))
          .limit(50),
      ]);

    const events: {
      id: string;
      type: string;
      timestamp: string;
      icon_hint: string;
      summary: string;
      severity: "error" | "warning" | "info";
    }[] = [];

    for (const row of messageRows) {
      const direction = row.sender === "nova" ? "outbound" : "inbound";
      const preview =
        row.content.length > 80
          ? `${row.content.slice(0, 80)}...`
          : row.content;
      events.push({
        id: `msg-${row.id}`,
        type: "message",
        timestamp: row.createdAt.toISOString(),
        icon_hint: "MessageSquare",
        summary: `${direction === "inbound" ? "In" : "Out"} [${row.channel}] ${row.sender ?? "unknown"}: ${preview}`,
        severity: "info",
      });
    }

    for (const row of obligationRows) {
      events.push({
        id: `obl-${row.id}`,
        type: "obligation",
        timestamp: row.createdAt.toISOString(),
        icon_hint: "CheckSquare",
        summary: `${row.detectedAction} — ${row.status}`,
        severity: getObligationSeverity(row.status),
      });
    }

    for (const row of diaryRows) {
      events.push({
        id: `diary-${row.id}`,
        type: "diary",
        timestamp: row.createdAt.toISOString(),
        icon_hint: "BookOpen",
        summary: `${row.slug} [${row.channel}]`,
        severity: "info",
      });
    }

    for (const row of sessionRows) {
      const isCompleted =
        row.status === "completed" || row.status === "stopped";
      const summary = isCompleted
        ? `Session completed: ${row.project} (${row.command})`
        : `Session started: ${row.project} (${row.command})`;
      events.push({
        id: `session-${row.id}`,
        type: "session",
        timestamp: row.startedAt.toISOString(),
        icon_hint: "Activity",
        summary,
        severity: "info",
      });
    }

    events.sort(
      (a, b) =>
        new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
    );

    return { events: events.slice(0, 50) };
  }),

  /**
   * Config: fleet service URLs and project config from env.
   */
  config: protectedProcedure.query(() => {
    return {
      tool_router_url:
        process.env.TOOL_ROUTER_URL ??
        "http://host.docker.internal:4100",
      memory_svc_url:
        process.env.MEMORY_SVC_URL ??
        "http://host.docker.internal:4101",
      messages_svc_url:
        process.env.MESSAGES_SVC_URL ??
        "http://host.docker.internal:4102",
      meta_svc_url:
        process.env.META_SVC_URL ??
        "http://host.docker.internal:4108",
      nv_projects: process.env.NV_PROJECTS ?? "[]",
    };
  }),

  /**
   * Memory: get topic or list of topics.
   */
  memory: protectedProcedure
    .input(z.object({ topic: z.string().optional() }))
    .query(async ({ input }) => {
      if (input.topic) {
        const row = await db
          .select()
          .from(memory)
          .where(eq(memory.topic, input.topic))
          .limit(1);

        if (row.length === 0) {
          return { topic: input.topic, content: "" };
        }

        return { topic: input.topic, content: row[0]!.content };
      }

      const rows = await db.select({ topic: memory.topic }).from(memory);
      return { topics: rows.map((r) => r.topic) };
    }),

  /**
   * Memory: upsert a topic.
   */
  updateMemory: protectedProcedure
    .input(
      z.object({
        topic: z.string().min(1),
        content: z.string(),
      }),
    )
    .mutation(async ({ input }) => {
      await db
        .insert(memory)
        .values({
          topic: input.topic,
          content: input.content,
          updatedAt: new Date(),
        })
        .onConflictDoUpdate({
          target: memory.topic,
          set: {
            content: input.content,
            updatedAt: new Date(),
          },
        });

      return { topic: input.topic, written: input.content.length };
    }),
});
