import {
  and,
  count,
  desc,
  eq,
  ilike,
  inArray,
  like,
  max,
} from "drizzle-orm";
import { z } from "zod";
import { TRPCError } from "@trpc/server";

import { db } from "@nova/db";
import {
  projects,
  obligations,
  sessions,
  memory,
  messages,
  diary,
} from "@nova/db";

import { createTRPCRouter, protectedProcedure } from "../trpc.js";

interface EnvProject {
  code: string;
  path?: string;
}

const DEFAULT_PROJECTS: EnvProject[] = [{ code: "nv", path: "~/dev/nv" }];
const ACTIVE_OBLIGATION_STATUSES = ["open", "in_progress"];

function formatDuration(start: Date, stop: Date): string {
  const ms = stop.getTime() - start.getTime();
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  const hours = Math.floor(mins / 60);
  return `${hours}h ${mins % 60}m`;
}

/** Shallow convert camelCase keys to snake_case, Date -> ISO string. */
function toSnakeCase(obj: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    const snakeKey = key.replace(/[A-Z]/g, (l) => `_${l.toLowerCase()}`);
    result[snakeKey] = value instanceof Date ? value.toISOString() : value;
  }
  return result;
}

export const projectRouter = createTRPCRouter({
  /**
   * List projects enriched with obligation/session counts.
   * Seeds from NV_PROJECTS env var if the table is empty.
   */
  list: protectedProcedure
    .input(
      z.object({
        category: z.string().optional(),
      }),
    )
    .query(async ({ input }) => {
      let projectRows = await db.select().from(projects);

      // Seed from NV_PROJECTS if empty
      if (projectRows.length === 0) {
        const envProjects = process.env.NV_PROJECTS;
        let seedList: EnvProject[];

        if (envProjects) {
          try {
            seedList = JSON.parse(envProjects) as EnvProject[];
          } catch {
            seedList = DEFAULT_PROJECTS;
          }
        } else {
          seedList = DEFAULT_PROJECTS;
        }

        for (const p of seedList) {
          await db
            .insert(projects)
            .values({
              code: p.code,
              name: p.code,
              category: "work",
              status: "active",
              path: p.path ?? null,
            })
            .onConflictDoNothing();
        }

        projectRows = await db.select().from(projects);
      }

      if (input.category) {
        projectRows = projectRows.filter((p) => p.category === input.category);
      }

      if (projectRows.length === 0) {
        return { projects: [] };
      }

      const codes = projectRows.map((p) => p.code);

      const obligationTotals = await db
        .select({
          projectCode: obligations.projectCode,
          total: count(obligations.id),
        })
        .from(obligations)
        .where(inArray(obligations.projectCode, codes))
        .groupBy(obligations.projectCode);

      const obligationActive = await db
        .select({
          projectCode: obligations.projectCode,
          activeTotal: count(obligations.id),
        })
        .from(obligations)
        .where(
          and(
            inArray(obligations.projectCode, codes),
            inArray(obligations.status, ACTIVE_OBLIGATION_STATUSES),
          ),
        )
        .groupBy(obligations.projectCode);

      const sessionStats = await db
        .select({
          project: sessions.project,
          sessionCount: count(sessions.id),
          lastStarted: max(sessions.startedAt),
        })
        .from(sessions)
        .where(inArray(sessions.project, codes))
        .groupBy(sessions.project);

      const totalByCode = new Map<string, number>();
      for (const row of obligationTotals) {
        if (row.projectCode) totalByCode.set(row.projectCode, row.total);
      }

      const activeByCode = new Map<string, number>();
      for (const row of obligationActive) {
        if (row.projectCode)
          activeByCode.set(row.projectCode, row.activeTotal);
      }

      const sessionByCode = new Map<
        string,
        { sessionCount: number; lastStarted: Date | null }
      >();
      for (const row of sessionStats) {
        sessionByCode.set(row.project, {
          sessionCount: row.sessionCount,
          lastStarted: row.lastStarted ?? null,
        });
      }

      const enriched = projectRows.map((p) => {
        const sessionInfo = sessionByCode.get(p.code);
        const lastActivity = sessionInfo?.lastStarted
          ? sessionInfo.lastStarted.toISOString()
          : null;

        return {
          id: p.id,
          code: p.code,
          name: p.name,
          category: p.category,
          status: p.status,
          description: p.description,
          content: p.content,
          path: p.path,
          obligation_count: totalByCode.get(p.code) ?? 0,
          active_obligation_count: activeByCode.get(p.code) ?? 0,
          session_count: sessionInfo?.sessionCount ?? 0,
          last_activity: lastActivity,
          created_at: p.createdAt.toISOString(),
          updated_at: p.updatedAt.toISOString(),
        };
      });

      return { projects: enriched };
    }),

  /**
   * Get a project by its code.
   */
  getByCode: protectedProcedure
    .input(z.object({ code: z.string().min(1) }))
    .query(async ({ input }) => {
      const [project] = await db
        .select()
        .from(projects)
        .where(eq(projects.code, input.code))
        .limit(1);

      if (!project) {
        throw new TRPCError({
          code: "NOT_FOUND",
          message: `Project with code '${input.code}' not found`,
        });
      }

      return {
        id: project.id,
        code: project.code,
        name: project.name,
        category: project.category,
        status: project.status,
        description: project.description,
        content: project.content,
        path: project.path,
        created_at: project.createdAt.toISOString(),
        updated_at: project.updatedAt.toISOString(),
      };
    }),

  /**
   * Create a new project.
   */
  create: protectedProcedure
    .input(
      z.object({
        code: z.string().min(1),
        name: z.string().min(1),
        category: z.string().optional(),
        status: z.string().optional(),
        description: z.string().optional(),
        content: z.string().optional(),
        path: z.string().optional(),
      }),
    )
    .mutation(async ({ input }) => {
      // Check for duplicate code
      const existing = await db
        .select({ id: projects.id })
        .from(projects)
        .where(eq(projects.code, input.code))
        .limit(1);

      if (existing.length > 0) {
        throw new TRPCError({
          code: "CONFLICT",
          message: `Project with code '${input.code}' already exists`,
        });
      }

      const [created] = await db
        .insert(projects)
        .values({
          code: input.code,
          name: input.name,
          category: input.category ?? "work",
          status: input.status ?? "active",
          path: input.path ?? null,
        })
        .returning();

      if (!created) {
        throw new TRPCError({
          code: "INTERNAL_SERVER_ERROR",
          message: "Failed to create project",
        });
      }

      return {
        id: created.id,
        code: created.code,
        name: created.name,
        category: created.category,
        status: created.status,
        description: created.description,
        content: created.content,
        path: created.path,
        obligation_count: 0,
        active_obligation_count: 0,
        session_count: 0,
        last_activity: null,
        created_at: created.createdAt.toISOString(),
        updated_at: created.updatedAt.toISOString(),
      };
    }),

  /**
   * Extract and assemble knowledge documents for all projects.
   */
  extract: protectedProcedure.mutation(async () => {
    const allProjects = await db.select().from(projects);

    if (allProjects.length === 0) {
      return { projects_updated: 0, sources_scanned: [] as string[] };
    }

    const sourcesSet = new Set<string>();
    let updatedCount = 0;

    for (const project of allProjects) {
      const code = project.code;
      const codePattern = `%${code}%`;

      const [messageStats, sessionStats, obligationStats, memoryTopics, diaryStats, topSenders] =
        await Promise.all([
          db
            .select({
              messageCount: count(messages.id),
              lastMention: max(messages.createdAt),
            })
            .from(messages)
            .where(ilike(messages.content, codePattern))
            .then((rows) => rows[0] ?? { messageCount: 0, lastMention: null }),
          db
            .select({
              sessionCount: count(sessions.id),
              lastStarted: max(sessions.startedAt),
            })
            .from(sessions)
            .where(eq(sessions.project, code))
            .then((rows) => rows[0] ?? { sessionCount: 0, lastStarted: null }),
          db
            .select({
              status: obligations.status,
              total: count(obligations.id),
            })
            .from(obligations)
            .where(eq(obligations.projectCode, code))
            .groupBy(obligations.status),
          db
            .select({ topic: memory.topic, content: memory.content })
            .from(memory)
            .where(like(memory.topic, "projects-%")),
          db
            .select({
              diaryCount: count(diary.id),
              lastEntry: max(diary.createdAt),
            })
            .from(diary)
            .where(ilike(diary.content, codePattern))
            .then((rows) => rows[0] ?? { diaryCount: 0, lastEntry: null }),
          db
            .select({
              sender: messages.sender,
              msgCount: count(messages.id),
            })
            .from(messages)
            .where(ilike(messages.content, codePattern))
            .groupBy(messages.sender)
            .orderBy(desc(count(messages.id)))
            .limit(10),
        ]);

      if (messageStats.messageCount > 0) sourcesSet.add("messages");
      if (sessionStats.sessionCount > 0) sourcesSet.add("sessions");
      if (obligationStats.length > 0) sourcesSet.add("obligations");
      if (diaryStats.diaryCount > 0) sourcesSet.add("diary");

      const relevantMemory = memoryTopics.filter((t) =>
        t.content.toLowerCase().includes(code.toLowerCase()),
      );
      if (relevantMemory.length > 0) sourcesSet.add("memory");

      const oblSummary: Record<string, number> = {};
      for (const row of obligationStats) {
        oblSummary[row.status] = row.total;
      }
      const totalObligations = Object.values(oblSummary).reduce(
        (a, b) => a + b,
        0,
      );

      const sendersList = topSenders
        .filter((s) => s.sender)
        .map((s) => `- ${s.sender} (${s.msgCount} messages)`)
        .join("\n");

      const memoryExcerpt = relevantMemory
        .slice(0, 3)
        .map((t) => `### ${t.topic}\n${t.content.slice(0, 300)}`)
        .join("\n\n");

      const contentParts: string[] = [
        `# Project: ${project.name} (${code})`,
        "",
        "## Activity Summary",
        "",
        `- **Messages mentioning project:** ${messageStats.messageCount}${messageStats.lastMention ? ` (last: ${new Date(messageStats.lastMention).toISOString().split("T")[0]})` : ""}`,
        `- **Sessions:** ${sessionStats.sessionCount}${sessionStats.lastStarted ? ` (last: ${new Date(sessionStats.lastStarted).toISOString().split("T")[0]})` : ""}`,
        `- **Obligations:** ${totalObligations} total${oblSummary["open"] ? `, ${oblSummary["open"]} open` : ""}${oblSummary["in_progress"] ? `, ${oblSummary["in_progress"]} in progress` : ""}${oblSummary["done"] ? `, ${oblSummary["done"]} done` : ""}`,
        `- **Diary entries:** ${diaryStats.diaryCount}${diaryStats.lastEntry ? ` (last: ${new Date(diaryStats.lastEntry).toISOString().split("T")[0]})` : ""}`,
      ];

      if (sendersList) {
        contentParts.push("", "## Top Contributors", "", sendersList);
      }
      if (memoryExcerpt) {
        contentParts.push("", "## Memory Context", "", memoryExcerpt);
      }

      const content = contentParts.join("\n");

      const descParts: string[] = [];
      if (sessionStats.sessionCount > 0) {
        descParts.push(`${sessionStats.sessionCount} sessions`);
      }
      if (totalObligations > 0) {
        descParts.push(`${totalObligations} obligations`);
      }
      if (messageStats.messageCount > 0) {
        descParts.push(`${messageStats.messageCount} messages`);
      }
      const description =
        descParts.length > 0 ? descParts.join(", ") : null;

      await db
        .update(projects)
        .set({ content, description, updatedAt: new Date() })
        .where(eq(projects.id, project.id));

      updatedCount++;
    }

    return {
      projects_updated: updatedCount,
      sources_scanned: Array.from(sourcesSet),
    };
  }),

  /**
   * Get related entities for a project (obligations, sessions, memory, messages).
   */
  getRelated: protectedProcedure
    .input(z.object({ code: z.string().min(1) }))
    .query(async ({ input }) => {
      const code = input.code;

      const obligationRows = await db
        .select()
        .from(obligations)
        .where(eq(obligations.projectCode, code))
        .orderBy(desc(obligations.updatedAt));

      const mappedObligations = obligationRows.map((row) => ({
        ...toSnakeCase(row as unknown as Record<string, unknown>),
        notes: [],
        attempt_count: 0,
      }));

      const obligationSummary = {
        total: obligationRows.length,
        open: obligationRows.filter((o) => o.status === "open").length,
        in_progress: obligationRows.filter((o) => o.status === "in_progress")
          .length,
        done: obligationRows.filter((o) => o.status === "done").length,
      };

      const sessionRows = await db
        .select()
        .from(sessions)
        .where(eq(sessions.project, code))
        .orderBy(desc(sessions.startedAt));

      const sessionCount = sessionRows.length;

      const mappedSessions = sessionRows.map((row) => ({
        id: row.id,
        project: row.project,
        status: row.status,
        agent_name: row.command,
        started_at: row.startedAt?.toISOString(),
        duration_display: row.stoppedAt
          ? formatDuration(row.startedAt, row.stoppedAt)
          : "running",
        branch: undefined,
        spec: undefined,
        progress: undefined,
      }));

      const allProjectTopics = await db
        .select({ topic: memory.topic, content: memory.content })
        .from(memory)
        .where(like(memory.topic, "projects-%"));

      const matchingTopics = allProjectTopics
        .filter((t) => t.content.toLowerCase().includes(code.toLowerCase()))
        .map((t) => ({
          topic: t.topic,
          preview: t.content.slice(0, 500),
        }));

      const recentMessageRows = await db
        .select()
        .from(messages)
        .where(ilike(messages.content, `%${code}%`))
        .orderBy(desc(messages.createdAt))
        .limit(20);

      const mappedRecentMessages = recentMessageRows.map((row, idx) => ({
        id: idx,
        timestamp: row.createdAt.toISOString(),
        direction: row.sender === "nova" ? "outbound" : "inbound",
        channel: row.channel ?? "unknown",
        sender: row.sender ?? "unknown",
        content: row.content,
        response_time_ms: null,
        tokens_in: null,
        tokens_out: null,
      }));

      return {
        project: { code, path: "" },
        obligations: mappedObligations,
        obligation_summary: obligationSummary,
        sessions: mappedSessions,
        session_count: sessionCount,
        memory_topics: matchingTopics,
        recent_messages: mappedRecentMessages,
      };
    }),
});
