import { and, desc, gte, lt, sql } from "drizzle-orm";
import { z } from "zod";
import { db } from "@nova/db";
import { diary } from "@nova/db";
import { createTRPCRouter, protectedProcedure } from "../trpc.js";
export const diaryRouter = createTRPCRouter({
    /**
     * List diary entries with optional date/limit filters.
     */
    list: protectedProcedure
        .input(z.object({
        date: z.string().optional(),
        limit: z.number().int().min(1).max(500).default(50),
    }))
        .query(async ({ input }) => {
        const conditions = [];
        let dateLabel = new Date().toISOString().slice(0, 10);
        if (input.date) {
            dateLabel = input.date;
            const dayStart = new Date(`${input.date}T00:00:00.000Z`);
            const dayEnd = new Date(`${input.date}T23:59:59.999Z`);
            conditions.push(gte(diary.createdAt, dayStart));
            conditions.push(lt(diary.createdAt, dayEnd));
        }
        const where = conditions.length === 0
            ? undefined
            : conditions.length === 1
                ? conditions[0]
                : and(...conditions);
        const rows = await db
            .select()
            .from(diary)
            .where(where)
            .orderBy(desc(diary.createdAt))
            .limit(input.limit);
        const [totalResult] = await db
            .select({ count: sql `count(*)::int` })
            .from(diary)
            .where(where);
        const [channelResult] = await db
            .select({
            count: sql `count(distinct ${diary.channel})::int`,
        })
            .from(diary)
            .where(where);
        const [lastResult] = await db
            .select({ last: sql `max(${diary.createdAt})` })
            .from(diary)
            .where(where);
        const entries = rows.map((row) => ({
            time: row.createdAt.toISOString(),
            trigger_type: row.triggerType,
            trigger_source: row.triggerSource,
            channel_source: row.channel,
            slug: row.slug,
            tools_called: row.toolsUsed ?? [],
            result_summary: row.content,
            response_latency_ms: row.responseLatencyMs ?? 0,
            tokens_in: row.tokensIn ?? 0,
            tokens_out: row.tokensOut ?? 0,
        }));
        return {
            date: dateLabel,
            entries,
            total: totalResult?.count ?? entries.length,
            distinct_channels: channelResult?.count ?? 0,
            last_interaction_at: lastResult?.last ?? null,
        };
    }),
});
//# sourceMappingURL=diary.js.map