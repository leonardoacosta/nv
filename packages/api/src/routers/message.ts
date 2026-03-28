import { and, asc, count, desc, eq, lt, ne, or, sql } from "drizzle-orm";
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
    .input(
      z.object({
        channel: z.string().optional(),
        direction: z.enum(["inbound", "outbound"]).optional(),
        sort: z.enum(["asc", "desc"]).default("desc"),
        type: z.enum(["conversation", "tool-call", "system"]).optional(),
        limit: z.number().int().min(1).max(500).default(50),
        offset: z.number().int().min(0).default(0),
      }),
    )
    .query(async ({ input }) => {
      const conditions = [];

      if (input.channel) {
        conditions.push(eq(messages.channel, input.channel));
      }

      if (input.direction === "outbound") {
        conditions.push(eq(messages.sender, "nova"));
      } else if (input.direction === "inbound") {
        conditions.push(ne(messages.sender, "nova"));
      }

      if (input.type) {
        conditions.push(
          sql`${messages.metadata}->>'type' = ${input.type}`,
        );
      }

      const where =
        conditions.length === 0
          ? undefined
          : conditions.length === 1
            ? conditions[0]
            : sql`${conditions.reduce((acc, cond) => sql`${acc} AND ${cond}`)}`;

      const orderBy =
        input.sort === "asc"
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
        const metadata = row.metadata as Record<string, unknown> | null;
        const messageType =
          typeof metadata?.type === "string"
            ? (metadata.type as "conversation" | "tool-call" | "system")
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

  /**
   * Cursor-based pagination for the chat history page.
   * Returns messages in reverse chronological order (newest first) so the
   * UI can use `flex-col-reverse` and load older pages upward.
   *
   * Filters to conversation-type messages only (excludes tool-call/system).
   * When `cursor` (ISO datetime) is provided, returns messages older than it.
   * Returns `nextCursor` (oldest row's createdAt ISO) when more pages exist.
   */
  chatHistory: protectedProcedure
    .input(
      z.object({
        limit: z.number().int().min(1).max(50).default(25),
        cursor: z.string().datetime().optional(),
      }),
    )
    .query(async ({ input }) => {
      const fetchLimit = input.limit + 1;

      // Filter: conversation messages only (exclude tool-call and system)
      const typeFilter = or(
        sql`${messages.metadata}->>'type' = 'conversation'`,
        sql`${messages.metadata}->>'type' IS NULL`,
      );

      const where = input.cursor
        ? and(typeFilter, lt(messages.createdAt, new Date(input.cursor)))
        : typeFilter;

      const rows = await db
        .select()
        .from(messages)
        .where(where)
        .orderBy(desc(messages.createdAt))
        .limit(fetchLimit);

      let nextCursor: string | null = null;
      if (rows.length > input.limit) {
        const extra = rows.pop();
        if (extra) {
          nextCursor = extra.createdAt.toISOString();
        }
      }

      const mapped = rows.map((row, idx) => {
        const metadata = row.metadata as Record<string, unknown> | null;
        const messageType =
          typeof metadata?.type === "string"
            ? (metadata.type as "conversation" | "tool-call" | "system")
            : "conversation";

        return {
          id: idx,
          timestamp: row.createdAt.toISOString(),
          direction: row.sender === "nova" ? "outbound" : "inbound",
          channel: row.channel ?? "unknown",
          sender: row.sender ?? "unknown",
          content: row.content,
          response_time_ms: null as number | null,
          tokens_in: null as number | null,
          tokens_out: null as number | null,
          type: messageType,
        };
      });

      return {
        messages: mapped,
        nextCursor,
      };
    }),
});
