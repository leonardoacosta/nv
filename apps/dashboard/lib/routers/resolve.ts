/**
 * Dashboard-local tRPC router for entity resolution.
 *
 * These procedures wrap the existing entity-resolution library which
 * is dashboard-specific. They live in apps/dashboard/ to keep
 * @nova/api free of dashboard-specific logic.
 */

import { eq } from "drizzle-orm";
import { createTRPCRouter, protectedProcedure } from "@nova/api";
import { db, contacts, messages, memory } from "@nova/db";
import {
  parsePeopleMemory,
  resolveContacts,
} from "@/lib/entity-resolution";

export const resolveRouter = createTRPCRouter({
  resolve: createTRPCRouter({
    /**
     * Resolve all senders to display names using contacts + memory.
     * Returns a resolution map, source counts, and unresolved count.
     */
    senders: protectedProcedure.query(async () => {
      // Load all contacts
      const contactRows = await db.select().from(contacts);

      // Load the `people` memory topic
      const [peopleTopic] = await db
        .select({ content: memory.content })
        .from(memory)
        .where(eq(memory.topic, "people"))
        .limit(1);

      const peopleProfiles = peopleTopic
        ? parsePeopleMemory(peopleTopic.content)
        : [];

      // Build resolution map
      const resolutionMap = resolveContacts(contactRows, peopleProfiles);

      // Compute source counts
      let contactsTableCount = 0;
      for (const contact of contactRows) {
        const raw = contact.channelIds;
        if (raw && typeof raw === "object" && !Array.isArray(raw)) {
          contactsTableCount += Object.keys(
            raw as Record<string, unknown>,
          ).length;
        }
      }

      const contactsOnlyMap = resolveContacts(contactRows, []);
      const memoryPeopleCount = resolutionMap.size - contactsOnlyMap.size;

      // Distinct senders
      const distinctSenders = await db
        .selectDistinct({
          channel: messages.channel,
          sender: messages.sender,
        })
        .from(messages);

      let unresolvedCount = 0;
      for (const row of distinctSenders) {
        if (!row.sender) continue;
        const key = `${row.channel}:${row.sender}`;
        if (!resolutionMap.has(key)) {
          unresolvedCount++;
        }
      }

      // Serialize Map to plain object
      const resolutions: Record<string, string> = {};
      for (const [key, name] of resolutionMap.entries()) {
        resolutions[key] = name;
      }

      return {
        resolutions,
        source_counts: {
          contacts_table: contactsTableCount,
          memory_people: memoryPeopleCount > 0 ? memoryPeopleCount : 0,
          unresolved: unresolvedCount,
        },
      };
    }),
  }),
});
