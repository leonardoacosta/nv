import { desc, eq } from "drizzle-orm";
import { z } from "zod";
import { TRPCError } from "@trpc/server";
import { db } from "@nova/db";
import { briefings, settings } from "@nova/db";
import { createTRPCRouter, protectedProcedure } from "../trpc.js";
function mapBriefingRow(row) {
    return {
        id: row.id,
        generated_at: row.generatedAt.toISOString(),
        content: row.content,
        suggested_actions: row.suggestedActions ?? [],
        sources_status: row.sourcesStatus ?? {},
        blocks: row.blocks ?? null,
    };
}
/**
 * Reads briefing_hour from the settings table.
 * Defaults to 7 if not set or invalid.
 */
async function getBriefingHour() {
    const [setting] = await db
        .select()
        .from(settings)
        .where(eq(settings.key, "briefing_hour"));
    if (!setting)
        return 7;
    const parsed = parseInt(setting.value, 10);
    return isNaN(parsed) ? 7 : parsed;
}
/**
 * Returns true if:
 * - Current time is past briefingHour + 1 (e.g., 08:00 when briefingHour = 7)
 * - The latest briefing was NOT generated today
 */
function computeMissedToday(latestGeneratedAt, briefingHour) {
    const now = new Date();
    const currentHour = now.getHours();
    if (currentHour < briefingHour + 1) {
        return false;
    }
    if (!latestGeneratedAt) {
        return true;
    }
    const todayStr = now.toISOString().slice(0, 10);
    const latestDateStr = latestGeneratedAt.toISOString().slice(0, 10);
    return latestDateStr !== todayStr;
}
export const briefingRouter = createTRPCRouter({
    /**
     * Get the latest briefing with missedToday flag.
     */
    latest: protectedProcedure.query(async () => {
        const [latest] = await db
            .select()
            .from(briefings)
            .orderBy(desc(briefings.generatedAt))
            .limit(1);
        const briefingHour = await getBriefingHour();
        const latestGeneratedAt = latest?.generatedAt ?? null;
        const missedToday = computeMissedToday(latestGeneratedAt, briefingHour);
        if (!latest) {
            return { entry: null, missedToday };
        }
        return { entry: mapBriefingRow(latest), missedToday };
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
        const DAEMON_URL = process.env["DAEMON_URL"] ?? "http://localhost:7700";
        try {
            const res = await fetch(`${DAEMON_URL}/briefing/generate`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
            });
            if (!res.ok) {
                const body = (await res.json().catch(() => ({})));
                throw new TRPCError({
                    code: "INTERNAL_SERVER_ERROR",
                    message: body.error ?? `Daemon returned ${res.status}`,
                });
            }
            const data = (await res.json());
            return { success: true, briefing_id: data.id };
        }
        catch (err) {
            if (err instanceof TRPCError)
                throw err;
            const message = err instanceof Error ? err.message : "Daemon unreachable";
            throw new TRPCError({
                code: "INTERNAL_SERVER_ERROR",
                message,
            });
        }
    }),
});
//# sourceMappingURL=briefing.js.map