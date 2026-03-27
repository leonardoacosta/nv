import { NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { contacts, memory, messages } from "@nova/db";
import { parsePeopleMemory, resolveContacts } from "@/lib/entity-resolution";

export async function GET() {
  try {
    // 1. Load all contacts
    const contactRows = await db.select().from(contacts);

    // 2. Load the `people` memory topic
    const [peopleTopic] = await db
      .select({ content: memory.content })
      .from(memory)
      .where(eq(memory.topic, "people"))
      .limit(1);

    const peopleProfiles = peopleTopic
      ? parsePeopleMemory(peopleTopic.content)
      : [];

    // 3. Build resolution map
    const resolutionMap = resolveContacts(contactRows, peopleProfiles);

    // 4. Compute source counts
    //    contacts_table — keys that came from contacts rows
    let contactsTableCount = 0;
    for (const contact of contactRows) {
      const raw = contact.channelIds;
      if (raw && typeof raw === "object" && !Array.isArray(raw)) {
        contactsTableCount += Object.keys(raw as Record<string, unknown>).length;
      }
    }

    //    memory_people — keys that came from memory profiles (i.e. in map but not from contacts table)
    //    Re-build a contacts-only map to diff
    const contactsOnlyMap = resolveContacts(contactRows, []);
    const memoryPeopleCount = resolutionMap.size - contactsOnlyMap.size;

    //    unresolved — distinct (channel, sender) pairs in messages not in the resolution map
    const distinctSenders = await db
      .selectDistinct({ channel: messages.channel, sender: messages.sender })
      .from(messages);

    let unresolvedCount = 0;
    for (const row of distinctSenders) {
      if (!row.sender) continue;
      const key = `${row.channel}:${row.sender}`;
      if (!resolutionMap.has(key)) {
        unresolvedCount++;
      }
    }

    // 5. Serialize the Map to a plain object
    const resolutions: Record<string, string> = {};
    for (const [key, name] of resolutionMap.entries()) {
      resolutions[key] = name;
    }

    return NextResponse.json({
      resolutions,
      source_counts: {
        contacts_table: contactsTableCount,
        memory_people: memoryPeopleCount > 0 ? memoryPeopleCount : 0,
        unresolved: unresolvedCount,
      },
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
