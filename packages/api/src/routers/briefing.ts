import { desc } from "drizzle-orm";
import { z } from "zod";
import { TRPCError } from "@trpc/server";

import { db } from "@nova/db";
import { briefings } from "@nova/db";

import { createTRPCRouter, protectedProcedure } from "../trpc.js";

export interface BriefingAction {
  label: string;
  action: string;
  priority?: string;
}

function mapBriefingRow(row: typeof briefings.$inferSelect) {
  return {
    id: row.id,
    generated_at: row.generatedAt.toISOString(),
    content: row.content,
    suggested_actions: (row.suggestedActions as BriefingAction[]) ?? [],
    sources_status: (row.sourcesStatus as Record<string, string>) ?? {},
  };
}

export const briefingRouter = createTRPCRouter({
  /**
   * Get the latest briefing.
   */
  latest: protectedProcedure.query(async () => {
    const [latest] = await db
      .select()
      .from(briefings)
      .orderBy(desc(briefings.generatedAt))
      .limit(1);

    if (!latest) {
      return { entry: null };
    }

    return { entry: mapBriefingRow(latest) };
  }),

  /**
   * Get briefing history.
   */
  history: protectedProcedure
    .input(z.object({ limit: z.number().int().min(1).max(100).default(20) }))
    .query(async ({ input }) => {
      const rows = await db
        .select()
        .from(briefings)
        .orderBy(desc(briefings.generatedAt))
        .limit(input.limit);

      return { entries: rows.map(mapBriefingRow) };
    }),

  /**
   * Trigger briefing generation via the daemon.
   */
  generate: protectedProcedure.mutation(async () => {
    const DAEMON_URL =
      process.env["DAEMON_URL"] ?? "http://localhost:7700";

    try {
      const res = await fetch(`${DAEMON_URL}/briefing/generate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
      });

      if (!res.ok) {
        const body = (await res.json().catch(() => ({}))) as {
          error?: string;
        };
        throw new TRPCError({
          code: "INTERNAL_SERVER_ERROR",
          message: body.error ?? `Daemon returned ${res.status}`,
        });
      }

      const data = (await res.json()) as {
        id: string;
        generated_at: string;
      };
      return { success: true, briefing_id: data.id };
    } catch (err) {
      if (err instanceof TRPCError) throw err;
      const message =
        err instanceof Error ? err.message : "Daemon unreachable";
      throw new TRPCError({
        code: "INTERNAL_SERVER_ERROR",
        message,
      });
    }
  }),
});
