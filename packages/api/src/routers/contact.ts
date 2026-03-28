import {
  and,
  count,
  desc,
  eq,
  gte,
  like,
  lte,
  ne,
  sql,
} from "drizzle-orm";
import { z } from "zod";
import { TRPCError } from "@trpc/server";

import { db } from "@nova/db";
import { contacts, messages, memory, obligations } from "@nova/db";

import { createTRPCRouter, protectedProcedure } from "../trpc.js";
import { materializeContacts } from "../lib/materialize-contacts.js";

/** Shallow convert camelCase keys to snake_case, Date -> ISO string. */
function toSnakeCase(obj: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    const snakeKey = key.replace(/[A-Z]/g, (l) => `_${l.toLowerCase()}`);
    result[snakeKey] = value instanceof Date ? value.toISOString() : value;
  }
  return result;
}

/** Normalise a channelIds jsonb blob to a flat Record<channel, senderId>. */
function normaliseChannelIds(raw: unknown): Record<string, string> {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  const result: Record<string, string> = {};
  for (const [channel, value] of Object.entries(
    raw as Record<string, unknown>,
  )) {
    if (typeof value === "string" && value.trim()) {
      result[channel] = value.trim();
    } else if (value && typeof value === "object" && !Array.isArray(value)) {
      const nested = value as Record<string, unknown>;
      const id = nested["id"] ?? nested["userId"] ?? nested["identifier"];
      if (typeof id === "string" && id.trim()) {
        result[channel] = id.trim();
      }
    }
  }
  return result;
}

export const contactRouter = createTRPCRouter({
  /**
   * List contacts with optional relationship/q filters.
   */
  list: protectedProcedure
    .input(
      z.object({
        relationship: z.string().optional(),
        q: z.string().optional(),
      }),
    )
    .query(async ({ input }) => {
      const conditions = [];
      if (input.relationship) {
        conditions.push(eq(contacts.relationshipType, input.relationship));
      }
      if (input.q) {
        conditions.push(like(contacts.name, `%${input.q}%`));
      }

      const where =
        conditions.length === 0
          ? undefined
          : conditions.length === 1
            ? conditions[0]
            : and(...conditions);

      const rows = await db.select().from(contacts).where(where);

      return rows.map((r) =>
        toSnakeCase(r as unknown as Record<string, unknown>),
      );
    }),

  /**
   * Get a single contact by ID.
   */
  getById: protectedProcedure
    .input(z.object({ id: z.string().uuid() }))
    .query(async ({ input }) => {
      const [contact] = await db
        .select()
        .from(contacts)
        .where(eq(contacts.id, input.id));

      if (!contact) {
        throw new TRPCError({
          code: "NOT_FOUND",
          message: "Contact not found",
        });
      }

      return toSnakeCase(contact as unknown as Record<string, unknown>);
    }),

  /**
   * Create a new contact.
   */
  create: protectedProcedure
    .input(
      z.object({
        name: z.string().min(1),
        channel_ids: z.record(z.string(), z.string()).default({}),
        relationship_type: z.string().nullable().optional(),
        notes: z.string().nullable().optional(),
      }),
    )
    .mutation(async ({ input }) => {
      const [created] = await db
        .insert(contacts)
        .values({
          name: input.name,
          channelIds: input.channel_ids,
          relationshipType: input.relationship_type ?? null,
          notes: input.notes ?? null,
        })
        .returning();

      if (!created) {
        throw new TRPCError({
          code: "INTERNAL_SERVER_ERROR",
          message: "Failed to create contact",
        });
      }

      return toSnakeCase(created as unknown as Record<string, unknown>);
    }),

  /**
   * Update a contact by ID.
   */
  update: protectedProcedure
    .input(
      z.object({
        id: z.string().uuid(),
        name: z.string().min(1).optional(),
        channel_ids: z.record(z.string(), z.string()).optional(),
        relationship_type: z.string().nullable().optional(),
        notes: z.string().nullable().optional(),
      }),
    )
    .mutation(async ({ input }) => {
      const updates: Record<string, unknown> = {};
      if (input.name !== undefined) updates.name = input.name;
      if (input.channel_ids !== undefined) updates.channelIds = input.channel_ids;
      if (input.relationship_type !== undefined) updates.relationshipType = input.relationship_type;
      if (input.notes !== undefined) updates.notes = input.notes;

      const [updated] = await db
        .update(contacts)
        .set(updates)
        .where(eq(contacts.id, input.id))
        .returning();

      if (!updated) {
        throw new TRPCError({
          code: "NOT_FOUND",
          message: "Contact not found",
        });
      }

      return toSnakeCase(updated as unknown as Record<string, unknown>);
    }),

  /**
   * Delete a contact by ID.
   */
  delete: protectedProcedure
    .input(z.object({ id: z.string().uuid() }))
    .mutation(async ({ input }) => {
      const [deleted] = await db
        .delete(contacts)
        .where(eq(contacts.id, input.id))
        .returning();

      if (!deleted) {
        throw new TRPCError({
          code: "NOT_FOUND",
          message: "Contact not found",
        });
      }

      return toSnakeCase(deleted as unknown as Record<string, unknown>);
    }),

  /**
   * Get related data for a contact (messages, obligations, memory profile).
   */
  getRelated: protectedProcedure
    .input(z.object({ id: z.string().uuid() }))
    .query(async ({ input }) => {
      const [contact] = await db
        .select()
        .from(contacts)
        .where(eq(contacts.id, input.id))
        .limit(1);

      if (!contact) {
        throw new TRPCError({
          code: "NOT_FOUND",
          message: "Contact not found",
        });
      }

      const channelIds = normaliseChannelIds(contact.channelIds);
      const channelPairs = Object.entries(channelIds);
      const channelsActive = Object.keys(channelIds);

      // Messages across all channel/sender pairs
      const messagesByChannel = await Promise.all(
        channelPairs.map(([channel, senderId]) =>
          db
            .select()
            .from(messages)
            .where(
              and(eq(messages.channel, channel), eq(messages.sender, senderId)),
            )
            .orderBy(desc(messages.createdAt))
            .limit(50),
        ),
      );

      const allMessages = messagesByChannel
        .flat()
        .sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime())
        .slice(0, 50);

      const mappedMessages = allMessages.map((row, idx) => ({
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

      // Total message count
      const messageCounts = await Promise.all(
        channelPairs.map(([channel, senderId]) =>
          db
            .select({ total: count() })
            .from(messages)
            .where(
              and(eq(messages.channel, channel), eq(messages.sender, senderId)),
            ),
        ),
      );
      const messageCount = messageCounts.reduce(
        (sum, rows) => sum + (rows[0]?.total ?? 0),
        0,
      );

      // Related obligations via channel + time proximity
      let relatedObligations: Record<string, unknown>[] = [];
      if (channelsActive.length > 0 && allMessages.length > 0) {
        const obligationCandidates = await Promise.all(
          channelsActive.map((channel) =>
            db
              .select()
              .from(obligations)
              .where(eq(obligations.sourceChannel, channel)),
          ),
        );

        const flatCandidates = obligationCandidates.flat();
        const messageTimestamps = allMessages.map((m) => m.createdAt.getTime());
        const ONE_HOUR_MS = 3_600_000;

        const filtered = flatCandidates.filter((obl) => {
          const oblTime = obl.createdAt.getTime();
          return messageTimestamps.some(
            (ts) => Math.abs(ts - oblTime) <= ONE_HOUR_MS,
          );
        });

        relatedObligations = filtered.map((row) => ({
          ...toSnakeCase(row as unknown as Record<string, unknown>),
          notes: [],
          attempt_count: 0,
        }));
      }

      // Memory profile
      const [peopleTopic] = await db
        .select({ content: memory.content })
        .from(memory)
        .where(eq(memory.topic, "people"))
        .limit(1);

      let memoryProfile: string | null = null;
      if (peopleTopic) {
        // Simple people memory parsing: find section for this contact
        const sections = peopleTopic.content.split(/\n(?=##?\s)/);
        const matched = sections.find(
          (s) => s.toLowerCase().includes(contact.name.toLowerCase()),
        );
        memoryProfile = matched?.trim() ?? null;
      }

      return {
        contact: {
          id: contact.id,
          name: contact.name,
          channel_ids: channelIds,
          relationship_type: contact.relationshipType ?? null,
          notes: contact.notes ?? null,
          created_at: contact.createdAt.toISOString(),
        },
        messages: mappedMessages,
        message_count: messageCount,
        obligations: relatedObligations,
        memory_profile: memoryProfile,
        channels_active: channelsActive,
      };
    }),

  /**
   * Materialize contacts from the "people" memory topic.
   * Parses PersonProfile records and upserts them into the contacts table.
   */
  materialize: protectedProcedure.mutation(async () => {
    return materializeContacts();
  }),

  /**
   * Get discovered contacts from message data.
   */
  discovered: protectedProcedure.query(async () => {
    const rows = await db.execute<{
      sender: string;
      message_count: number;
      channels: string;
      first_seen: string;
      last_seen: string;
      contact_id: string | null;
      relationship_type: string | null;
      notes: string | null;
      channel_ids: Record<string, string> | null;
    }>(sql`
      SELECT
        m.sender,
        COUNT(*)::int                          AS message_count,
        STRING_AGG(DISTINCT m.channel, ',')    AS channels,
        MIN(m.created_at)::text                AS first_seen,
        MAX(m.created_at)::text                AS last_seen,
        c.id                                   AS contact_id,
        c.relationship_type                    AS relationship_type,
        c.notes                                AS notes,
        c.channel_ids                          AS channel_ids
      FROM ${messages} m
      LEFT JOIN ${contacts} c ON LOWER(c.name) = LOWER(m.sender)
      WHERE m.sender IS NOT NULL
        AND LOWER(m.sender) != 'nova'
      GROUP BY m.sender, c.id, c.relationship_type, c.notes, c.channel_ids
      ORDER BY MAX(m.created_at) DESC
    `);

    const totalMessages = await db
      .execute<{ count: number }>(
        sql`SELECT COUNT(*)::int AS count FROM ${messages}`,
      )
      .then(
        (r) => (r as unknown as { count: number }[])[0]?.count ?? 0,
      );

    const mapped = (
      rows as unknown as {
        sender: string;
        message_count: number;
        channels: string;
        first_seen: string;
        last_seen: string;
        contact_id: string | null;
        relationship_type: string | null;
        notes: string | null;
      }[]
    ).map((r) => ({
      name: r.sender,
      channels: r.channels ? r.channels.split(",") : [],
      message_count: r.message_count,
      first_seen: r.first_seen,
      last_seen: r.last_seen,
      contact_id: r.contact_id,
      relationship_type: r.relationship_type,
      notes: r.notes,
      channel_ids: null,
    }));

    return {
      contacts: mapped,
      total_senders: mapped.length,
      total_messages_scanned: totalMessages,
    };
  }),

  /**
   * Get sender relationship co-occurrences.
   */
  relationships: protectedProcedure
    .input(z.object({ min_count: z.number().int().min(1).default(2) }))
    .query(async ({ input }) => {
      const rows = await db.execute<{
        person_a: string;
        person_b: string;
        shared_channel: string;
        co_occurrence_count: number;
      }>(sql`
        WITH sender_days AS (
          SELECT DISTINCT sender, channel, DATE(created_at) AS day
          FROM ${messages}
          WHERE sender IS NOT NULL
            AND LOWER(sender) != 'nova'
        )
        SELECT
          a.sender  AS person_a,
          b.sender  AS person_b,
          a.channel AS shared_channel,
          COUNT(*)::int AS co_occurrence_count
        FROM sender_days a
        JOIN sender_days b
          ON a.channel = b.channel
          AND a.day = b.day
          AND a.sender < b.sender
        GROUP BY a.sender, b.sender, a.channel
        HAVING COUNT(*) >= ${input.min_count}
        ORDER BY co_occurrence_count DESC
      `);

      return {
        relationships: (
          rows as unknown as {
            person_a: string;
            person_b: string;
            shared_channel: string;
            co_occurrence_count: number;
          }[]
        ).map((r) => ({
          person_a: r.person_a,
          person_b: r.person_b,
          shared_channel: r.shared_channel,
          co_occurrence_count: r.co_occurrence_count,
        })),
      };
    }),

  /**
   * Resolve sender identifiers to contact display names.
   */
  resolve: protectedProcedure
    .input(z.object({ senders: z.array(z.string()) }))
    .mutation(async ({ input }) => {
      if (input.senders.length === 0) {
        return {} as Record<string, string>;
      }

      const rows = await db.select().from(contacts);
      const result: Record<string, string> = {};

      for (const row of rows) {
        const channelIds = row.channelIds as Record<string, string | undefined> | null;
        if (!channelIds) continue;

        for (const [platform, platformId] of Object.entries(channelIds)) {
          if (!platformId) continue;
          const composed = `${platform}:${platformId}`;
          if (input.senders.includes(composed)) {
            result[composed] = row.name;
          }
          if (input.senders.includes(platformId)) {
            result[platformId] = row.name;
          }
        }
      }

      return result;
    }),
});
