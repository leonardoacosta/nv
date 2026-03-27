import { type NextRequest, NextResponse } from "next/server";
import { and, count, desc, eq, gte, lte } from "drizzle-orm";
import { db } from "@/lib/db";
import { contacts, messages, memory, obligations } from "@nova/db";
import { parsePeopleMemory } from "@/lib/entity-resolution";
import { toSnakeCase } from "@/lib/case";

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

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;

    // [3.1] Load the contact
    const [contact] = await db
      .select()
      .from(contacts)
      .where(eq(contacts.id, id))
      .limit(1);

    if (!contact) {
      return NextResponse.json({ error: "Contact not found" }, { status: 404 });
    }

    const channelIds = normaliseChannelIds(contact.channelIds);
    const channelPairs = Object.entries(channelIds); // [[channel, senderId], ...]
    const channelsActive = Object.keys(channelIds);

    // [3.2] Query messages across all channel/sender pairs, limit 50 total
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

    // Merge and re-sort by createdAt desc, keep top 50
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

    // Total message count across all channels for this contact
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

    // [3.3] Query obligations that might be linked to this contact.
    // Strategy: obligations on any of the contact's channels where a message
    // from this contact exists within +/- 1 hour of the obligation's createdAt.
    //
    // We fetch candidate obligations by sourceChannel, then filter in app code
    // using the message timestamps we already have.
    const contactChannels = channelsActive;
    let relatedObligations: Record<string, unknown>[] = [];

    if (contactChannels.length > 0 && allMessages.length > 0) {
      // Candidate obligations on the same channels
      const obligationCandidates = await Promise.all(
        contactChannels.map((channel) =>
          db
            .select()
            .from(obligations)
            .where(eq(obligations.sourceChannel, channel)),
        ),
      );

      const flatCandidates = obligationCandidates.flat();

      // For each candidate, check if a message from this contact exists within 1 hour
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

    // [3.4] Query memory profile for this contact
    const [peopleTopic] = await db
      .select({ content: memory.content })
      .from(memory)
      .where(eq(memory.topic, "people"))
      .limit(1);

    let memoryProfile: string | null = null;
    if (peopleTopic) {
      const profiles = parsePeopleMemory(peopleTopic.content);
      const matched = profiles.find(
        (p) => p.name.toLowerCase() === contact.name.toLowerCase(),
      );
      memoryProfile = matched?.notes ?? null;
    }

    // [3.5] Assemble ContactRelatedResponse
    const response = {
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

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
