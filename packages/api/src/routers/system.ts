import { count, desc, eq, gte, gt, ilike, lt, max, sql } from "drizzle-orm";
import { z } from "zod";

import { db } from "@nova/db";
import {
  contacts,
  diary,
  digestSuppression,
  fleetHealthSnapshots,
  memory,
  messages,
  obligations,
  sessionEvents,
  sessions,
} from "@nova/db";

import { createTRPCRouter, protectedProcedure } from "../trpc.js";
import { fleetFetch } from "../lib/fleet.js";

// ── Config sources types (used by configSources procedure) ──────────
export type ConfigSource = "env" | "toml" | "db" | "default";

export interface ConfigKeyInfo {
  source: ConfigSource;
  value: string | number | boolean | null;
  validated: boolean;
}

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
  { name: "Telegram", status: "configured" as const, direction: "bidirectional" as const, messages_24h: null as number | null, messages_per_hour: null as number | null },
  { name: "Discord", status: "configured" as const, direction: "bidirectional" as const, messages_24h: null as number | null, messages_per_hour: null as number | null },
  { name: "Microsoft Teams", status: "configured" as const, direction: "bidirectional" as const, messages_24h: null as number | null, messages_per_hour: null as number | null },
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
   * Fleet service status — calls meta-svc /services for live health data.
   * Falls back to static registry with status "unknown" if meta-svc is unreachable.
   * Also inserts health snapshots for historical uptime tracking.
   */
  fleetStatus: protectedProcedure.query(async () => {
    const checkedAt = new Date();
    let services: {
      name: string;
      url: string;
      port: number;
      status: "healthy" | "unreachable" | "unknown";
      latency_ms: number | null;
      tools: string[];
      last_checked: string | null;
      uptime_secs: number | null;
    }[];

    try {
      // Call meta-svc /services for live health probes
      const metaResponse = await fleetFetch<{
        services: {
          name: string;
          url: string;
          status: "healthy" | "unhealthy" | "unreachable";
          latency_ms: number;
          uptime_secs?: number;
          error?: string;
        }[];
        summary: unknown;
      }>("meta-svc", "/services");

      const lastChecked = checkedAt.toISOString();

      services = metaResponse.services.map((svc) => {
        // Extract port from URL
        let port = 0;
        try {
          port = parseInt(new URL(svc.url).port, 10) || 0;
        } catch {
          // leave port as 0 if URL is unparseable
        }

        // Map "unhealthy" to "unreachable" (dashboard has 3 states)
        const mappedStatus =
          svc.status === "healthy"
            ? "healthy"
            : ("unreachable" as const);

        // Merge tools from static registry
        const staticEntry = FLEET_SERVICES.find((f) => f.name === svc.name);

        return {
          name: svc.name,
          url: svc.url,
          port: staticEntry?.port ?? port,
          status: mappedStatus,
          latency_ms: svc.latency_ms,
          tools: staticEntry?.tools ?? [],
          last_checked: lastChecked,
          uptime_secs: svc.uptime_secs ?? null,
        };
      });

      // Insert health snapshots for historical uptime (fire-and-forget)
      void (async () => {
        try {
          const snapshotValues = services.map((svc) => ({
            serviceName: svc.name,
            status: svc.status,
            latencyMs: svc.latency_ms ?? null,
            checkedAt,
          }));

          if (snapshotValues.length > 0) {
            await db.insert(fleetHealthSnapshots).values(snapshotValues);
          }

          // Delete snapshots older than 7 days
          const sevenDaysAgo = new Date(Date.now() - 7 * 24 * 60 * 60 * 1000);
          await db
            .delete(fleetHealthSnapshots)
            .where(lt(fleetHealthSnapshots.checkedAt, sevenDaysAgo));
        } catch {
          // Snapshot write failure is non-critical
        }
      })();
    } catch {
      // meta-svc unreachable — fall back to static registry
      services = FLEET_SERVICES.map((svc) => ({
        ...svc,
        status: "unknown" as const,
        latency_ms: null,
        last_checked: null,
        uptime_secs: null,
      }));
    }

    // Fetch channel status from daemon /channels/status
    let channels: {
      name: string;
      status: "configured" | "unknown" | "connected" | "disconnected" | "unconfigured";
      direction: "bidirectional" | "inbound" | "outbound";
      messages_24h: number | null;
      messages_per_hour: number | null;
    }[];

    try {
      const daemonChannels = await fleetFetch<
        { name: string; status: string; direction: string }[]
      >("daemon", "/channels/status");

      channels = daemonChannels.map((ch) => ({
        name: ch.name,
        status: (["connected", "configured", "disconnected", "unconfigured"].includes(ch.status)
          ? ch.status
          : "unknown") as "configured" | "unknown" | "connected" | "disconnected" | "unconfigured",
        direction: (["bidirectional", "inbound", "outbound"].includes(ch.direction)
          ? ch.direction
          : "bidirectional") as "bidirectional" | "inbound" | "outbound",
        messages_24h: null,
        messages_per_hour: null,
      }));
    } catch {
      // Daemon unreachable — fall back to static channels
      channels = KNOWN_CHANNELS.map((ch) => ({
        ...ch,
        messages_24h: null,
        messages_per_hour: null,
      }));
    }

    const healthyCount = services.filter((s) => s.status === "healthy").length;

    return {
      fleet: {
        status:
          healthyCount === services.length
            ? ("healthy" as const)
            : healthyCount === 0
              ? ("unknown" as const)
              : ("degraded" as const),
        services,
        healthy_count: healthyCount,
        total_count: services.length,
      },
      channels,
    };
  }),

  /**
   * Channel volume: message counts per channel over the last 24h, bucketed by hour.
   */
  channelVolume: protectedProcedure.query(async () => {
    const twentyFourHoursAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);

    const rows = await db
      .select({
        channel: messages.channel,
        hour: sql<string>`date_trunc('hour', ${messages.createdAt})::text`,
        count: sql<number>`count(*)::int`,
      })
      .from(messages)
      .where(gte(messages.createdAt, twentyFourHoursAgo))
      .groupBy(messages.channel, sql`date_trunc('hour', ${messages.createdAt})`)
      .orderBy(messages.channel, sql`date_trunc('hour', ${messages.createdAt})`);

    // Group by channel
    const channelMap = new Map<
      string,
      { total_24h: number; hourly: { hour: string; count: number }[] }
    >();

    for (const row of rows) {
      if (!channelMap.has(row.channel)) {
        channelMap.set(row.channel, { total_24h: 0, hourly: [] });
      }
      const entry = channelMap.get(row.channel)!;
      entry.total_24h += row.count;
      entry.hourly.push({ hour: row.hour, count: row.count });
    }

    return {
      channels: Array.from(channelMap.entries()).map(([name, data]) => ({
        name,
        ...data,
      })),
    };
  }),

  /**
   * Error rates: session_events with error/tool_error types in last 24h,
   * grouped by hour and event type.
   */
  errorRates: protectedProcedure.query(async () => {
    const twentyFourHoursAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);

    const [hourlyRows, byTypeRows] = await Promise.all([
      db
        .select({
          hour: sql<string>`date_trunc('hour', ${sessionEvents.createdAt})::text`,
          count: sql<number>`count(*)::int`,
        })
        .from(sessionEvents)
        .where(
          sql`${sessionEvents.eventType} IN ('error', 'tool_error') AND ${sessionEvents.createdAt} >= ${twentyFourHoursAgo}`,
        )
        .groupBy(sql`date_trunc('hour', ${sessionEvents.createdAt})`)
        .orderBy(sql`date_trunc('hour', ${sessionEvents.createdAt})`),
      db
        .select({
          event_type: sessionEvents.eventType,
          count: sql<number>`count(*)::int`,
        })
        .from(sessionEvents)
        .where(
          sql`${sessionEvents.eventType} IN ('error', 'tool_error') AND ${sessionEvents.createdAt} >= ${twentyFourHoursAgo}`,
        )
        .groupBy(sessionEvents.eventType)
        .orderBy(desc(sql`count(*)`)),
    ]);

    const total_24h = hourlyRows.reduce((sum, row) => sum + row.count, 0);

    return {
      total_24h,
      hourly: hourlyRows,
      by_type: byTypeRows,
    };
  }),

  /**
   * Fleet history: last 24h of fleet health snapshots, downsampled to 15-min buckets.
   * Returns uptime percentage per service.
   */
  fleetHistory: protectedProcedure.query(async () => {
    const twentyFourHoursAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);

    // Downsample to 15-minute buckets, taking worst status per bucket
    const rows = await db
      .select({
        serviceName: fleetHealthSnapshots.serviceName,
        bucket: sql<string>`date_trunc('minute', ${fleetHealthSnapshots.checkedAt} - (EXTRACT(MINUTE FROM ${fleetHealthSnapshots.checkedAt})::int % 15) * INTERVAL '1 minute')::text`,
        // Worst status: unreachable > unhealthy > healthy
        worstStatus: sql<string>`
          CASE
            WHEN bool_or(${fleetHealthSnapshots.status} = 'unreachable') THEN 'unreachable'
            WHEN bool_or(${fleetHealthSnapshots.status} = 'unhealthy') THEN 'unhealthy'
            ELSE 'healthy'
          END
        `,
        avgLatencyMs: sql<number | null>`avg(${fleetHealthSnapshots.latencyMs})::int`,
      })
      .from(fleetHealthSnapshots)
      .where(gte(fleetHealthSnapshots.checkedAt, twentyFourHoursAgo))
      .groupBy(fleetHealthSnapshots.serviceName, sql`date_trunc('minute', ${fleetHealthSnapshots.checkedAt} - (EXTRACT(MINUTE FROM ${fleetHealthSnapshots.checkedAt})::int % 15) * INTERVAL '1 minute')`)
      .orderBy(fleetHealthSnapshots.serviceName, sql`date_trunc('minute', ${fleetHealthSnapshots.checkedAt} - (EXTRACT(MINUTE FROM ${fleetHealthSnapshots.checkedAt})::int % 15) * INTERVAL '1 minute')`);

    // Group by service name
    const serviceMap = new Map<
      string,
      { snapshots: { time: string; status: string; latency_ms: number | null }[]; healthy_buckets: number; total_buckets: number }
    >();

    for (const row of rows) {
      if (!serviceMap.has(row.serviceName)) {
        serviceMap.set(row.serviceName, { snapshots: [], healthy_buckets: 0, total_buckets: 0 });
      }
      const entry = serviceMap.get(row.serviceName)!;
      entry.snapshots.push({
        time: row.bucket,
        status: row.worstStatus,
        latency_ms: row.avgLatencyMs,
      });
      entry.total_buckets++;
      if (row.worstStatus === "healthy") {
        entry.healthy_buckets++;
      }
    }

    return {
      services: Array.from(serviceMap.entries()).map(([name, data]) => ({
        name,
        snapshots: data.snapshots,
        uptime_pct_24h:
          data.total_buckets > 0
            ? Math.round((data.healthy_buckets / data.total_buckets) * 100)
            : 0,
      })),
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

  /* configSources: moved below — Zod-based implementation replaces Rust daemon proxy */

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

  /**
   * Digest stats: aggregate digest run metrics from the diary table plus
   * a live count of active (non-expired) suppressions.
   */
  digestStats: protectedProcedure.query(async () => {
    const now = new Date();

    // Count active suppressions (not yet expired)
    const [activeCountRow] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(digestSuppression)
      .where(gt(digestSuppression.expiresAt, now));

    const activeCount = activeCountRow?.count ?? 0;

    // Most recent digest_run diary entry
    const [lastRunRow] = await db
      .select({
        createdAt: diary.createdAt,
        content: diary.content,
      })
      .from(diary)
      .where(eq(diary.triggerType, "digest_run"))
      .orderBy(desc(diary.createdAt))
      .limit(1);

    // Suppression counts by priority from active rows
    const byPriorityRows = await db
      .select({
        priority: digestSuppression.priority,
        count: sql<number>`count(*)::int`,
      })
      .from(digestSuppression)
      .where(gt(digestSuppression.expiresAt, now))
      .groupBy(digestSuppression.priority)
      .orderBy(digestSuppression.priority);

    const suppressionByPriority: Record<string, number> = {};
    for (const row of byPriorityRows) {
      const label = row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2";
      suppressionByPriority[label] = row.count;
    }

    return {
      last_run_at: lastRunRow?.createdAt.toISOString() ?? null,
      last_run_summary: lastRunRow?.content ?? null,
      active_suppressions_count: activeCount,
      suppression_by_priority: suppressionByPriority,
    };
  }),

  /**
   * Digest suppressions: active (non-expired) suppression entries ordered
   * by last_sent_at DESC.
   */
  digestSuppressions: protectedProcedure.query(async () => {
    const now = new Date();

    const rows = await db
      .select()
      .from(digestSuppression)
      .where(gt(digestSuppression.expiresAt, now))
      .orderBy(desc(digestSuppression.lastSentAt));

    return rows.map((row) => ({
      hash: row.hash,
      source: row.source,
      priority: row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2",
      last_sent_at: row.lastSentAt.toISOString(),
      expires_at: row.expiresAt.toISOString(),
    }));
  }),

  /**
   * Config source introspection.
   *
   * Returns per-key source (env/toml/db/default) and redacts sensitive values.
   * Sources are determined by probing the environment variables that override config.
   * TOML and default are inferred by absence of env override.
   */
  configSources: protectedProcedure.query(() => {

    const SENSITIVE_KEYS = new Set([
      "database.url",
      "telegram.botToken",
      "discord.botToken",
      "teams.webhookUrl",
    ]);

    function redact(key: string, value: string | undefined): string | null {
      if (value === undefined || value === "") return null;
      if (SENSITIVE_KEYS.has(key)) return "***set***";
      return value;
    }

    function envSource(envVar: string): ConfigSource {
      return process.env[envVar] !== undefined ? "env" : "default";
    }

    const keys: Record<string, ConfigKeyInfo> = {
      "daemon.port": {
        source: envSource("NV_DAEMON_PORT"),
        value: process.env["NV_DAEMON_PORT"] ? parseInt(process.env["NV_DAEMON_PORT"]!, 10) : 7700,
        validated: true,
      },
      "daemon.logLevel": {
        source: envSource("NV_LOG_LEVEL"),
        value: process.env["NV_LOG_LEVEL"] ?? "info",
        validated: true,
      },
      "daemon.toolRouterUrl": {
        source: envSource("TOOL_ROUTER_URL"),
        value: process.env["TOOL_ROUTER_URL"] ?? "http://localhost:4100",
        validated: true,
      },
      "database.url": {
        source: process.env["DATABASE_URL"] ? "env" : "default",
        value: redact("database.url", process.env["DATABASE_URL"]),
        validated: true,
      },
      "telegram.botToken": {
        source: process.env["TELEGRAM_BOT_TOKEN"] ? "env" : "default",
        value: redact("telegram.botToken", process.env["TELEGRAM_BOT_TOKEN"]),
        validated: false,
      },
      "telegram.chatId": {
        source: envSource("TELEGRAM_CHAT_ID"),
        value: process.env["TELEGRAM_CHAT_ID"] ?? null,
        validated: false,
      },
      "discord.botToken": {
        source: process.env["DISCORD_BOT_TOKEN"] ? "env" : "default",
        value: redact("discord.botToken", process.env["DISCORD_BOT_TOKEN"]),
        validated: false,
      },
      "teams.webhookUrl": {
        source: process.env["TEAMS_WEBHOOK_URL"] ? "env" : "default",
        value: redact("teams.webhookUrl", process.env["TEAMS_WEBHOOK_URL"]),
        validated: false,
      },
      "queue.concurrency": {
        source: envSource("NV_QUEUE_CONCURRENCY"),
        value: process.env["NV_QUEUE_CONCURRENCY"]
          ? parseInt(process.env["NV_QUEUE_CONCURRENCY"]!, 10)
          : 2,
        validated: true,
      },
      "queue.maxQueueSize": {
        source: envSource("NV_QUEUE_MAX_SIZE"),
        value: process.env["NV_QUEUE_MAX_SIZE"]
          ? parseInt(process.env["NV_QUEUE_MAX_SIZE"]!, 10)
          : 20,
        validated: true,
      },
      "agent.systemPromptPath": {
        source: envSource("NV_SYSTEM_PROMPT_PATH"),
        value: process.env["NV_SYSTEM_PROMPT_PATH"] ?? "config/system-prompt.md",
        validated: true,
      },
    };

    return {
      keys,
      note: "TOML-sourced and default values reflect compiled-in defaults. Set env vars to override.",
    };
  }),
});
