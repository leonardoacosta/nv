/**
 * Contact materialization: read the "people" memory topic, parse into
 * PersonProfile[], match each profile to existing contacts by channel ID
 * or name, and upsert.
 *
 * Returns { created, updated, unchanged }.
 */

import { eq } from "drizzle-orm";

import { db } from "@nova/db";
import { contacts, memory } from "@nova/db";
import type { Contact } from "@nova/db";

import { parsePeopleMemory } from "./people-parser.js";
import type { PersonProfile } from "./people-parser.js";

export interface MaterializeResult {
  created: number;
  updated: number;
  unchanged: number;
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

/** Merge two channelId maps — union, profile values win on conflict. */
function mergeChannelIds(
  existing: Record<string, string>,
  incoming: Record<string, string>,
): Record<string, string> {
  return { ...existing, ...incoming };
}

/**
 * Check if two channelId maps have any overlapping channel + ID pairs.
 */
function channelIdsOverlap(
  a: Record<string, string>,
  b: Record<string, string>,
): boolean {
  for (const [channel, id] of Object.entries(b)) {
    if (a[channel] && a[channel] === id) return true;
  }
  return false;
}

export async function materializeContacts(): Promise<MaterializeResult> {
  // 1. Read the "people" memory topic
  const [peopleTopic] = await db
    .select({ content: memory.content })
    .from(memory)
    .where(eq(memory.topic, "people"))
    .limit(1);

  if (!peopleTopic) {
    return { created: 0, updated: 0, unchanged: 0 };
  }

  const profiles = parsePeopleMemory(peopleTopic.content);
  if (profiles.length === 0) {
    return { created: 0, updated: 0, unchanged: 0 };
  }

  // 2. Load all existing contacts in-memory (table is small, <1000 rows)
  const existingContacts = await db.select().from(contacts);

  let created = 0;
  let updated = 0;
  let unchanged = 0;

  for (const profile of profiles) {
    await processProfile(profile, existingContacts, {
      onCreated: () => created++,
      onUpdated: () => updated++,
      onUnchanged: () => unchanged++,
    });
  }

  return { created, updated, unchanged };
}

interface ProfileCallbacks {
  onCreated: () => void;
  onUpdated: () => void;
  onUnchanged: () => void;
}

async function processProfile(
  profile: PersonProfile,
  existingContacts: Contact[],
  callbacks: ProfileCallbacks,
): Promise<void> {
  const rows = existingContacts;

  // Match 1: by channel ID overlap
  let matched: Contact | undefined = rows.find((r) => {
    const existing = normaliseChannelIds(r.channelIds);
    return channelIdsOverlap(existing, profile.channelIds);
  });

  // Match 2: by case-insensitive name (fallback)
  if (!matched) {
    matched = rows.find(
      (r) => r.name.toLowerCase() === profile.name.toLowerCase(),
    );
  }

  if (matched) {
    // Upsert existing contact
    const existingIds = normaliseChannelIds(matched.channelIds);
    const mergedIds = mergeChannelIds(existingIds, profile.channelIds);

    const channelIdsChanged =
      JSON.stringify(existingIds) !== JSON.stringify(mergedIds);
    const notesChanged =
      profile.notes &&
      profile.notes !== matched.notes;
    const relationshipChanged =
      !matched.relationshipType &&
      profile.role;

    if (!channelIdsChanged && !notesChanged && !relationshipChanged) {
      callbacks.onUnchanged();
      return;
    }

    const updates: Record<string, unknown> = {};
    if (channelIdsChanged) updates.channelIds = mergedIds;
    if (notesChanged) updates.notes = profile.notes;
    if (relationshipChanged) updates.relationshipType = profile.role;

    await db
      .update(contacts)
      .set(updates)
      .where(eq(contacts.id, matched.id));

    callbacks.onUpdated();
  } else {
    // Insert new contact
    await db
      .insert(contacts)
      .values({
        name: profile.name,
        channelIds: profile.channelIds as Record<string, string>,
        relationshipType: profile.role,
        notes: profile.notes || null,
      })
      .onConflictDoNothing();

    callbacks.onCreated();
  }
}
