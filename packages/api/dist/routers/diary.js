import { and, desc, gte, lt, sql } from "drizzle-orm";
import { z } from "zod";
import { db } from "@nova/db";
import { diary } from "@nova/db";
import { createTRPCRouter, protectedProcedure } from "../trpc.js";
/**
 * Normalize a raw toolsUsed value from Postgres (jsonb).
 * Handles both legacy string[] and new ToolCallDetail[] formats.
 */
function normalizeToolsUsed(raw) {
    if (!Array.isArray(raw))
        return [];
    return raw.map((item) => {
        if (typeof item === "string") {
            // Legacy format — wrap as ToolCallDetail
            return { name: item, input_summary: "", duration_ms: null };
        }
        if (item && typeof item === "object") {
            const obj = item;
            return {
                name: typeof obj["name"] === "string" ? obj["name"] : "",
                input_summary: typeof obj["input_summary"] === "string" ? obj["input_summary"] : "",
                duration_ms: typeof obj["duration_ms"] === "number" ? obj["duration_ms"] : null,
            };
        }
        return { name: "", input_summary: "", duration_ms: null };
    });
}
export const diaryRouter = createTRPCRouter({
    /**
     * List diary entries with optional date/limit filters.
     * Returns normalized entries with both legacy and new tool formats unified,
     * plus daily aggregate statistics.
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
        const entries = rows.map((row) => {
            const tools_detail = normalizeToolsUsed(row.toolsUsed);
            return {
                time: row.createdAt.toISOString(),
                trigger_type: row.triggerType,
                trigger_source: row.triggerSource,
                channel_source: row.channel,
                slug: row.slug,
                tools_called: tools_detail.map((t) => t.name),
                tools_detail,
                result_summary: row.content,
                response_latency_ms: row.responseLatencyMs ?? 0,
                tokens_in: row.tokensIn ?? 0,
                tokens_out: row.tokensOut ?? 0,
                model: row.model ?? null,
                cost_usd: row.costUsd != null ? Number(row.costUsd) : null,
            };
        });
        // ── Aggregate computation ─────────────────────────────────────────────
        let total_tokens_in = 0;
        let total_tokens_out = 0;
        let total_cost_usd_sum = 0;
        let cost_usd_count = 0;
        let latency_sum = 0;
        let latency_count = 0;
        const toolFreqMap = new Map();
        for (const entry of entries) {
            total_tokens_in += entry.tokens_in;
            total_tokens_out += entry.tokens_out;
            if (entry.cost_usd != null) {
                total_cost_usd_sum += entry.cost_usd;
                cost_usd_count++;
            }
            if (entry.response_latency_ms > 0) {
                latency_sum += entry.response_latency_ms;
                latency_count++;
            }
            for (const tool of entry.tools_detail) {
                if (tool.name) {
                    toolFreqMap.set(tool.name, (toolFreqMap.get(tool.name) ?? 0) + 1);
                }
            }
        }
        const tool_frequency = Array.from(toolFreqMap.entries())
            .map(([name, count]) => ({ name, count }))
            .sort((a, b) => b.count - a.count)
            .slice(0, 10);
        const aggregates = {
            total_tokens_in,
            total_tokens_out,
            total_cost_usd: cost_usd_count > 0 ? total_cost_usd_sum : null,
            avg_latency_ms: latency_count > 0 ? Math.round(latency_sum / latency_count) : 0,
            tool_frequency,
        };
        return {
            date: dateLabel,
            entries,
            total: totalResult?.count ?? entries.length,
            distinct_channels: channelResult?.count ?? 0,
            last_interaction_at: lastResult?.last ?? null,
            aggregates,
        };
    }),
});
//# sourceMappingURL=diary.js.map