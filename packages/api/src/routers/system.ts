import { count, desc, eq, gte, ilike, max, sql } from "drizzle-orm";
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

// ── Shared types ──────────────────────────────────────────────────────────

export interface ConfigSourceEntry {
  key: string;
  source: "env" | "file" | "default";
  envVar?: string;
}

// ── Helpers ───────────────────────────────────────────────────────────────

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

  /**
   * Config sources: proxy GET /config/sources from the Rust daemon.
   * Returns which env var, TOML file, or default resolved each config key.
   * Falls back to an empty array if the daemon endpoint is not yet available.
   */
  configSources: protectedProcedure.query(async () => {
    try {
      const sources = await fleetFetch<ConfigSourceEntry[]>(
        "daemon",
        "/config/sources",
      );
      return Array.isArray(sources) ? sources : ([] as ConfigSourceEntry[]);
    } catch {
      // Daemon endpoint not yet implemented — return empty so UI degrades gracefully
      return [] as ConfigSourceEntry[];
    }
  }),

  /**
   * Channel status: call channels-svc /channels for live connection status
   * and enrich each channel with lastMessageAt from the messages table.
   * Falls back to the static registry on error.
   */
  channelStatus: protectedProcedure.query(async () => {
    interface ChannelStatusEntry {
      name: string;
      connected: boolean;
      error?: string;
      identity?: { username?: string; displayName?: string };
    }

    // Fetch live channel status from channels-svc
    let liveChannels: ChannelStatusEntry[] = [];
    try {
      liveChannels = await fleetFetch<ChannelStatusEntry[]>(
        "channels-svc",
        "/channels",
      );
    } catch {
      // channels-svc unreachable — fall back to static registry
      liveChannels = KNOWN_CHANNELS.map((c) => ({
        name: c.name,
        connected: false,
        error: "channels-svc unreachable",
      }));
    }

    // Fetch last message timestamps per channel from DB
    const lastMessageRows = await db
      .select({
        channel: messages.channel,
        lastAt: max(messages.createdAt),
      })
      .from(messages)
      .groupBy(messages.channel);

    const lastMessageByChannel = new Map<string, string | null>();
    for (const row of lastMessageRows) {
      lastMessageByChannel.set(
        row.channel,
        row.lastAt ? row.lastAt.toISOString() : null,
      );
    }

    return liveChannels.map((ch) => ({
      name: ch.name,
      connected: ch.connected,
      error: ch.error ?? null,
      identity: ch.identity ?? null,
      lastMessageAt:
        lastMessageByChannel.get(ch.name.toLowerCase()) ??
        lastMessageByChannel.get(ch.name) ??
        null,
    }));
  }),

  /**
   * Test channel: send a test message via channels-svc /send.
   */
  testChannel: protectedProcedure
    .input(
      z.object({
        channel: z.string().min(1),
        target: z.string().min(1),
      }),
    )
    .mutation(async ({ input }) => {
      const start = Date.now();
      try {
        await fleetFetch("channels-svc", "/send", {
          method: "POST",
          body: JSON.stringify({
            channel: input.channel,
            target: input.target,
            message: `[Nova] Connection test at ${new Date().toISOString()}`,
          }),
        });
        return {
          valid: true,
          error: null,
          latencyMs: Date.now() - start,
        };
      } catch (err) {
        return {
          valid: false,
          error: err instanceof Error ? err.message : "Unknown error",
          latencyMs: Date.now() - start,
        };
      }
    }),

  /**
   * Test integration: validate an external service API key by making a
   * lightweight server-side request. Keys are never sent to the browser.
   */
  testIntegration: protectedProcedure
    .input(
      z.object({
        service: z.enum([
          "anthropic",
          "openai",
          "elevenlabs",
          "github",
          "sentry",
          "posthog",
        ]),
      }),
    )
    .mutation(async ({ input }) => {
      const start = Date.now();

      const INTEGRATIONS: Record<
        string,
        { url: string; envVar: string; authHeader?: (key: string) => string }
      > = {
        anthropic: {
          url: "https://api.anthropic.com/v1/models",
          envVar: "ANTHROPIC_API_KEY",
          authHeader: (key) => `x-api-key ${key}`,
        },
        openai: {
          url: "https://api.openai.com/v1/models",
          envVar: "OPENAI_API_KEY",
          authHeader: (key) => `Bearer ${key}`,
        },
        elevenlabs: {
          url: "https://api.elevenlabs.io/v1/voices",
          envVar: "ELEVENLABS_API_KEY",
          authHeader: (key) => `xi-api-key ${key}`,
        },
        github: {
          url: "https://api.github.com/user",
          envVar: "GITHUB_TOKEN",
          authHeader: (key) => `Bearer ${key}`,
        },
        sentry: {
          url: "https://sentry.io/api/0/",
          envVar: "SENTRY_AUTH_TOKEN",
          authHeader: (key) => `Bearer ${key}`,
        },
        posthog: {
          url: "https://app.posthog.com/api/feature_flags/?token=",
          envVar: "POSTHOG_API_KEY",
        },
      };

      const config = INTEGRATIONS[input.service];
      if (!config) {
        return {
          valid: false,
          error: `Unknown service: ${input.service}`,
          latencyMs: 0,
        };
      }

      const apiKey = process.env[config.envVar];
      if (!apiKey) {
        return {
          valid: false,
          error: `${config.envVar} is not set`,
          latencyMs: 0,
        };
      }

      try {
        const headers: Record<string, string> = {
          Accept: "application/json",
        };

        if (config.authHeader) {
          // Split "scheme key" into Authorization header
          const [scheme, ...rest] = config.authHeader(apiKey).split(" ");
          if (scheme && rest.length > 0) {
            // Some APIs use custom header names (e.g., xi-api-key)
            if (
              scheme === "x-api-key" ||
              scheme === "xi-api-key"
            ) {
              headers[scheme] = rest.join(" ");
            } else {
              headers["Authorization"] = `${scheme} ${rest.join(" ")}`;
            }
          }
        }

        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 8_000);

        let url = config.url;
        if (input.service === "posthog") {
          url = `${config.url}${apiKey}`;
        }

        const response = await fetch(url, {
          headers,
          signal: controller.signal,
        });
        clearTimeout(timeoutId);

        if (response.ok || response.status === 200) {
          return {
            valid: true,
            error: null,
            latencyMs: Date.now() - start,
          };
        }

        return {
          valid: false,
          error: `HTTP ${response.status}`,
          latencyMs: Date.now() - start,
        };
      } catch (err) {
        return {
          valid: false,
          error: err instanceof Error ? err.message : "Network error",
          latencyMs: Date.now() - start,
        };
      }
    }),

  /**
   * Memory summary: returns topic count, topic names, last write timestamp,
   * and total content size in bytes.
   */
  memorySummary: protectedProcedure.query(async () => {
    const [countRow, topicsRow, lastWriteRow, sizeRow] = await Promise.all([
      db
        .select({ count: sql<number>`count(*)::int` })
        .from(memory)
        .then((rows) => rows[0]),
      db.select({ topic: memory.topic }).from(memory),
      db
        .select({ lastWriteAt: max(memory.updatedAt) })
        .from(memory)
        .then((rows) => rows[0]),
      db
        .select({
          totalSizeBytes: sql<number>`SUM(LENGTH(content))::int`,
        })
        .from(memory)
        .then((rows) => rows[0]),
    ]);

    return {
      count: countRow?.count ?? 0,
      topics: topicsRow.map((r) => r.topic),
      lastWriteAt: lastWriteRow?.lastWriteAt?.toISOString() ?? null,
      totalSizeBytes: sizeRow?.totalSizeBytes ?? 0,
    };
  }),
});
