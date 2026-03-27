/**
 * Contact resolver.
 *
 * Builds a `Map<"channel:senderId", displayName>` from two sources:
 *  1. Contacts table rows (channelIds jsonb field)
 *  2. Memory people profiles (parsed from the `people` memory topic)
 *
 * Contacts table always takes precedence over memory profiles.
 * Key format: `"${channel}:${senderId}"` e.g. `"telegram:7380462766"`.
 */

import type { PersonProfile } from "./people-parser.js";

/**
 * Drizzle-inferred Contact row shape.
 * channelIds is stored as jsonb and may come back as various object shapes.
 */
export interface ContactRow {
  id: string;
  name: string;
  channelIds: unknown;
}

/**
 * Normalise a raw channelIds jsonb value to a flat Record<channel, senderId>.
 *
 * Handles two observed formats:
 *   { telegram: "7380462766" }
 *   { telegram: { id: "7380462766", name: "Leo" } }
 */
function normaliseChannelIds(raw: unknown): Record<string, string> {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};

  const result: Record<string, string> = {};
  for (const [channel, value] of Object.entries(
    raw as Record<string, unknown>,
  )) {
    if (typeof value === "string" && value.trim()) {
      result[channel] = value.trim();
    } else if (value && typeof value === "object" && !Array.isArray(value)) {
      // { id: "...", name: "..." } shape
      const nested = value as Record<string, unknown>;
      const id = nested["id"] ?? nested["userId"] ?? nested["identifier"];
      if (typeof id === "string" && id.trim()) {
        result[channel] = id.trim();
      }
    }
  }
  return result;
}

/**
 * Build a sender resolution map from contacts rows and parsed people profiles.
 *
 * @param contacts  Rows from the contacts table
 * @param peopleProfiles  Profiles parsed from the `people` memory topic
 * @returns Map of "channel:senderId" -> display name
 */
export function resolveContacts(
  contacts: ContactRow[],
  peopleProfiles: PersonProfile[],
): Map<string, string> {
  const map = new Map<string, string>();

  // Pass 1 — contacts table (highest priority)
  for (const contact of contacts) {
    const channelIds = normaliseChannelIds(contact.channelIds);
    for (const [channel, senderId] of Object.entries(channelIds)) {
      const key = `${channel}:${senderId}`;
      map.set(key, contact.name);
    }
  }

  // Pass 2 — memory people profiles (only fill gaps not already mapped)
  for (const profile of peopleProfiles) {
    for (const [channel, senderId] of Object.entries(profile.channelIds)) {
      const key = `${channel}:${senderId}`;
      if (!map.has(key)) {
        map.set(key, profile.name);
      }
    }
  }

  return map;
}
