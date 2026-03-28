/**
 * Batched sender resolution for message.list.
 *
 * Resolution priority (first match wins):
 *   1. Contacts table — channel_ids JSONB match
 *   2. Telegram metadata — extract first_name/last_name/username from metadata JSONB
 *   3. Memory people profiles — parse `people` memory topic and match by channel ID
 *   4. Raw fallback — return the raw sender string unchanged
 */

import { eq } from "drizzle-orm";

import { db } from "@nova/db";
import { contacts, memory } from "@nova/db";

import { extractTelegramName } from "./telegram-metadata.js";
import { parsePeopleMemory } from "./people-parser.js";

export type ResolutionSource = "contact" | "telegram-meta" | "memory" | "raw";

export interface SenderResolution {
  displayName: string;
  avatarInitial: string;
  source: ResolutionSource;
}

export interface SenderInput {
  raw: string;
  channel: string;
  metadata: unknown;
}

/**
 * Resolve a batch of sender+channel pairs to display names.
 *
 * All DB queries are batched — contacts and memory are loaded once and
 * reused across all senders in the request.
 *
 * @param senders - Array of unique sender descriptors from the current page
 * @returns Map keyed by `${channel}:${raw}` → SenderResolution
 */
export async function resolveSenders(
  senders: SenderInput[],
): Promise<Map<string, SenderResolution>> {
  const result = new Map<string, SenderResolution>();

  if (senders.length === 0) return result;

  // Load all contacts once (table is small)
  const allContacts = await db.select().from(contacts);

  // Build a fast lookup: for each contact, map (channel, senderId) → name
  const contactLookup = new Map<string, string>();
  for (const contact of allContacts) {
    const channelIds = normaliseChannelIds(contact.channelIds);
    for (const [channel, senderId] of Object.entries(channelIds)) {
      contactLookup.set(`${channel}:${senderId}`, contact.name);
    }
  }

  // Load people memory topic once and parse
  const [peopleTopic] = await db
    .select({ content: memory.content })
    .from(memory)
    .where(eq(memory.topic, "people"))
    .limit(1);

  const memoryProfiles = peopleTopic
    ? parsePeopleMemory(peopleTopic.content)
    : [];

  // Resolve each unique sender
  for (const sender of senders) {
    const key = `${sender.channel}:${sender.raw}`;

    if (result.has(key)) continue; // already resolved (dedup)

    // 1. Contact lookup
    const contactName = contactLookup.get(key);
    if (contactName) {
      result.set(key, {
        displayName: contactName,
        avatarInitial: contactName[0]?.toUpperCase() ?? "?",
        source: "contact",
      });
      continue;
    }

    // 2. Telegram metadata extraction
    if (sender.channel === "telegram") {
      const metadata =
        sender.metadata &&
        typeof sender.metadata === "object" &&
        !Array.isArray(sender.metadata)
          ? (sender.metadata as Record<string, unknown>)
          : null;

      const telegramName = extractTelegramName(metadata);
      if (telegramName) {
        result.set(key, {
          displayName: telegramName,
          avatarInitial: telegramName[0]?.toUpperCase() ?? "?",
          source: "telegram-meta",
        });
        continue;
      }
    }

    // 3. Memory people profiles — match by channel ID
    const memoryMatch = memoryProfiles.find((profile) => {
      const channelId = profile.channelIds[sender.channel];
      return channelId && channelId === sender.raw;
    });

    if (memoryMatch) {
      result.set(key, {
        displayName: memoryMatch.name,
        avatarInitial: memoryMatch.name[0]?.toUpperCase() ?? "?",
        source: "memory",
      });
      continue;
    }

    // 4. Raw fallback
    result.set(key, {
      displayName: sender.raw,
      avatarInitial: sender.raw[0]?.toUpperCase() ?? "?",
      source: "raw",
    });
  }

  return result;
}

/** Normalise channelIds blob to Record<channel, senderId>. */
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
