import { asc, count, desc, eq, ne, sql } from "drizzle-orm";
import { z } from "zod";
import { db } from "@nova/db";
import { messages } from "@nova/db";
import { createTRPCRouter, protectedProcedure } from "../trpc.js";
export const messageRouter = createTRPCRouter({
    /**
     * List messages with channel/direction/sort/type/limit/offset filters.
     * Returns { messages, total, limit, offset }.
     */
    list: protectedProcedure
        .input(z.object({
        channel: z.string().optional(),
        direction: z.enum(["inbound", "outbound"]).optional(),
        sort: z.enum(["asc", "desc"]).default("desc"),
        type: z.enum(["conversation", "tool-call", "system"]).optional(),
        limit: z.number().int().min(1).max(500).default(50),
        offset: z.number().int().min(0).default(0),
    }))
        .query(async ({ input }) => {
        const conditions = [];
        if (input.channel) {
            conditions.push(eq(messages.channel, input.channel));
        }
        if (input.direction === "outbound") {
            conditions.push(eq(messages.sender, "nova"));
        }
        else if (input.direction === "inbound") {
            conditions.push(ne(messages.sender, "nova"));
        }
        if (input.type) {
            conditions.push(sql `${messages.metadata}->>'type' = ${input.type}`);
        }
        const where = conditions.length === 0
            ? undefined
            : conditions.length === 1
                ? conditions[0]
                : sql `${conditions.reduce((acc, cond) => sql `${acc} AND ${cond}`)}`;
        const orderBy = input.sort === "asc"
            ? asc(messages.createdAt)
            : desc(messages.createdAt);
        const [rows, countResult] = await Promise.all([
            db
                .select()
                .from(messages)
                .where(where)
                .orderBy(orderBy)
                .limit(input.limit)
                .offset(input.offset),
            db
                .select({ total: count() })
                .from(messages)
                .where(where),
        ]);
        const total = countResult[0]?.total ?? 0;
        const mapped = rows.map((row, idx) => {
            const metadata = row.metadata;
            const messageType = typeof metadata?.type === "string"
                ? metadata.type
                : "conversation";
            return {
                id: idx + input.offset,
                timestamp: row.createdAt.toISOString(),
                direction: row.sender === "nova" ? "outbound" : "inbound",
                channel: row.channel ?? "unknown",
                sender: row.sender ?? "unknown",
                content: row.content,
                response_time_ms: null,
                tokens_in: null,
                tokens_out: null,
                type: messageType,
            };
        });
        return {
            messages: mapped,
            total,
            limit: input.limit,
            offset: input.offset,
        };
    }),
});
//# sourceMappingURL=message.js.map